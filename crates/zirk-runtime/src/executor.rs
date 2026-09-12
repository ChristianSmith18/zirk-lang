//! The single-threaded cooperative executor: one loop, a first-in-first-out
//! ready queue, and the timer service, driving internal stackful-coroutine
//! scheduler tasks. These tasks are runtime control blocks, not language-level
//! `Task<T>` values.
//!
//! No task is ever preempted — a task yields only at a safe point (here, an
//! explicit [`yield_now`], [`await_task`], or a future channel / timer wait).
//! The loop, the task control blocks, and the timer heap all live in one
//! [`Executor`] owned by one OS thread (`ADR-017`).
//!
//! # Reentrancy
//!
//! [`spawn`], [`yield_now`], [`await_task`] and [`suspend_current`] are called
//! from inside a running task body and must reach back into the `Executor` that
//! is driving that body. They do so through a raw-pointer thread-local
//! ([`EXEC`]). Soundness rests on two facts: execution is single-threaded, and
//! the loop never holds a `&mut Executor` across `TaskContext::resume` — it
//! takes the `TaskContext` out of its slot first (design D1/D6). So the only
//! `&mut Executor` alive while a body runs is the one that body creates.
//!
//! Design: `openspec/changes/fase-5-executor-core/design.md` D3, D5–D8.

use std::cell::Cell;
use std::collections::VecDeque;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::context::{self, TaskContext};
use crate::failure::fatal;
use crate::task::{
    CleanupState, ScopeId, ScopeRegistry, TaskId, TaskOutcome, TaskRegistry, TaskState, WaitReason,
};

/// Stack size for the root task — it runs the program's `main`, which used to
/// run on the OS main-thread stack. 8 MiB matches a typical main-thread stack;
/// it is one allocation for the whole program.
pub const ROOT_TASK_STACK_BYTES: usize = 8 * 1024 * 1024;

thread_local! {
    /// The executor currently running on this thread, or null. Set for the
    /// duration of [`Executor::run`] only.
    static EXEC: Cell<*mut Executor> = const { Cell::new(std::ptr::null_mut()) };
    /// The task whose body is executing right now, if any.
    static CURRENT_TASK: Cell<Option<TaskId>> = const { Cell::new(None) };
}

/// Runs `f` with a fresh, short-lived `&mut Executor`. The closure must not
/// resume or suspend a task, so no second `&mut Executor` is ever live.
fn with_exec<R>(f: impl FnOnce(&mut Executor) -> R) -> R {
    let ptr = EXEC.get();
    assert!(!ptr.is_null(), "no executor is running on this thread");
    // SAFETY: single-threaded; `f` runs to completion without re-entering the
    // executor (it never calls `resume`), so this is the only live `&mut`.
    f(unsafe { &mut *ptr })
}

/// Cross-thread wake channel (`ADR-019` D1). A `parallel` worker signals this
/// when a region's chunks have all joined, and the collector signals it when a
/// stop-the-world collection is requested — either way the executor thread
/// leaves an idle wait promptly instead of only at its 1 ms backstop.
static POOL_EVENT: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Woken from a worker thread or the collector — see [`POOL_EVENT`].
pub fn notify_pool_event() {
    let (flag, cv) = &POOL_EVENT;
    *flag.lock().unwrap() = true;
    cv.notify_all();
}

/// The executor thread waits here when it has nothing ready to run but a
/// `parallel` region is still outstanding. `timeout` is a backstop; a
/// [`notify_pool_event`] wakes it immediately.
fn wait_pool_event(timeout: Duration) {
    let (flag, cv) = &POOL_EVENT;
    let mut set = flag.lock().unwrap();
    if !*set {
        set = cv.wait_timeout(set, timeout).unwrap().0;
    }
    *set = false;
}

/// The cooperative executor.
pub struct Executor {
    registry: TaskRegistry,
    scopes: ScopeRegistry,
    ready: VecDeque<TaskId>,
    timers: crate::timer::TimerService,
    root: Option<TaskId>,
    /// Branches suspended inside a `parallel` region: each entry is the
    /// region's outstanding-chunk counter and the branch to wake when it hits
    /// zero (`ADR-019` D1, `parallel-cpu-regions`).
    parallel_regions: Vec<(std::sync::Arc<AtomicUsize>, TaskId)>,
}

/// The observable state of a scope close attempt. Lowered `ScopeExit` retries
/// after the executor has driven the registered branches to a terminal cleanup
/// state; it never permits control to leave the lexical scope early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeExit {
    Pending,
    Complete {
        primary_failure: Option<TaskId>,
        suppressed: Vec<TaskId>,
    },
}

/// Internal runtime representation of the compiler-known `CancelledError`.
/// Lowered language exception handling will map this payload to its public
/// throwable; keeping it distinct from a fatal runtime failure lets scope
/// cleanup treat cancellation as an ordinary cooperative branch outcome.
#[derive(Debug)]
pub struct Cancelled;

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            registry: TaskRegistry::new(),
            scopes: ScopeRegistry::new(),
            ready: VecDeque::new(),
            timers: crate::timer::TimerService::new(),
            root: None,
            parallel_regions: Vec::new(),
        }
    }

    /// Wakes every branch whose `parallel` region has finished, and reports
    /// whether any region is still outstanding.
    fn service_parallel_regions(&mut self) -> bool {
        let mut still_running = Vec::new();
        for (remaining, owner) in std::mem::take(&mut self.parallel_regions) {
            if remaining.load(Ordering::SeqCst) == 0 {
                self.make_ready(owner);
            } else {
                still_running.push((remaining, owner));
            }
        }
        let outstanding = !still_running.is_empty();
        self.parallel_regions = still_running;
        outstanding
    }

    /// Opens a structured-concurrency scope.
    pub fn scope_enter(&mut self) -> ScopeId {
        self.scopes.insert()
    }

    /// Makes `task` owned by `scope`. A branch can have exactly one lexical
    /// owner; registering it twice is an executor bug rather than a recoverable
    /// runtime condition.
    pub fn branch_register(&mut self, scope: ScopeId, task: TaskId) {
        assert!(self.scopes.get(scope).is_some(), "unknown scope");
        let tcb = self.registry.get_mut(task).expect("unknown branch");
        assert!(tcb.parent.is_none(), "branch already belongs to a scope");
        tcb.parent = Some(scope);
        self.scopes
            .get_mut(scope)
            .expect("scope was checked above")
            .branches
            .push(task);
    }

    /// Makes a timer job owned by `scope`. Ambient timers receive parent
    /// cancellation, but `scope_exit` never waits for their natural result.
    pub fn ambient_timer_register(&mut self, scope: ScopeId, task: TaskId) {
        assert!(self.scopes.get(scope).is_some(), "unknown scope");
        let tcb = self.registry.get_mut(task).expect("unknown timer job");
        assert!(tcb.parent.is_none(), "timer job already belongs to a scope");
        tcb.parent = Some(scope);
        self.scopes
            .get_mut(scope)
            .expect("scope was checked above")
            .ambient_timers
            .push(task);
    }

    /// Requests cooperative cancellation. Suspending branches are requeued so
    /// their next safe-point check can observe the request; ready/running and
    /// terminal branches need no queue mutation.
    pub fn request_cancel(&mut self, task: TaskId) {
        let wake = match self.registry.get_mut(task) {
            Some(tcb) if !tcb.state.is_terminal() => {
                if tcb.cancel_requested {
                    return;
                }
                tcb.cancel_requested = true;
                tcb.state == TaskState::Suspended
            }
            _ => false,
        };
        if wake {
            self.make_ready(task);
        }
    }

    /// Requests cancellation for every branch currently owned by `scope`.
    pub fn request_scope_cancel(&mut self, scope: ScopeId) {
        let owned = {
            let scope = self.scopes.get_mut(scope).expect("unknown scope");
            if scope.cancel_requested {
                return;
            }
            scope.cancel_requested = true;
            scope
                .branches
                .iter()
                .chain(&scope.ambient_timers)
                .copied()
                .collect::<Vec<_>>()
        };
        for branch in owned {
            self.request_cancel(branch);
        }
    }

    /// Advances the join-or-cancel-and-clean protocol for `scope`.
    pub fn scope_exit(&mut self, scope: ScopeId) -> ScopeExit {
        let (branches, ambient_timers) = {
            let scope_tcb = self.scopes.get(scope).expect("unknown scope");
            (scope_tcb.branches.clone(), scope_tcb.ambient_timers.clone())
        };

        // Ambient work is support work: cancellation happens when the lexical
        // scope closes, but it must not make a scope wait for a timer's normal
        // completion (notably an infinite `Timer.every`).
        for timer in ambient_timers {
            self.request_cancel(timer);
        }

        let failures: Vec<TaskId> = branches
            .iter()
            .copied()
            .filter(|branch| {
                self.registry
                    .get(*branch)
                    .is_some_and(|tcb| tcb.state == TaskState::Failed)
            })
            .collect();

        if let Some(primary) = failures.first().copied() {
            let should_cancel = {
                let scope_tcb = self.scopes.get_mut(scope).expect("scope exists");
                if scope_tcb.primary_failure.is_none() {
                    scope_tcb.primary_failure = Some(primary);
                    scope_tcb.suppressed = failures.into_iter().skip(1).collect();
                    true
                } else {
                    false
                }
            };
            if should_cancel {
                for branch in &branches {
                    if *branch != primary {
                        self.request_cancel(*branch);
                    }
                }
            }
        }

        let all_clean = branches.iter().all(|branch| {
            // A branch whose result was already consumed by `wait()` is
            // reclaimed out of the registry — it finished and was cleaned, so
            // its absence counts as terminal-and-clean here.
            self.registry.get(*branch).is_none_or(|tcb| {
                tcb.state.is_terminal() && tcb.cleanup_state == CleanupState::Done
            })
        });
        if !all_clean {
            return ScopeExit::Pending;
        }

        let scope_tcb = self.scopes.remove(scope).expect("scope exists");
        ScopeExit::Complete {
            primary_failure: scope_tcb.primary_failure,
            suppressed: scope_tcb.suppressed,
        }
    }

    /// Spawns `body` as the root task, runs the loop until the root and every
    /// descendant is terminal, and returns the root's outcome.
    ///
    /// The root gets a large stack ([`ROOT_TASK_STACK_BYTES`]): it runs the
    /// whole program's `main`, which historically ran on the operating-system
    /// main-thread stack, so it must not be squeezed onto the 128 KiB a
    /// spawned task gets. One allocation for the life of the program.
    pub fn run_with_root<F>(mut self, body: F) -> TaskOutcome
    where
        F: FnOnce() -> usize + Send + 'static,
    {
        let root = self
            .registry
            .insert(TaskContext::new(ROOT_TASK_STACK_BYTES, move |_suspender| {
                body()
            }));
        self.root = Some(root);
        self.ready.push_back(root);

        // One executor runs at a time, process-wide: a real program runs
        // `run_with_root` exactly once, and serializing it in the unit-test
        // binary is what keeps this run's stop-the-world collection from
        // racing another test's executor over the shared collector state
        // (`ADR-019`).
        static GC_WORLD: Mutex<()> = Mutex::new(());
        let _gc_world = GC_WORLD.lock().unwrap_or_else(|p| p.into_inner());

        // Publish a fresh heap for this run — the executor thread and every
        // `parallel` worker thread allocate onto it (`crate::collector`'s
        // `mod heap`). Dropped (and its list freed) when the run ends.
        let heap_scope = HeapScope::new();

        // Publish `self` and drive the loop through `with_exec` only. `self` is
        // not touched again until `drive` returns, so no `&mut Executor` is ever
        // live at the same time as one created inside a task body.
        let previous = EXEC.replace(&mut self as *mut Executor);
        crate::collector::set_task_root_walker(Some(walk_all_task_roots));
        let restore = Restore(previous);
        drive();
        drop(restore);
        drop(heap_scope);

        let mut root_tcb = self
            .registry
            .remove(root)
            .expect("the root task must still be registered at shutdown");
        root_tcb
            .outcome
            .take()
            .expect("a terminal task always has an outcome")
    }

    /// A task reached a terminal state: record its outcome, wake its waiter,
    /// try to reclaim.
    fn complete(&mut self, task: TaskId, outcome: TaskOutcome, state: TaskState) {
        let waiter = {
            let tcb = self.registry.get_mut(task).unwrap();
            tcb.outcome = Some(outcome);
            tcb.state = state;
            tcb.waiter.take()
        };
        if let Some(waiter) = waiter {
            self.make_ready(waiter);
        }
        self.reclaim();
    }

    fn wake_expired_timers(&mut self) {
        for (_timer, payload) in self.timers.poll_expired(Instant::now()) {
            let waiter = TaskId::from_bits(payload);
            self.make_ready(waiter);
        }
    }

    /// Move a suspended task back to the ready queue (no-op if it is gone or
    /// already ready / terminal).
    fn make_ready(&mut self, task: TaskId) {
        if let Some(tcb) = self.registry.get_mut(task)
            && tcb.state == TaskState::Suspended
        {
            tcb.wait = WaitReason::None;
            tcb.state = TaskState::Ready;
            self.ready.push_back(task);
        }
    }

    fn all_tasks_terminal(&self) -> bool {
        self.registry
            .live_ids()
            .all(|id| self.registry.get(id).unwrap().state.is_terminal())
    }

    /// Free the slot (and stack) of every terminal task whose result has been
    /// consumed. A terminal task with an un-woken waiter, or a completed task
    /// nobody has awaited yet, stays until it is consumed (or until the
    /// executor is dropped). Full detach / structured cleanup arrives with the
    /// language surface — `cleanup_state` is already the gate.
    fn reclaim(&mut self) {
        let current = CURRENT_TASK.get();
        let dead: Vec<TaskId> = self
            .registry
            .live_ids()
            .filter(|&id| {
                if Some(id) == self.root || Some(id) == current {
                    return false;
                }
                let tcb = self.registry.get(id).unwrap();
                tcb.state.is_terminal()
                    && tcb.result_consumed
                    && tcb.cleanup_state == CleanupState::Done
            })
            .collect();
        for id in dead {
            self.registry.remove(id);
        }
    }
}

/// Owns this run's published collector heap. On drop it unpublishes the heap
/// (so later bootstrap allocations fall back to the thread-local one) and frees
/// everything still on its list.
struct HeapScope {
    heap: Box<crate::collector::PublishedHeap>,
    _world: crate::collector::WorldGuard,
}

impl HeapScope {
    fn new() -> Self {
        let mut heap = crate::collector::PublishedHeap::new();
        crate::collector::publish_executor_heap(&mut *heap as *mut _);
        // The executor thread is now in its own world: its allocations go to
        // the published heap.
        let _world = crate::collector::enter_executor_world();
        HeapScope { heap, _world }
    }
}

impl Drop for HeapScope {
    fn drop(&mut self) {
        // `_world` drops after this (field order) — but unpublish + free must
        // see this thread still "in world" is irrelevant; what matters is no
        // other thread races. Unpublish first, then free the list.
        crate::collector::unpublish_executor_heap();
        crate::collector::free_all(&mut self.heap);
    }
}

/// Restores the collector hooks (and the previous `EXEC`) on every exit path
/// of a run, including an unwinding task panic.
struct Restore(*mut Executor);
impl Drop for Restore {
    fn drop(&mut self) {
        EXEC.set(self.0);
        crate::collector::set_active_frames(std::ptr::null_mut());
        if self.0.is_null() {
            crate::collector::set_task_root_walker(None);
        }
    }
}

/// The scheduling loop. Every executor access goes through a short-lived
/// `with_exec`; `run_one_turn` is the only thing that hands control to a task,
/// and it holds no executor borrow while it does.
fn drive() {
    loop {
        // The executor parks at a safepoint at its next scheduling turn so a
        // stop-the-world collection requested from a `parallel` worker can run
        // (`ADR-019` D4, `parallel-cpu-regions` task 3.2).
        crate::collector::safepoint_poll();
        let next = with_exec(|exec| {
            exec.wake_expired_timers();
            let regions_outstanding = exec.service_parallel_regions();
            match exec.ready.pop_front() {
                Some(task) => NextStep::Run(task),
                None if exec.all_tasks_terminal() && !regions_outstanding => NextStep::Done,
                None if regions_outstanding => NextStep::WaitPool,
                None => match exec.timers.peek_deadline() {
                    Some(deadline) => NextStep::SleepUntil(deadline),
                    None => NextStep::Unresolvable,
                },
            }
        });
        match next {
            NextStep::Run(task) => run_one_turn(task),
            NextStep::Done => break,
            // A `parallel` region is running on the worker pool and no I/O
            // branch is ready. Wait for a chunk to join (or a GC request);
            // the 1 ms is only a backstop, `notify_pool_event` wakes us.
            NextStep::WaitPool => wait_pool_event(Duration::from_millis(1)),
            NextStep::SleepUntil(deadline) => {
                let now = Instant::now();
                if deadline > now {
                    std::thread::sleep(deadline - now);
                }
            }
            // A real program bug (a deadlock), not a fatal-error check: unwind
            // out of the executor so the process exits nonzero, never hangs.
            NextStep::Unresolvable => panic!(
                "executor: unresolvable wait — every remaining task is blocked \
                 and no timer is armed"
            ),
        }
    }
}

enum NextStep {
    Run(TaskId),
    Done,
    WaitPool,
    SleepUntil(Instant),
    Unresolvable,
}

/// Resume `task` once and act on the result. No executor borrow is held across
/// `TaskContext::resume`.
fn run_one_turn(task: TaskId) {
    let mut context = with_exec(|exec| {
        let tcb = exec
            .registry
            .get_mut(task)
            .expect("a task on the ready queue must be registered");
        tcb.state = TaskState::Running;
        tcb.wait = WaitReason::None;
        // Route push/pop and any collection triggered inside this body at the
        // task's own root chain — a `Vec` in a boxed, address-stable control
        // block, so the pointer stays valid across the resume.
        let roots: *mut Vec<crate::collector::Frame> = &mut tcb.roots;
        crate::collector::set_active_frames(roots);
        tcb.context
            .take()
            .expect("a ready task always holds its context")
    });

    CURRENT_TASK.set(Some(task));
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| context.resume()));
    CURRENT_TASK.set(None);
    crate::collector::set_active_frames(std::ptr::null_mut());

    with_exec(move |exec| {
        exec.registry
            .get_mut(task)
            .expect("the task cannot vanish while its context is out")
            .context = Some(context);

        match outcome {
            Ok(crate::context::Run::Suspended) => {
                let tcb = exec.registry.get_mut(task).unwrap();
                tcb.state = TaskState::Suspended;
                if tcb.wait == WaitReason::Yielded {
                    tcb.wait = WaitReason::None;
                    tcb.state = TaskState::Ready;
                    exec.ready.push_back(task);
                }
            }
            Ok(crate::context::Run::Finished(value)) => {
                exec.complete(task, TaskOutcome::Value(value), TaskState::Completed);
            }
            Err(payload) => {
                let state = if payload.is::<Cancelled>() {
                    TaskState::Cancelled
                } else {
                    TaskState::Failed
                };
                exec.complete(task, TaskOutcome::Panicked(payload), state);
            }
        }
    });
}

/// Visits the roots in every live task's frame chain — registered with the
/// collector for the duration of a run (design D4). Marking is idempotent, so
/// visiting the running task's chain here as well as through `ACTIVE_FRAMES` is
/// harmless.
fn walk_all_task_roots(mark: &mut dyn FnMut(*mut std::ffi::c_void)) {
    with_exec(|exec| {
        for id in exec.registry.live_ids() {
            let tcb = exec.registry.get(id).expect("a live id resolves");
            if let Some(capture) = tcb.capture_root {
                mark(capture);
            }
            for frame in &tcb.roots {
                for index in 0..frame.count {
                    // SAFETY: same contract as `zirk_rt_push_frame` — each entry
                    // is the address of a reference-typed slot outliving the
                    // frame.
                    let root_addr = unsafe { *frame.roots.add(index) };
                    if !root_addr.is_null() {
                        let object = unsafe { *(root_addr as *mut *mut std::ffi::c_void) };
                        mark(object);
                    }
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Called from inside a running task body
// ---------------------------------------------------------------------------

/// Spawns `body` as a new task in the running executor. It starts as ready and
/// runs no later than the current task's next safe point.
pub fn spawn<F>(body: F) -> TaskId
where
    F: FnOnce() -> usize + Send + 'static,
{
    spawn_with_capture_root(body, std::ptr::null_mut())
}

/// Opens a scope in the executor currently running this task.
pub fn scope_enter_current() -> ScopeId {
    with_exec(|exec| exec.scope_enter())
}

/// Registers a child branch with its lexical scope.
pub fn branch_register_current(scope: ScopeId, branch: TaskId) {
    with_exec(|exec| exec.branch_register(scope, branch));
}

/// Registers an ambient timer job with its lexical scope.
pub fn ambient_timer_register_current(scope: ScopeId, timer: TaskId) {
    with_exec(|exec| exec.ambient_timer_register(scope, timer));
}

/// Advances a scope close. `true` means every branch is terminal and cleanup
/// finished; lowering retries after suspension while it is `false`.
pub fn scope_exit_current(scope: ScopeId) -> bool {
    matches!(
        with_exec(|exec| exec.scope_exit(scope)),
        ScopeExit::Complete { .. }
    )
}

/// The unwind payload of a `concurrent` branch that ended on an unhandled Zirk
/// exception. The `usize` is the pending-exception object pointer, taken from
/// the thread slot by the branch thunk before it unwinds so a sibling running
/// next does not mistake it for its own failure.
pub struct BranchFailure(pub usize);

/// One turn of the scope-close protocol. `None` means the scope is still
/// waiting on a branch; `Some(exception)` means it is done, carrying the
/// primary failure's exception pointer (as bits) to re-raise, or `None` inside
/// the `Some` when every branch finished normally.
pub fn scope_exit_poll(scope: ScopeId) -> Option<Option<usize>> {
    match with_exec(|exec| exec.scope_exit(scope)) {
        ScopeExit::Pending => None,
        ScopeExit::Complete {
            primary_failure, ..
        } => Some(primary_failure.and_then(take_branch_failure)),
    }
}

/// Takes a terminal branch's [`BranchFailure`] payload, if that is how it
/// ended. Leaves any other outcome in place.
fn take_branch_failure(task: TaskId) -> Option<usize> {
    with_exec(|exec| {
        let tcb = exec.registry.get_mut(task)?;
        match tcb.outcome.take() {
            Some(TaskOutcome::Panicked(payload)) => match payload.downcast::<BranchFailure>() {
                Ok(failure) => Some(failure.0),
                Err(payload) => {
                    tcb.outcome = Some(TaskOutcome::Panicked(payload));
                    None
                }
            },
            other => {
                tcb.outcome = other;
                None
            }
        }
    })
}

/// Requests cancellation of a job idempotently.
pub fn cancel_current(branch: TaskId) {
    with_exec(|exec| exec.request_cancel(branch));
}

/// Spawns generated code and roots its callable capture block for the task's
/// entire lifetime, including before the body installs compiler frame roots.
pub fn spawn_with_capture_root<F>(body: F, capture_root: *mut std::ffi::c_void) -> TaskId
where
    F: FnOnce() -> usize + Send + 'static,
{
    with_exec(|exec| {
        let id = exec.registry.insert_with_capture_root(
            TaskContext::new(
                crate::context::DEFAULT_TASK_STACK_BYTES,
                move |_suspender| body(),
            ),
            (!capture_root.is_null()).then_some(capture_root),
        );
        exec.ready.push_back(id);
        id
    })
}

/// Yields the current task: it returns to the back of the ready queue and the
/// executor runs everyone else first.
pub fn yield_now() {
    suspend_current(WaitReason::Yielded);
}

/// Suspends the current branch inside a `parallel` region: it has submitted the
/// region's chunks to the worker pool, and yields the executor thread so other
/// I/O branches keep running (`ADR-019` D1). The branch wakes when `remaining`
/// reaches zero — a worker decrements it as each chunk joins and calls
/// [`notify_pool_event`] on the last one.
pub fn block_current_on_region(remaining: std::sync::Arc<AtomicUsize>) {
    let me = CURRENT_TASK
        .get()
        .expect("a parallel region runs inside a task body");
    if remaining.load(Ordering::SeqCst) == 0 {
        return;
    }
    with_exec(|exec| exec.parallel_regions.push((remaining, me)));
    suspend_current(WaitReason::Parallel);
}

/// Suspends the current task until at least `delay_nanos` from now, letting the
/// executor run other tasks meanwhile. A negative delay is a fatal error.
/// (This is the timer safe point `await ... timeout` and `select { after ... }`
/// will build on.)
pub fn sleep(delay_nanos: i64) {
    check_cancelled();
    let me = CURRENT_TASK.get().expect("sleep outside a task body");
    let timer = with_exec(|exec| match exec.timers.arm(delay_nanos, me.to_bits()) {
        Ok(id) => id,
        Err(_) => fatal("sleep with a negative duration"),
    });
    suspend_current(WaitReason::Timer(timer.to_bits()));
    check_cancelled();
}

/// Delivers a pending cancellation at an explicit safe point. There is no
/// shielded region in this change, but preserving the depth check makes the
/// rule forward-compatible with the non-cancellable region introduced later.
pub fn check_cancelled() {
    let me = CURRENT_TASK
        .get()
        .expect("check_cancelled outside a task body");
    let cancelled = with_exec(|exec| {
        exec.registry
            .get(me)
            .is_some_and(|tcb| tcb.cancel_requested && tcb.shield_depth == 0)
    });
    if cancelled {
        std::panic::panic_any(Cancelled);
    }
}

/// Records `reason` on the current task and suspends it. The executor decides
/// (from `reason`) whether to re-enqueue it or leave it parked.
pub fn suspend_current(reason: WaitReason) {
    let me = CURRENT_TASK
        .get()
        .expect("suspend_current outside a task body");
    with_exec(|exec| {
        let tcb = exec.registry.get_mut(me).expect("the current task");
        tcb.wait = reason;
        tcb.state = TaskState::Suspended;
    });
    // Control leaves the task here; no executor borrow is held.
    context::suspend_current();
}

/// Consumes the result of `target` exactly once, blocking the current task
/// until `target` is terminal. A second `await` of the same task, or an
/// `await` of a task that is already gone, is a fatal error.
pub fn await_task(target: TaskId) -> usize {
    let me = CURRENT_TASK.get().expect("await_task outside a task body");
    assert_ne!(Some(target), Some(me), "a task cannot await itself");

    let ready_now = with_exec(|exec| match exec.registry.get_mut(target) {
        None => fatal("await of a task that no longer exists"),
        Some(tcb) if tcb.state.is_terminal() => {
            if tcb.result_consumed {
                fatal("task result awaited more than once");
            }
            true
        }
        Some(tcb) => {
            if tcb.waiter.is_some() {
                fatal("task result awaited more than once");
            }
            tcb.waiter = Some(me);
            false
        }
    });

    if !ready_now {
        suspend_current(WaitReason::AwaitingTask(target));
    }

    with_exec(|exec| {
        let tcb = exec
            .registry
            .get_mut(target)
            .expect("an awaited task is kept alive until its result is taken");
        tcb.result_consumed = true;
        match tcb
            .outcome
            .take()
            .expect("a terminal task always has an outcome")
        {
            TaskOutcome::Value(value) => value,
            TaskOutcome::Panicked(payload) => std::panic::resume_unwind(payload),
        }
    })
}

/// Whether the calling thread is currently running a task body on the
/// cooperative executor — false on a `parallel` worker thread and during
/// bootstrap. [`crate::pool`] uses it to decide whether a `parallel` region
/// can block cooperatively (executor thread) or must run inline (worker
/// thread, for a nested region).
pub fn in_task_context() -> bool {
    CURRENT_TASK.get().is_some()
}

/// Whether `target` has reached a terminal state. Safe to call on a stale id
/// (returns `false`).
pub fn is_done(target: TaskId) -> bool {
    with_exec(|exec| {
        exec.registry
            .get(target)
            .is_some_and(|tcb| tcb.state.is_terminal())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn run<F: FnOnce() -> usize + Send + 'static>(body: F) -> usize {
        match Executor::new().run_with_root(body) {
            TaskOutcome::Value(v) => v,
            TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }
    }

    fn dormant_context() -> TaskContext {
        crate::context::spawn_default(|_s| 0)
    }

    #[test]
    fn scope_exit_waits_for_each_registered_branch() {
        let mut exec = Executor::new();
        let scope = exec.scope_enter();
        let a = exec.registry.insert(dormant_context());
        let b = exec.registry.insert(dormant_context());
        exec.branch_register(scope, a);
        exec.branch_register(scope, b);

        assert_eq!(exec.scope_exit(scope), ScopeExit::Pending);
        exec.registry.get_mut(a).unwrap().state = TaskState::Completed;
        exec.registry.get_mut(b).unwrap().state = TaskState::Completed;
        assert_eq!(
            exec.scope_exit(scope),
            ScopeExit::Complete {
                primary_failure: None,
                suppressed: vec![]
            }
        );
    }

    #[test]
    fn first_scope_failure_cancels_and_wakes_suspended_siblings() {
        let mut exec = Executor::new();
        let scope = exec.scope_enter();
        let failed = exec.registry.insert(dormant_context());
        let sibling = exec.registry.insert(dormant_context());
        exec.branch_register(scope, failed);
        exec.branch_register(scope, sibling);
        exec.registry.get_mut(failed).unwrap().state = TaskState::Failed;
        let sibling_tcb = exec.registry.get_mut(sibling).unwrap();
        sibling_tcb.state = TaskState::Suspended;
        sibling_tcb.wait = WaitReason::Timer(1);

        assert_eq!(exec.scope_exit(scope), ScopeExit::Pending);
        let sibling_tcb = exec.registry.get(sibling).unwrap();
        assert!(sibling_tcb.cancel_requested);
        assert_eq!(sibling_tcb.state, TaskState::Ready);
    }

    #[test]
    fn scope_exit_cancels_ambient_timers_without_joining_them() {
        let mut exec = Executor::new();
        let scope = exec.scope_enter();
        let timer = exec.registry.insert(dormant_context());
        exec.ambient_timer_register(scope, timer);

        assert_eq!(
            exec.scope_exit(scope),
            ScopeExit::Complete {
                primary_failure: None,
                suppressed: vec![]
            }
        );
        assert!(exec.registry.get(timer).unwrap().cancel_requested);
    }

    #[test]
    fn cancellation_is_delivered_when_a_sleeping_branch_is_woken() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let child = spawn(|| {
                    sleep(1_000_000_000);
                    0
                });
                yield_now(); // let the child arm its timer and suspend
                with_exec(|exec| exec.request_cancel(child));
                await_task(child)
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn a_root_with_no_children_runs_and_returns() {
        assert_eq!(run(|| 42), 42);
    }

    #[test]
    fn two_children_interleave_at_yield() {
        // Each child appends its id to a shared log on every turn. FIFO
        // scheduling gives a strict round-robin: a,b,a,b,...
        let log = Arc::new(std::sync::Mutex::new(String::new()));
        let l = Arc::clone(&log);
        run(move || {
            for (name, turns) in [('a', 3), ('b', 3)] {
                let l = Arc::clone(&l);
                spawn(move || {
                    for _ in 0..turns {
                        l.lock().unwrap().push(name);
                        yield_now();
                    }
                    0
                });
            }
            0
        });
        assert_eq!(*log.lock().unwrap(), "ababab");
    }

    #[test]
    fn await_produces_the_child_value_once() {
        let got = run(|| {
            let child = spawn(|| 0xBEEF);
            await_task(child)
        });
        assert_eq!(got, 0xBEEF);
    }

    #[test]
    fn await_of_an_already_finished_child_takes_the_fast_path() {
        let got = run(|| {
            let child = spawn(|| 7);
            yield_now(); // let the child finish first
            assert!(is_done(child));
            await_task(child)
        });
        assert_eq!(got, 7);
    }

    #[test]
    fn ready_tasks_are_served_first_in_first_out() {
        let order = Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
        let o = Arc::clone(&order);
        run(move || {
            for id in 0..5u32 {
                let o = Arc::clone(&o);
                spawn(move || {
                    o.lock().unwrap().push(id);
                    0
                });
            }
            0
        });
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn program_waits_for_a_background_child_after_root_returns() {
        let child_finished = Arc::new(AtomicUsize::new(0));
        let cf = Arc::clone(&child_finished);
        run(move || {
            let cf = Arc::clone(&cf);
            spawn(move || {
                for _ in 0..10 {
                    yield_now();
                }
                cf.store(1, Ordering::SeqCst);
                0
            });
            0 // root returns immediately, child still running
        });
        assert_eq!(child_finished.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_child_panic_surfaces_when_awaited() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let child = spawn(|| panic!("boom"));
                await_task(child)
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn sleep_wakes_the_task_via_the_timer_and_others_run_meanwhile() {
        let progress = Arc::new(AtomicUsize::new(0));
        let p = Arc::clone(&progress);
        let slept = run(move || {
            let p2 = Arc::clone(&p);
            // A busy sibling that keeps yielding while the root sleeps.
            spawn(move || {
                for _ in 0..20 {
                    p2.fetch_add(1, Ordering::SeqCst);
                    yield_now();
                }
                0
            });
            let before = Instant::now();
            sleep(3_000_000); // 3 ms
            assert!(before.elapsed().as_millis() >= 3);
            1
        });
        assert_eq!(slept, 1);
        assert!(
            progress.load(Ordering::SeqCst) > 0,
            "the sibling should have run while the root slept"
        );
    }

    #[test]
    fn idle_executor_with_only_a_sleeping_task_still_finishes() {
        // No sibling: the loop's ready queue empties, it sleeps until the
        // timer deadline, wakes the task, and the task completes.
        assert_eq!(
            run(|| {
                sleep(2_000_000);
                9
            }),
            9
        );
    }

    #[test]
    fn an_unresolvable_wait_aborts_instead_of_hanging() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                // Park forever awaiting a sibling that never finishes because
                // it, too, parks forever. No timer is armed.
                let a = spawn(|| {
                    suspend_current(WaitReason::AwaitingTask(TaskId::from_bits(u64::MAX)));
                    0
                });
                await_task(a)
            })
        });
        assert!(outcome.is_err(), "the executor must abort, not hang");
    }

    #[test]
    fn an_awaited_child_slot_is_reclaimed_after_consumption() {
        // Spawn many children one at a time, await each, and rely on
        // reclamation keeping the live-task count from growing without bound.
        run(|| {
            for _ in 0..50 {
                let c = spawn(|| 1);
                assert_eq!(await_task(c), 1);
            }
            with_exec(|e| {
                assert!(
                    e.registry.len() <= 2,
                    "consumed children were not reclaimed"
                )
            });
            0
        });
    }
}
