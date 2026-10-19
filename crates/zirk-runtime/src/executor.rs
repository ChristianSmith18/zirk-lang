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
use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::background::{
    AdmissionError, BackgroundDiagnostic, BackgroundJobId, BackgroundSupervisor,
};
use crate::context::{self, TaskContext};
use crate::failure::fatal;
use crate::services::{ServiceContext, ServiceError, ServiceId, ServiceState, ServiceSupervisor};
use crate::task::{
    CleanupState, ScopeId, ScopeMode, ScopeRegistry, TaskId, TaskOutcome, TaskRegistry, TaskState,
    WaitReason,
};

/// Stack size for the root task — it runs the program's `main`, which used to
/// run on the OS main-thread stack. 8 MiB matches a typical main-thread stack;
/// it is one allocation for the whole program.
pub const ROOT_TASK_STACK_BYTES: usize = 8 * 1024 * 1024;
/// Maximum number of detached jobs admitted by one executor run.
pub const BACKGROUND_JOB_CAPACITY: usize = 256;
/// Cooperative shutdown grace for detached work after the root task finishes.
pub const BACKGROUND_SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
static LAST_HOST_SHUTDOWN_SIGNAL: AtomicUsize = AtomicUsize::new(0);
static NEXT_EXECUTOR_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) fn last_host_shutdown_signal() -> Option<i32> {
    let signal = LAST_HOST_SHUTDOWN_SIGNAL.load(Ordering::Acquire);
    (signal != 0).then_some(signal as i32)
}

pub(crate) fn take_host_shutdown_signal() -> Option<i32> {
    let signal = LAST_HOST_SHUTDOWN_SIGNAL.swap(0, Ordering::AcqRel);
    (signal != 0).then_some(signal as i32)
}

pub(crate) fn reset_host_shutdown_signal() {
    LAST_HOST_SHUTDOWN_SIGNAL.store(0, Ordering::Release);
}

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
static POOL_EVENT_EPOCH: AtomicUsize = AtomicUsize::new(0);
static POOL_TASK_WAITERS: Mutex<Vec<TaskId>> = Mutex::new(Vec::new());
static POOL_TASK_WAKEUPS: Mutex<Vec<TaskId>> = Mutex::new(Vec::new());
const NETWORK_WAITER_CAPACITY: usize = crate::task::MAX_ACTIVE_TASKS;

#[derive(Default)]
struct NetworkWaiters {
    waiting: Vec<(u64, TaskId)>,
    wakeups: Vec<(u64, TaskId)>,
}

impl NetworkWaiters {
    fn register(&mut self, target: (u64, TaskId), capacity: usize) -> Result<(), ()> {
        if self.waiting.contains(&target) {
            return Ok(());
        }
        if self.waiting.len() >= capacity {
            return Err(());
        }
        self.waiting.push(target);
        Ok(())
    }

    fn notify(&mut self, target: (u64, TaskId), capacity: usize) {
        if let Some(index) = self.waiting.iter().position(|waiter| *waiter == target) {
            self.waiting.swap_remove(index);
            debug_assert!(self.wakeups.len() < capacity);
            if self.wakeups.len() < capacity {
                self.wakeups.push(target);
            }
        }
    }

    fn unregister(&mut self, target: (u64, TaskId)) {
        self.waiting.retain(|waiter| *waiter != target);
        self.wakeups.retain(|wakeup| *wakeup != target);
    }

    fn take_wakeups(&mut self) -> Vec<(u64, TaskId)> {
        std::mem::take(&mut self.wakeups)
    }
}

static NETWORK_WAITERS: Mutex<NetworkWaiters> = Mutex::new(NetworkWaiters {
    waiting: Vec::new(),
    wakeups: Vec::new(),
});
static NETWORK_EVENT_EPOCH: AtomicUsize = AtomicUsize::new(0);
static EXTERNAL_CANCEL_REQUESTS: Mutex<Vec<(u64, TaskId)>> = Mutex::new(Vec::new());

pub(crate) enum NetworkPark {
    EventOccurred,
    Parked,
    CapacityExceeded,
}

/// Only one executor heap may be published at a time. This is process-global
/// rather than local to `run_with_root` so collector tests can coordinate with
/// the same ownership boundary.
static GC_WORLD: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn lock_gc_world_for_test() -> std::sync::MutexGuard<'static, ()> {
    GC_WORLD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Woken from a worker thread or the collector — see [`POOL_EVENT`].
pub fn notify_pool_event() {
    POOL_EVENT_EPOCH.fetch_add(1, Ordering::AcqRel);
    let waiting = {
        let mut waiters = POOL_TASK_WAITERS.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *waiters)
    };
    if !waiting.is_empty() {
        POOL_TASK_WAKEUPS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(waiting);
    }
    let (flag, cv) = &POOL_EVENT;
    *flag.lock().unwrap() = true;
    cv.notify_all();
}

pub(crate) fn current_executor_id() -> u64 {
    let ptr = EXEC.get();
    assert!(!ptr.is_null(), "network operation outside an executor");
    // SAFETY: executor identity is immutable while its run is active.
    unsafe { (*ptr).id }
}

pub(crate) fn notify_network_task(executor_id: u64, task: TaskId) {
    NETWORK_EVENT_EPOCH.fetch_add(1, Ordering::AcqRel);
    NETWORK_WAITERS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .notify((executor_id, task), NETWORK_WAITER_CAPACITY);
    let (flag, cv) = &POOL_EVENT;
    *flag.lock().unwrap_or_else(|error| error.into_inner()) = true;
    cv.notify_all();
}

/// Requests cancellation from a native reactor thread. The executor remains
/// the sole writer of task state; this bounded queue transfers the request to
/// its next scheduling turn and wakes it if it is parked on native I/O.
pub(crate) fn request_external_cancel(executor_id: u64, task: TaskId) -> bool {
    {
        let mut requests = EXTERNAL_CANCEL_REQUESTS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let request = (executor_id, task);
        if requests.contains(&request) {
            return true;
        }
        if requests.len() >= NETWORK_WAITER_CAPACITY {
            return false;
        }
        requests.push(request);
    }
    let (flag, cv) = &POOL_EVENT;
    *flag.lock().unwrap_or_else(|error| error.into_inner()) = true;
    cv.notify_all();
    true
}

pub(crate) fn pool_event_epoch() -> usize {
    POOL_EVENT_EPOCH.load(Ordering::Acquire)
}

pub(crate) fn network_event_epoch() -> usize {
    NETWORK_EVENT_EPOCH.load(Ordering::Acquire)
}

/// Registers the running caller before parking. The epoch check closes the
/// completion-before-registration race; the scheduler drains wakeups only
/// after this task has yielded back to it.
pub(crate) fn park_current_for_pool_event_if_unchanged(epoch: usize) -> bool {
    let task = CURRENT_TASK.get().expect("pool wait outside a task");
    let mut waiters = POOL_TASK_WAITERS.lock().unwrap_or_else(|e| e.into_inner());
    if POOL_EVENT_EPOCH.load(Ordering::Acquire) != epoch {
        return false;
    }
    if !waiters.contains(&task) {
        waiters.push(task);
    }
    drop(waiters);
    suspend_current(WaitReason::PoolEvent);
    true
}

/// Registers a network waiter before parking. The shared event epoch closes
/// the wake-before-registration race without waking unrelated I/O tasks.
pub(crate) fn park_current_for_network_event_if_unchanged(epoch: usize) -> NetworkPark {
    let task = CURRENT_TASK.get().expect("network wait outside a task");
    let executor_id = current_executor_id();
    let mut waiters = NETWORK_WAITERS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if NETWORK_EVENT_EPOCH.load(Ordering::Acquire) != epoch {
        return NetworkPark::EventOccurred;
    }
    let target = (executor_id, task);
    if waiters.register(target, NETWORK_WAITER_CAPACITY).is_err() {
        return NetworkPark::CapacityExceeded;
    }
    drop(waiters);
    suspend_current(WaitReason::Network);
    NetworkPark::Parked
}

fn drain_pool_task_wakeups() {
    let wakeups = std::mem::take(&mut *POOL_TASK_WAKEUPS.lock().unwrap_or_else(|e| e.into_inner()));
    if !wakeups.is_empty() {
        with_exec(|exec| {
            for task in wakeups {
                exec.make_ready(task);
            }
        });
    }
}

fn drain_network_task_wakeups() {
    let wakeups = NETWORK_WAITERS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take_wakeups();
    if !wakeups.is_empty() {
        with_exec(|exec| {
            for (executor_id, task) in wakeups {
                if executor_id == exec.id {
                    exec.make_ready(task);
                }
            }
        });
    }
}

fn drain_external_cancel_requests() {
    let executor_id = with_exec(|exec| exec.id);
    let requests = {
        let mut pending = EXTERNAL_CANCEL_REQUESTS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut local = Vec::new();
        pending.retain(|(id, task)| {
            if *id == executor_id {
                local.push(*task);
                false
            } else {
                true
            }
        });
        local
    };
    if !requests.is_empty() {
        with_exec(|exec| {
            for task in requests {
                exec.request_cancel(task);
            }
        });
    }
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
    id: u64,
    registry: TaskRegistry,
    scopes: ScopeRegistry,
    ready: VecDeque<TaskId>,
    any_waiters: HashMap<TaskId, Vec<TaskId>>,
    next_completion_sequence: u64,
    /// Non-terminal tasks, independent of terminal outcomes retained in registry.
    active_tasks: usize,
    timers: crate::timer::TimerService,
    root: Option<TaskId>,
    /// Branches suspended inside a `parallel` region: each entry is the
    /// region's outstanding-chunk counter and the branch to wake when it hits
    /// zero (`ADR-019` D1, `parallel-cpu-regions`).
    parallel_regions: Vec<(std::sync::Arc<AtomicUsize>, TaskId)>,
    // Shared separately so completion delivery can call into the supervisor
    // after releasing the executor's mutable borrow.
    background: std::sync::Arc<BackgroundSupervisor>,
    background_tasks: HashMap<TaskId, BackgroundJobId>,
    pending_background_failures: VecDeque<PendingBackgroundFailure>,
    background_delivery_active: bool,
    background_error_handler: Option<ManagedBackgroundErrorHandler>,
    background_handler_tasks: HashMap<TaskId, PendingBackgroundFailure>,
    background_handler_failures: HashMap<TaskId, usize>,
    started_tasks: HashSet<TaskId>,
    next_background_job: u64,
    background_shutdown_grace: Duration,
    background_shutdown_deadline: Option<Instant>,
    host_shutdown_signal: Option<i32>,
    services: ServiceSupervisor,
    service_callbacks: HashMap<ServiceId, ServiceCallback>,
    service_tasks: HashMap<TaskId, ServiceId>,
    service_task_ids: HashMap<ServiceId, TaskId>,
    service_drain: VecDeque<ServiceId>,
    services_started: bool,
}

struct ServiceCallback {
    dependencies: Vec<ServiceId>,
    capture_root: *mut std::ffi::c_void,
    callback: Option<Box<dyn FnOnce(ServiceContext) -> usize + Send + 'static>>,
}

/// A failed detached task awaiting supervisor delivery. `throwable` remains
/// part of the executor root set until the diagnostic/handler callback has
/// returned, even if the task's TCB is reclaimed first.
#[derive(Clone)]
struct PendingBackgroundFailure {
    job: BackgroundJobId,
    message: String,
    throwable: Option<usize>,
}

#[derive(Clone, Copy)]
pub(crate) struct ManagedBackgroundErrorHandler {
    pub thunk: extern "C-unwind" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
    ) -> u32,
    pub target: usize,
    pub capture: usize,
}

pub type ManagedBackgroundErrorHandlerThunk = extern "C-unwind" fn(
    *mut std::ffi::c_void,
    *mut std::ffi::c_void,
    *mut std::ffi::c_void,
) -> u32;

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

/// Typed rejection when a timer-backed task cannot reserve both resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerTaskAdmissionError {
    TaskCapacity,
    TimerCapacity,
    NegativeDuration,
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
        Self::with_background_shutdown_grace(BACKGROUND_SHUTDOWN_GRACE)
    }

    /// Create an executor with a caller-selected grace period for cancelling
    /// and draining detached background jobs after the root task completes.
    ///
    /// The deadline is cooperative: it bounds how long the executor waits
    /// before reporting unfinished jobs, but it cannot preempt a non-yielding
    /// task or release roots while work can still access the heap.
    pub fn with_background_shutdown_grace(background_shutdown_grace: Duration) -> Self {
        Self::with_background_failure_handler(background_shutdown_grace, |failure| {
            eprintln!(
                "background job {} failed: {}",
                failure.job.0, failure.message
            );
        })
    }

    fn with_background_failure_handler(
        background_shutdown_grace: Duration,
        on_error: impl Fn(&crate::background::BackgroundFailure) + Send + Sync + 'static,
    ) -> Self {
        Executor {
            id: NEXT_EXECUTOR_ID.fetch_add(1, Ordering::Relaxed),
            registry: TaskRegistry::new(),
            scopes: ScopeRegistry::new(),
            ready: VecDeque::new(),
            any_waiters: HashMap::new(),
            next_completion_sequence: 0,
            active_tasks: 0,
            timers: crate::timer::TimerService::new(),
            root: None,
            parallel_regions: Vec::new(),
            background: std::sync::Arc::new(BackgroundSupervisor::new(
                BACKGROUND_JOB_CAPACITY,
                on_error,
            )),
            background_tasks: HashMap::new(),
            pending_background_failures: VecDeque::new(),
            background_delivery_active: false,
            background_error_handler: None,
            background_handler_tasks: HashMap::new(),
            background_handler_failures: HashMap::new(),
            started_tasks: HashSet::new(),
            next_background_job: 1,
            background_shutdown_grace,
            background_shutdown_deadline: None,
            host_shutdown_signal: None,
            services: ServiceSupervisor::new(),
            service_callbacks: HashMap::new(),
            service_tasks: HashMap::new(),
            service_task_ids: HashMap::new(),
            service_drain: VecDeque::new(),
            services_started: false,
        }
    }

    /// Set the cooperative drain grace for the currently running executor.
    /// A negative nanosecond value is rejected by the C ABI before this call.
    pub fn set_current_background_shutdown_grace(grace: Duration) -> bool {
        with_exec(|exec| {
            if exec.background_shutdown_deadline.is_some() {
                return false;
            }
            exec.background_shutdown_grace = grace;
            true
        })
    }

    pub(crate) fn set_current_managed_background_error_handler(
        handler: ManagedBackgroundErrorHandler,
    ) {
        with_exec(|exec| exec.set_managed_background_error_handler(Some(handler)));
    }

    pub(crate) fn report_current_background_handler_failure(throwable: usize) {
        let task = CURRENT_TASK
            .get()
            .expect("background error handler failure must run in a scheduler task");
        with_exec(|exec| exec.managed_background_handler_failed(task, throwable));
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
        self.scope_enter_with_mode(ScopeMode::FailFast)
    }

    /// Opens a scope that joins every child while preserving each child's
    /// terminal outcome for later collection by the caller.
    pub fn scope_enter_collect_all(&mut self) -> ScopeId {
        self.scope_enter_with_mode(ScopeMode::CollectAll)
    }

    fn scope_enter_with_mode(&mut self, mode: ScopeMode) -> ScopeId {
        let id = self.scopes.insert();
        self.scopes.get_mut(id).expect("new scope exists").mode = mode;
        id
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

    /// Requests cooperative cancellation. Ordinary suspended branches are
    /// requeued so their next safe point can observe the request. A branch
    /// suspended on a parallel region stays parked until all workers join;
    /// resuming it early would expose incomplete result slots and could let
    /// the caller tear down state still used by those workers.
    pub fn request_cancel(&mut self, task: TaskId) {
        let (wake, waiting_any, waiting_parallel, waiting_timer, waiting_pool, waiting_network) =
            match self.registry.get_mut(task) {
                Some(tcb) if !tcb.state.is_terminal() => {
                    if tcb.cancel_requested {
                        return;
                    }
                    tcb.cancel_requested = true;
                    (
                        tcb.state == TaskState::Suspended,
                        tcb.wait == WaitReason::AwaitingAny,
                        tcb.wait == WaitReason::Parallel,
                        match tcb.wait {
                            WaitReason::Timer(timer) => {
                                Some(crate::timer::TimerId::from_bits(timer))
                            }
                            _ => None,
                        },
                        tcb.wait == WaitReason::PoolEvent,
                        tcb.wait == WaitReason::Network,
                    )
                }
                _ => (false, false, false, None, false, false),
            };
        if let Some(timer) = waiting_timer {
            self.timers.disarm(timer);
        }
        if wake && !waiting_parallel {
            if waiting_pool {
                POOL_TASK_WAITERS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .retain(|waiter| *waiter != task);
            }
            if waiting_network {
                NETWORK_WAITERS
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .unregister((self.id, task));
            }
            if waiting_any {
                self.remove_any_waiter(task);
            }
            self.make_ready(task);
        }
    }

    fn remove_any_waiter(&mut self, waiter: TaskId) {
        self.any_waiters.retain(|_, waiters| {
            waiters.retain(|candidate| *candidate != waiter);
            !waiters.is_empty()
        });
    }

    #[cfg(test)]
    fn waiter_registration_count(&self) -> usize {
        let task_waiters = self
            .registry
            .live_ids()
            .filter(|&task| {
                self.registry
                    .get(task)
                    .is_some_and(|tcb| tcb.waiter.is_some())
            })
            .count();
        let any_waiters = self.any_waiters.values().map(Vec::len).sum::<usize>();
        task_waiters + any_waiters
    }

    #[cfg(test)]
    fn task_count(&self) -> usize {
        self.registry
            .live_ids()
            .filter(|&task| {
                self.registry
                    .get(task)
                    .is_some_and(|tcb| !tcb.state.is_terminal() || !tcb.result_consumed)
            })
            .count()
    }

    fn wake_any_waiters(&mut self, completed: TaskId) {
        let waiters = self.any_waiters.remove(&completed).unwrap_or_default();
        for waiter in waiters {
            let is_waiting = self.registry.get(waiter).is_some_and(|tcb| {
                tcb.state == TaskState::Suspended && tcb.wait == WaitReason::AwaitingAny
            });
            if is_waiting {
                self.remove_any_waiter(waiter);
                self.make_ready(waiter);
            }
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
        let (branches, ambient_timers, mode) = {
            let scope_tcb = self.scopes.get(scope).expect("unknown scope");
            (
                scope_tcb.branches.clone(),
                scope_tcb.ambient_timers.clone(),
                scope_tcb.mode,
            )
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

        if mode == ScopeMode::FailFast
            && let Some(primary) = failures.first().copied()
        {
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
    pub fn run_with_root<F>(self, body: F) -> TaskOutcome
    where
        F: FnOnce() -> usize + Send + 'static,
    {
        self.run_with_root_deadline_policy(body, false)
    }

    /// Drives the root executor with an explicit host policy for an expired
    /// background drain. `terminate_on_deadline` is used only by generated
    /// CLI entrypoints; embedded hosts retain the safe wait-and-join behavior.
    pub fn run_with_root_deadline_policy<F>(
        mut self,
        body: F,
        terminate_on_deadline: bool,
    ) -> TaskOutcome
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
        self.active_tasks += 1;

        // One executor runs at a time, process-wide: a real program runs
        // `run_with_root` exactly once, and serializing it in the unit-test
        // binary is what keeps this run's stop-the-world collection from
        // racing another test's executor over the shared collector state
        // (`ADR-019`).
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
        drive(terminate_on_deadline);
        crate::http_api::close_executor_clients(self.id);
        // The scheduler has joined every service callback (including
        // cancellation cleanup); now release captures of registrations that
        // never started as well as any residual lifecycle metadata, before
        // unpublishing/freeing the run heap.
        self.service_callbacks.clear();
        debug_assert!(
            self.service_tasks.is_empty(),
            "shutdown leaked a service task mapping"
        );
        debug_assert!(
            self.service_task_ids.is_empty(),
            "shutdown leaked a service identity mapping"
        );
        debug_assert!(self.all_tasks_terminal(), "shutdown left a live task");
        debug_assert_eq!(self.active_tasks, 0, "shutdown leaked task permits");
        debug_assert!(self.timers.is_empty(), "shutdown left an armed timer");
        debug_assert_eq!(
            self.timers.active_registrations(),
            0,
            "shutdown leaked timer permits"
        );
        // A cancelled detached caller can unwind while a managed native
        // callback still owns pointers into its coroutine stack. `drive`
        // joins the managed pool before returning; only now are retained
        // shadow-frame addresses no longer reachable by native workers.
        let remaining_tasks = self.registry.live_ids().collect::<Vec<_>>();
        for id in remaining_tasks {
            if let Some(tcb) = self.registry.get_mut(id) {
                tcb.roots.clear();
            }
        }
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
        debug_assert!(state.is_terminal());
        // Native network resources are scoped to their creating task. Close
        // any handles the program did not explicitly release before making
        // the task's terminal outcome observable to joiners.
        crate::network_api::close_task_streams(task);
        crate::http_api::close_task_streams(task);
        crate::http_api::close_task_uploads(task);
        crate::http_api::close_task_server_producers(task);
        self.service_task_completed(task, state, &outcome);
        let was_nonterminal = self
            .registry
            .get(task)
            .is_some_and(|tcb| !tcb.state.is_terminal());
        let (background_failure, background_throwable) = match (&outcome, state) {
            (TaskOutcome::Panicked(payload), TaskState::Failed) => Some(
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| {
                        payload
                            .downcast_ref::<&'static str>()
                            .map(|message| (*message).to_owned())
                    })
                    .unwrap_or_else(|| "background task failed".to_owned()),
            )
            .map(|message| {
                let throwable = payload
                    .downcast_ref::<BranchFailure>()
                    .map(|failure| failure.0);
                (Some(message), throwable)
            })
            .unwrap_or((None, None)),
            _ => (None, None),
        };
        let background_job = self.background_tasks.remove(&task);
        let managed_handler_delivery = self.background_handler_tasks.remove(&task);
        self.started_tasks.remove(&task);
        let completion_sequence = self.next_completion_sequence;
        self.next_completion_sequence = self
            .next_completion_sequence
            .checked_add(1)
            .expect("task completion sequence exhausted");
        let (waiter, release_timer_reservation) = {
            let tcb = self.registry.get_mut(task).unwrap();
            tcb.outcome = Some(outcome);
            // The terminal outcome now owns and traces any promoted cleanup
            // Throwable, so its temporary TCB root can be released.
            tcb.cancel_cleanup_failure = None;
            tcb.completion_sequence = Some(completion_sequence);
            tcb.state = state;
            let release_timer_reservation = std::mem::replace(&mut tcb.timer_reservation, false);
            (tcb.waiter.take(), release_timer_reservation)
        };
        if release_timer_reservation {
            self.timers.release_reservation();
        }
        if was_nonterminal {
            self.active_tasks = self
                .active_tasks
                .checked_sub(1)
                .expect("active task reservation must be released exactly once");
        }
        if let Some(waiter) = waiter {
            self.make_ready(waiter);
        }
        if let Some(job) = background_job
            && let Some(message) = background_failure
        {
            self.pending_background_failures
                .push_back(PendingBackgroundFailure {
                    job,
                    message,
                    throwable: background_throwable,
                });
        } else if let Some(job) = background_job {
            // Successful and cancelled jobs release admission at terminal
            // completion too. Failures release their slot when deferred
            // delivery begins, before invoking the user error handler.
            self.background.complete(job, None);
        }
        if let Some(delivery) = managed_handler_delivery {
            let removed = self
                .pending_background_failures
                .pop_front()
                .expect("managed handler delivery retains its pending failure");
            debug_assert_eq!(removed.job, delivery.job);
            self.background_delivery_active = false;
            let handler_failure = self.background_handler_failures.contains_key(&task);
            if state == TaskState::Failed || state == TaskState::Cancelled || handler_failure {
                let message = if handler_failure {
                    "managed background error handler threw"
                } else if state == TaskState::Cancelled {
                    "managed background error handler was cancelled"
                } else {
                    "managed background error handler failed internally"
                };
                self.background
                    .report_error_handler_failure(delivery.job, message.to_owned());
                eprintln!(
                    "background error handler failed for job {:?}: {message}",
                    delivery.job
                );
            }
            self.background_handler_failures.remove(&task);
            self.background
                .complete_handled(delivery.job, delivery.message);
        }
        self.wake_any_waiters(task);
        self.reclaim();
    }

    fn spawn_background<F>(
        &mut self,
        body: F,
        capture_root: *mut std::ffi::c_void,
    ) -> Result<TaskId, AdmissionError>
    where
        F: FnOnce() -> usize + Send + 'static,
    {
        self.ensure_task_capacity()
            .map_err(|_| AdmissionError::TaskCapacityReached)?;
        let job = BackgroundJobId(self.next_background_job);
        let next_background_job = self
            .next_background_job
            .checked_add(1)
            .ok_or(AdmissionError::IdExhausted)?;
        self.background.admit(job)?;
        self.next_background_job = next_background_job;

        let task = self.registry.insert_with_capture_root(
            TaskContext::new(
                crate::context::DEFAULT_TASK_STACK_BYTES,
                move |_suspender| body(),
            ),
            (!capture_root.is_null()).then_some(capture_root),
        );
        self.background_tasks.insert(task, job);
        self.ready.push_back(task);
        self.active_tasks += 1;
        Ok(task)
    }

    fn ensure_task_capacity(&self) -> Result<(), crate::task::TaskAdmissionError> {
        if self.active_tasks >= crate::task::MAX_ACTIVE_TASKS {
            Err(crate::task::TaskAdmissionError::CapacityExceeded)
        } else {
            Ok(())
        }
    }

    fn background_diagnostics(&self) -> Vec<BackgroundDiagnostic> {
        self.background.diagnostics()
    }

    fn register_service<F>(
        &mut self,
        name: impl Into<String>,
        required: bool,
        dependencies: &[ServiceId],
        capture_root: *mut std::ffi::c_void,
        callback: F,
    ) -> Result<ServiceId, ServiceError>
    where
        F: FnOnce(ServiceContext) -> usize + Send + 'static,
    {
        let id = self
            .services
            .register_with_dependencies(name, required, dependencies)?;
        self.service_callbacks.insert(
            id,
            ServiceCallback {
                dependencies: dependencies.to_vec(),
                capture_root,
                callback: Some(Box::new(callback)),
            },
        );
        Ok(id)
    }

    fn start_services(&mut self) -> Result<(), ServiceError> {
        if !matches!(
            self.services.lifecycle(),
            ServiceState::Created | ServiceState::Starting | ServiceState::Ready
        ) {
            return Err(ServiceError::InvalidTransition {
                operation: crate::services::ServiceOperation::BeginStartup,
                state: self.services.lifecycle(),
            });
        }
        if !self.services_started {
            self.services.begin_startup()?;
            self.services_started = true;
        }
        self.start_eligible_services()?;
        Ok(())
    }

    fn start_eligible_services(&mut self) -> Result<(), ServiceError> {
        if !self.services_started
            || !matches!(
                self.services.lifecycle(),
                ServiceState::Starting | ServiceState::Ready
            )
        {
            return Ok(());
        }
        // Registration order is the iteration order of `services()`; dependencies
        // are rechecked by the supervisor immediately before each start.
        let candidates = self
            .services
            .services()
            .into_iter()
            .filter(|(_, _, _, state)| *state == ServiceState::Created)
            .map(|(id, _, _, _)| id)
            .collect::<Vec<_>>();
        for id in candidates {
            let eligible = self.service_callbacks.get(&id).is_some_and(|entry| {
                entry.dependencies.iter().all(|dependency| {
                    self.services.service_state(*dependency) == Ok(ServiceState::Ready)
                })
            });
            if !eligible {
                continue;
            }
            if self.ensure_task_capacity().is_err() {
                if let Ok(order) = self
                    .services
                    .fail_admission(id, "executor task capacity exceeded")
                {
                    self.service_drain = order.into();
                    self.discard_stopped_service_callbacks();
                    self.cancel_next_service();
                }
                return Err(ServiceError::TaskCapacityReached);
            }
            let (callback, capture_root) = match self.service_callbacks.get_mut(&id) {
                Some(entry)
                    if entry.dependencies.iter().all(|dependency| {
                        self.services.service_state(*dependency) == Ok(ServiceState::Ready)
                    }) =>
                {
                    let capture_root =
                        std::mem::replace(&mut entry.capture_root, std::ptr::null_mut());
                    (entry.callback.take(), capture_root)
                }
                _ => (None, std::ptr::null_mut()),
            };
            let Some(callback) = callback else { continue };
            if self.services.begin_service(id).is_err() {
                continue;
            }
            let context = ServiceContext::new(id);
            let task = self.registry.insert_with_capture_root(
                TaskContext::new(crate::context::DEFAULT_TASK_STACK_BYTES, move |_| {
                    callback(context)
                }),
                (!capture_root.is_null()).then_some(capture_root),
            );
            self.service_tasks.insert(task, id);
            self.service_task_ids.insert(id, task);
            self.ready.push_back(task);
            self.active_tasks += 1;
        }
        Ok(())
    }

    fn service_ready(&mut self, id: ServiceId) -> Result<ServiceState, ServiceError> {
        let task = CURRENT_TASK
            .get()
            .ok_or(ServiceError::ServiceNotStarting(id))?;
        if self.service_tasks.get(&task) != Some(&id) {
            return Err(ServiceError::ServiceNotStarting(id));
        }
        let state = self.services.report_ready(id)?;
        self.start_eligible_services()?;
        Ok(state)
    }

    fn service_failed(&mut self, task: TaskId) -> Result<(), ServiceError> {
        let id = self
            .service_tasks
            .get(&task)
            .copied()
            .ok_or(ServiceError::WrongExecutorTask)?;
        if crate::exceptions::zirk_rt_take_pending_exception().is_null() {
            return Err(ServiceError::ServiceNotStarting(id));
        }
        let state = self.services.service_state(id)?;
        let order = match state {
            ServiceState::Starting => self
                .services
                .fail_startup(id, "service callback raised a Zirk exception"),
            ServiceState::Ready => self
                .services
                .fail_runtime(id, "service callback raised a Zirk exception"),
            _ => return Err(ServiceError::ServiceNotStarting(id)),
        }?;
        self.service_drain = order.into();
        self.discard_stopped_service_callbacks();
        self.cancel_next_service();
        Ok(())
    }

    fn begin_service_shutdown(&mut self) {
        let Some(root) = self.root else { return };
        if self.host_shutdown_signal.is_none()
            && !self
                .registry
                .get(root)
                .is_some_and(|tcb| tcb.state.is_terminal())
        {
            return;
        }
        if !self.services_started
            || !matches!(
                self.services.lifecycle(),
                ServiceState::Starting | ServiceState::Ready
            )
        {
            return;
        }
        if let Ok(order) = self.services.request_shutdown() {
            self.service_drain = order.into();
            self.discard_stopped_service_callbacks();
            self.cancel_next_service();
        }
    }

    fn discard_stopped_service_callbacks(&mut self) {
        self.service_callbacks
            .retain(|id, _| self.services.service_state(*id) != Ok(ServiceState::Stopped));
    }

    fn cancel_next_service(&mut self) {
        if !self.service_drain.is_empty() {
            let id = self.service_drain.pop_front().unwrap();
            if let Some(task) = self.service_task_ids.get(&id).copied() {
                self.request_cancel(task);
            } else {
                let _ = self.services.notify_stopped(id);
                self.service_callbacks.remove(&id);
                self.cancel_next_service();
            }
        }
    }

    fn service_task_completed(&mut self, task: TaskId, state: TaskState, outcome: &TaskOutcome) {
        let Some(id) = self.service_tasks.remove(&task) else {
            return;
        };
        self.service_task_ids.remove(&id);
        let service_state = self.services.service_state(id).ok();
        match service_state {
            Some(ServiceState::Starting) => {
                let msg = service_outcome_message(state, outcome);
                if let Ok(order) = self.services.fail_startup(id, msg) {
                    self.service_drain = order.into();
                    self.discard_stopped_service_callbacks();
                    self.cancel_next_service();
                }
            }
            Some(ServiceState::Ready) => {
                let msg = service_outcome_message(state, outcome);
                if let Ok(order) = self.services.fail_runtime(id, msg) {
                    self.service_drain = order.into();
                    self.discard_stopped_service_callbacks();
                    self.cancel_next_service();
                }
            }
            Some(ServiceState::Draining) => {
                let _ = self.services.notify_stopped(id);
                self.service_callbacks.remove(&id);
                self.cancel_next_service();
            }
            _ => {}
        }
    }

    /// Deliver one queued notification without holding `&mut Executor` across
    /// the supervisor callback. The queue entry remains rooted until delivery
    /// returns, and the guard prevents callback-triggered recursive dispatch.
    fn dispatch_pending_background_failure() -> bool {
        enum Dispatch {
            Managed,
            Fallback(
                PendingBackgroundFailure,
                std::sync::Arc<BackgroundSupervisor>,
            ),
        }
        let pending = with_exec(|exec| {
            if exec.background_delivery_active {
                return None;
            }
            let pending = exec.pending_background_failures.front().cloned()?;
            if let (Some(handler), Some(throwable)) =
                (exec.background_error_handler, pending.throwable)
            {
                if exec.ensure_task_capacity().is_err() {
                    // Leave the typed failure rooted and queued. The scheduler
                    // will retry after another task releases capacity.
                    return None;
                }
                exec.background_delivery_active = true;
                let task = exec.registry.insert_with_capture_root(
                    TaskContext::new(crate::context::DEFAULT_TASK_STACK_BYTES, move |_| {
                        (handler.thunk)(
                            handler.target as *mut std::ffi::c_void,
                            handler.capture as *mut std::ffi::c_void,
                            throwable as *mut std::ffi::c_void,
                        ) as usize
                    }),
                    (handler.capture != 0).then_some(handler.capture as *mut std::ffi::c_void),
                );
                exec.background_handler_tasks.insert(task, pending);
                exec.ready.push_back(task);
                exec.active_tasks += 1;
                return Some(Dispatch::Managed);
            }
            // Native-only failures (without a managed Throwable) cannot be
            // passed to the source callback; preserve the documented stderr
            // diagnostic and release the queue entry after reporting it.
            exec.background_delivery_active = true;
            Some(Dispatch::Fallback(
                pending,
                std::sync::Arc::clone(&exec.background),
            ))
        });
        let Some(dispatch) = pending else {
            return false;
        };
        match dispatch {
            Dispatch::Managed => {}
            Dispatch::Fallback(pending, supervisor) => {
                supervisor.complete(pending.job, Some(pending.message.clone()));
                with_exec(|exec| {
                    let removed = exec
                        .pending_background_failures
                        .pop_front()
                        .expect("the in-flight background failure remains queued");
                    debug_assert_eq!(removed.job, pending.job);
                    exec.background_delivery_active = false;
                });
            }
        }
        true
    }

    fn set_managed_background_error_handler(
        &mut self,
        handler: Option<ManagedBackgroundErrorHandler>,
    ) {
        self.background_error_handler = handler;
    }

    fn managed_background_handler_failed(&mut self, task: TaskId, throwable: usize) {
        self.background_handler_failures.insert(task, throwable);
    }

    fn start_background_shutdown(&mut self) {
        if self.background_shutdown_deadline.is_some() {
            return;
        }
        let Some(root) = self.root else { return };
        if !self
            .registry
            .get(root)
            .is_some_and(|tcb| tcb.state.is_terminal())
        {
            return;
        }

        let deadline = Instant::now() + self.background_shutdown_grace;
        self.background_shutdown_deadline = Some(deadline);
        let jobs = self.background.begin_drain();
        let tasks = self
            .background_tasks
            .iter()
            .filter_map(|(task, job)| jobs.contains(job).then_some(*task))
            .collect::<Vec<_>>();
        let mut tasks = tasks;
        tasks.extend(self.background_handler_tasks.keys().copied());
        for task in tasks {
            self.request_cancel(task);
        }
        if jobs.is_empty() {
            let _ = self.background.drain_until(Instant::now());
        }
    }

    fn background_drain_expired(&self) -> bool {
        self.background_shutdown_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
            && if self.host_shutdown_signal.is_some() {
                !self.all_tasks_terminal()
            } else {
                !self.background.unfinished().is_empty() || !self.service_task_ids.is_empty()
            }
    }

    fn drain_expired_report(&self, jobs: Vec<u64>) -> DrainExpiredReport {
        let mut services = self
            .service_task_ids
            .keys()
            .map(|id| id.0)
            .collect::<Vec<_>>();
        services.sort_unstable();
        let other_tasks = if self.host_shutdown_signal.is_some() {
            self.registry
                .live_ids()
                .filter(|task| {
                    self.registry
                        .get(*task)
                        .is_some_and(|tcb| !tcb.state.is_terminal())
                })
                .filter(|task| {
                    !self.background_tasks.contains_key(task)
                        && !self.background_handler_tasks.contains_key(task)
                        && !self.service_tasks.contains_key(task)
                })
                .map(|task| task.to_bits())
                .collect()
        } else {
            Vec::new()
        };
        DrainExpiredReport {
            jobs,
            services,
            other_tasks,
        }
    }

    fn request_host_shutdown(&mut self, signal: i32) {
        if self.host_shutdown_signal.is_some() {
            return;
        }
        self.host_shutdown_signal = Some(signal);
        LAST_HOST_SHUTDOWN_SIGNAL.store(signal as usize, Ordering::Release);
        self.background_shutdown_deadline = Some(Instant::now() + self.background_shutdown_grace);
        let _ = self.background.begin_drain();
        self.begin_service_shutdown();

        // Service callbacks retain reverse-dependency shutdown ordering via
        // `cancel_next_service`; all other live tasks are independently
        // cancellable. Cancellation only marks/wakes tasks. Their cleanup runs
        // on their own stacks when the executor schedules them.
        let service_tasks = self.service_tasks.keys().copied().collect::<HashSet<_>>();
        let tasks = self
            .registry
            .live_ids()
            .filter(|task| !service_tasks.contains(task))
            .collect::<Vec<_>>();
        for task in tasks {
            self.request_cancel(task);
        }
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

    fn has_external_pool_waiters(&self) -> bool {
        self.registry.live_ids().any(|id| {
            self.registry.get(id).is_some_and(|tcb| {
                tcb.state == TaskState::Suspended && tcb.wait == WaitReason::PoolEvent
            })
        })
    }

    fn has_network_waiters(&self) -> bool {
        self.registry.live_ids().any(|id| {
            self.registry.get(id).is_some_and(|tcb| {
                tcb.state == TaskState::Suspended && tcb.wait == WaitReason::Network
            })
        })
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
fn drive(terminate_on_deadline: bool) {
    let mut managed_pool_shutdown: Option<std::thread::JoinHandle<()>> = None;
    loop {
        if let Some(signal) = crate::host_signal::take() {
            with_exec(|exec| exec.request_host_shutdown(signal));
        }
        // The executor parks at a safepoint at its next scheduling turn so a
        // stop-the-world collection requested from a `parallel` worker can run
        // (`ADR-019` D4, `parallel-cpu-regions` task 3.2).
        crate::collector::safepoint_poll();
        drain_pool_task_wakeups();
        drain_network_task_wakeups();
        drain_external_cancel_requests();
        while Executor::dispatch_pending_background_failure() {}
        let pool_shutdown_started = managed_pool_shutdown.is_some();
        let pool_shutdown_finished = managed_pool_shutdown
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished);
        let next = with_exec(|exec| {
            exec.wake_expired_timers();
            let regions_outstanding = exec.service_parallel_regions();
            exec.start_background_shutdown();
            exec.begin_service_shutdown();
            if exec.background_drain_expired() && !pool_shutdown_started {
                let report = exec.background.drain_until(Instant::now());
                // Preserve the previous grace-expiry contract: callbacks that
                // have not received their first executor turn are cancelled
                // without running user code. Already-started callers remain
                // registered and are driven until their managed callbacks join.
                let unstarted = exec
                    .background_tasks
                    .keys()
                    .filter(|task| !exec.started_tasks.contains(task))
                    .copied()
                    .collect::<Vec<_>>();
                // These tasks are still represented in the ready queue. Drop
                // those entries before terminal completion so the scheduler
                // cannot resume a TCB whose active reservation was released.
                exec.ready.retain(|ready| !unstarted.contains(ready));
                for task in unstarted {
                    exec.complete(
                        task,
                        TaskOutcome::Panicked(Box::new(Cancelled)),
                        TaskState::Cancelled,
                    );
                }
                let jobs = if exec.host_shutdown_signal.is_some() {
                    exec.registry
                        .live_ids()
                        .filter(|task| {
                            exec.registry
                                .get(*task)
                                .is_some_and(|tcb| !tcb.state.is_terminal())
                        })
                        .map(|task| task.to_bits())
                        .collect()
                } else {
                    report.unfinished.into_iter().map(|job| job.0).collect()
                };
                return NextStep::DrainExpired(exec.drain_expired_report(jobs));
            }
            match exec.ready.pop_front() {
                Some(task) => NextStep::Run(task),
                None if exec.all_tasks_terminal()
                    && !regions_outstanding
                    && pool_shutdown_finished =>
                {
                    NextStep::Done
                }
                None if exec.all_tasks_terminal() && !regions_outstanding => {
                    NextStep::StartManagedPoolShutdown
                }
                None if pool_shutdown_started => NextStep::WaitPool(Duration::from_secs(60)),
                None if regions_outstanding => NextStep::WaitPool(Duration::from_secs(60)),
                None if exec.has_external_pool_waiters() => NextStep::WaitPool(
                    exec.timers
                        .peek_deadline()
                        .map_or(Duration::from_secs(60), |deadline| {
                            deadline
                                .saturating_duration_since(Instant::now())
                                .min(Duration::from_secs(60))
                        }),
                ),
                None if exec.has_network_waiters() => NextStep::WaitNetwork(
                    exec.timers
                        .peek_deadline()
                        .map_or(Duration::from_secs(60), |deadline| {
                            deadline
                                .saturating_duration_since(Instant::now())
                                .min(Duration::from_secs(60))
                        }),
                ),
                None => {
                    let timer_deadline = exec.timers.peek_deadline();
                    let deadline = match (timer_deadline, exec.background_shutdown_deadline) {
                        (Some(timer), Some(drain)) => Some(timer.min(drain)),
                        (Some(timer), None) => Some(timer),
                        (None, Some(drain)) => Some(drain),
                        (None, None) => None,
                    };
                    match deadline {
                        Some(deadline) => NextStep::SleepUntil(deadline),
                        None => NextStep::Unresolvable,
                    }
                }
            }
        });
        match next {
            NextStep::Run(task) => run_one_turn(task),
            NextStep::Done => {
                if let Some(shutdown) = managed_pool_shutdown.take() {
                    shutdown
                        .join()
                        .expect("managed pool shutdown coordinator must not panic");
                }
                break;
            }
            NextStep::DrainExpired(report) => {
                eprintln!(
                    "background shutdown grace expired; unfinished jobs: {:?}; unfinished services: {:?}; other tasks: {:?}",
                    report.jobs, report.services, report.other_tasks
                );
                if terminate_on_deadline {
                    // The process is exiting, so the OS reclaims the address
                    // space only after all native threads stop. Do not unwind
                    // or free GC/TCB memory while a non-cooperative callback
                    // may still hold raw pointers into it.
                    std::process::exit(124);
                }
                // Grace expiry is diagnostic only. Stop new managed callback
                // admission and join accepted callbacks on a helper thread;
                // keep this executor driving safepoints and retain its TCBs
                // and heap until workers have unregistered and callers finish.
                managed_pool_shutdown = Some(begin_managed_pool_shutdown());
            }
            NextStep::StartManagedPoolShutdown => {
                // Even a nominally drained executor must establish that no
                // accepted managed callback can still hold raw ABI pointers.
                managed_pool_shutdown = Some(begin_managed_pool_shutdown());
            }
            // A `parallel` region is running on the worker pool and no I/O
            // branch is ready. Wait for a chunk to join (or a GC request);
            // the 1 ms is only a backstop, `notify_pool_event` wakes us.
            NextStep::WaitPool(timeout) => wait_pool_event(timeout),
            NextStep::WaitNetwork(timeout) => wait_pool_event(timeout),
            NextStep::SleepUntil(deadline) => {
                let now = Instant::now();
                if deadline > now {
                    // OS signal handlers only publish an atomic flag, so the
                    // scheduler must periodically return to a safe boundary
                    // to consume it even while every task is timer-suspended.
                    std::thread::sleep((deadline - now).min(Duration::from_millis(10)));
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

// `optimization_bench` includes this module directly in a lightweight test
// harness that intentionally has no runtime module tree. Production and
// integration builds use the full implementation; unit-only executor runs
// have no managed ABI workers to join.
#[cfg(not(test))]
fn begin_managed_pool_shutdown() -> std::thread::JoinHandle<()> {
    std::thread::spawn(crate::managed_blocking::shutdown_managed_pool)
}

#[cfg(test)]
fn begin_managed_pool_shutdown() -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {})
}

enum NextStep {
    Run(TaskId),
    Done,
    StartManagedPoolShutdown,
    DrainExpired(DrainExpiredReport),
    WaitPool(Duration),
    WaitNetwork(Duration),
    SleepUntil(Instant),
    Unresolvable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DrainExpiredReport {
    jobs: Vec<u64>,
    services: Vec<u64>,
    other_tasks: Vec<u64>,
}

fn service_outcome_message(state: TaskState, outcome: &TaskOutcome) -> String {
    match outcome {
        TaskOutcome::Panicked(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                payload
                    .downcast_ref::<&'static str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_else(|| format!("service callback ended in {state:?}")),
        TaskOutcome::Value(_) => "service callback returned".to_owned(),
    }
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
        exec.started_tasks.insert(task);
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
        let tcb = exec
            .registry
            .get_mut(task)
            .expect("the task cannot vanish while its context is out");
        tcb.context = Some(context);
        let cancel_requested = tcb.cancel_requested;

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
                if cancel_requested {
                    // A shield only defers delivery while cleanup is running.
                    // Normalize it once the context is terminal in case a
                    // cleanup return skipped its matching exit.
                    exec.registry.get_mut(task).unwrap().shield_depth = 0;
                    complete_cancellation(exec, task);
                } else {
                    exec.complete(task, TaskOutcome::Value(value), TaskState::Completed);
                }
            }
            Err(payload) => {
                // A native unwind may bypass generated `zirk_rt_pop_frame`
                // calls. The task is terminal now, so none of its stack
                // slots may remain registered with the collector: those
                // addresses point into frames already unwound. A detached
                // caller is the exception: an accepted managed callback may
                // still hold raw pointers to those slots, so retain its TCB
                // roots until the managed pool has joined at executor exit.
                if !exec.background_tasks.contains_key(&task) {
                    exec.registry.get_mut(task).unwrap().roots.clear();
                }
                if cancel_requested {
                    exec.registry.get_mut(task).unwrap().shield_depth = 0;
                    complete_cancellation(exec, task);
                    return;
                }
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

/// Convert cancellation to its terminal outcome, promoting a recorded
/// cleanup Throwable only for jobs opted in by the first() loser path.
fn complete_cancellation(exec: &mut Executor, task: TaskId) {
    let failure = exec
        .registry
        .get(task)
        .and_then(|tcb| tcb.cancel_cleanup_failure);
    if let Some(exception) = failure {
        exec.complete(
            task,
            TaskOutcome::Panicked(Box::new(BranchFailure(exception))),
            TaskState::Failed,
        );
    } else {
        exec.complete(
            task,
            TaskOutcome::Panicked(Box::new(Cancelled)),
            TaskState::Cancelled,
        );
    }
}

/// Visits the roots in every live task's frame chain — registered with the
/// collector for the duration of a run (design D4). Marking is idempotent, so
/// visiting the running task's chain here as well as through `ACTIVE_FRAMES` is
/// harmless.
fn walk_all_task_roots(mark: &mut dyn FnMut(*mut std::ffi::c_void)) {
    with_exec(|exec| {
        for pending in &exec.pending_background_failures {
            if let Some(throwable) = pending.throwable {
                mark(throwable as *mut std::ffi::c_void);
            }
        }
        for capture in exec
            .background_error_handler
            .iter()
            .map(|handler| handler.capture)
        {
            if capture != 0 {
                mark(capture as *mut std::ffi::c_void);
            }
        }
        for &throwable in exec.background_handler_failures.values() {
            mark(throwable as *mut std::ffi::c_void);
        }
        // A dependency-blocked service has not yet acquired a task control
        // block, so its callable capture remains rooted by this registration.
        for capture in exec
            .service_callbacks
            .values()
            .map(|service| service.capture_root)
            .filter(|capture| !capture.is_null())
        {
            mark(capture);
        }
        for id in exec.registry.live_ids() {
            let tcb = exec.registry.get(id).expect("a live id resolves");
            if let Some(capture) = tcb.capture_root {
                mark(capture);
            }
            if let Some(exception) = tcb.cancel_cleanup_failure {
                mark(exception as *mut std::ffi::c_void);
            }
            if let Some(outcome) = &tcb.outcome {
                mark_task_outcome_root(outcome, mark);
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

/// A completed task can remain joinable while another task triggers a GC.
/// Keep pointer-shaped source results and structured-branch throwables rooted
/// until their outcome is consumed. The collector first checks exact allocation
/// membership, so scalar words and non-managed pointers are never dereferenced.
fn mark_task_outcome_root(outcome: &TaskOutcome, mark: &mut dyn FnMut(*mut std::ffi::c_void)) {
    match outcome {
        TaskOutcome::Value(value) => mark(*value as *mut std::ffi::c_void),
        TaskOutcome::Panicked(payload) => {
            if let Some(failure) = payload.downcast_ref::<BranchFailure>() {
                mark(failure.0 as *mut std::ffi::c_void);
            }
        }
    }
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
    try_spawn(body).expect("executor task capacity exceeded")
}

/// Attempts task admission and reports capacity exhaustion explicitly.
pub fn try_spawn<F>(body: F) -> Result<TaskId, crate::task::TaskAdmissionError>
where
    F: FnOnce() -> usize + Send + 'static,
{
    try_spawn_with_capture_root(body, std::ptr::null_mut())
}

/// Opens a scope in the executor currently running this task.
pub fn scope_enter_current() -> ScopeId {
    with_exec(|exec| exec.scope_enter())
}

/// Opens a collect-all scope in the executor currently running this task.
pub fn scope_enter_collect_all_current() -> ScopeId {
    with_exec(|exec| exec.scope_enter_collect_all())
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
    match with_exec(|exec| {
        let scope_state = exec
            .scopes
            .get(scope)
            .unwrap_or_else(|| fatal("unknown scope"));
        if scope_state.mode != ScopeMode::FailFast {
            fatal("fail-fast scope exit used with a collect-all scope");
        }
        exec.scope_exit(scope)
    }) {
        ScopeExit::Pending => None,
        ScopeExit::Complete {
            primary_failure, ..
        } => Some(primary_failure.and_then(take_branch_failure)),
    }
}

/// Joins a collect-all scope without propagating child failures. Terminal
/// outcomes remain attached to their task handles for explicit collection.
pub fn scope_exit_collect_all_poll(scope: ScopeId) -> Option<()> {
    let caller_cancelled = current_task_cancel_requested();
    match with_exec(|exec| {
        let scope_state = exec
            .scopes
            .get(scope)
            .unwrap_or_else(|| fatal("unknown scope"));
        if scope_state.mode != ScopeMode::CollectAll {
            fatal("collect-all scope exit used with a fail-fast scope");
        }
        if caller_cancelled {
            exec.request_scope_cancel(scope);
        }
        exec.scope_exit(scope)
    }) {
        ScopeExit::Pending => None,
        ScopeExit::Complete { .. } => Some(()),
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

/// Opts a target into cleanup-failure promotion, then requests ordinary
/// cooperative cancellation. Stale or terminal task ids are ignored.
pub fn cancel_with_cleanup_failure_current(branch: TaskId) {
    with_exec(|exec| {
        if let Some(tcb) = exec.registry.get_mut(branch)
            && !tcb.state.is_terminal()
        {
            tcb.promote_cancel_cleanup_failure = true;
        }
        exec.request_cancel(branch);
    });
}

/// Captures a Throwable thrown from opted-in cancellation cleanup. `zirk_rt_throw`
/// calls this while the task's cancellation shield is active.
pub(crate) fn capture_cancel_cleanup_throw(exception: *const std::ffi::c_void) {
    if exception.is_null() {
        return;
    }
    let Some(me) = CURRENT_TASK.get() else { return };
    let exception = exception as usize;
    let prior = with_exec(|exec| {
        let tcb = exec.registry.get_mut(me)?;
        if !tcb.cancel_requested || !tcb.promote_cancel_cleanup_failure || tcb.shield_depth == 0 {
            return None;
        }
        match tcb.cancel_cleanup_failure {
            Some(first) => Some(first),
            None => {
                tcb.cancel_cleanup_failure = Some(exception);
                Some(0)
            }
        }
    });
    if let Some(prior) = prior.filter(|prior| *prior != 0) {
        // SAFETY: both values are non-null Throwable pointers provided by the
        // generated cleanup path; use the runtime's established chain rules.
        unsafe {
            crate::exceptions::zirk_rt_set_suppressed(
                prior as *const std::ffi::c_void,
                exception as *const std::ffi::c_void,
            );
        }
    }
}

/// Spawns generated code and roots its callable capture block for the task's
/// entire lifetime, including before the body installs compiler frame roots.
pub fn spawn_with_capture_root<F>(body: F, capture_root: *mut std::ffi::c_void) -> TaskId
where
    F: FnOnce() -> usize + Send + 'static,
{
    try_spawn_with_capture_root(body, capture_root).expect("executor task capacity exceeded")
}

/// Generated-code variant of [`try_spawn`] that roots its capture block.
pub fn try_spawn_with_capture_root<F>(
    body: F,
    capture_root: *mut std::ffi::c_void,
) -> Result<TaskId, crate::task::TaskAdmissionError>
where
    F: FnOnce() -> usize + Send + 'static,
{
    with_exec(|exec| {
        exec.ensure_task_capacity()?;
        let id = exec.registry.insert_with_capture_root(
            TaskContext::new(
                crate::context::DEFAULT_TASK_STACK_BYTES,
                move |_suspender| body(),
            ),
            (!capture_root.is_null()).then_some(capture_root),
        );
        exec.ready.push_back(id);
        exec.active_tasks += 1;
        Ok(id)
    })
}

/// Atomically reserves task and timer capacity before publishing an ambient
/// Timer.after/every callback to the executor.
pub fn try_spawn_timer_with_capture_root<F>(
    body: F,
    capture_root: *mut std::ffi::c_void,
    delay_nanos: i64,
) -> Result<TaskId, TimerTaskAdmissionError>
where
    F: FnOnce() -> usize + Send + 'static,
{
    if delay_nanos < 0 {
        return Err(TimerTaskAdmissionError::NegativeDuration);
    }
    with_exec(|exec| {
        exec.ensure_task_capacity()
            .map_err(|_| TimerTaskAdmissionError::TaskCapacity)?;
        exec.timers
            .reserve()
            .map_err(|_| TimerTaskAdmissionError::TimerCapacity)?;
        let id = exec.registry.insert_with_capture_root(
            TaskContext::new(
                crate::context::DEFAULT_TASK_STACK_BYTES,
                move |_suspender| body(),
            ),
            (!capture_root.is_null()).then_some(capture_root),
        );
        exec.registry
            .get_mut(id)
            .expect("new timer task is registered")
            .timer_reservation = true;
        exec.ready.push_back(id);
        exec.active_tasks += 1;
        Ok(id)
    })
}

/// Spawns short-lived background work under the executor's bounded app-level
/// supervisor. Admission is reserved before the task becomes runnable. The
/// returned task handle is runtime bookkeeping only; detached language code
/// does not receive a `Job<T>` that could accidentally be joined.
pub fn spawn_background<F>(body: F) -> Result<TaskId, AdmissionError>
where
    F: FnOnce() -> usize + Send + 'static,
{
    spawn_background_with_capture_root(body, std::ptr::null_mut())
}

/// Generated-code variant that keeps the callback's capture block rooted for
/// the complete task lifetime, including before its first stack frame exists.
pub fn spawn_background_with_capture_root<F>(
    body: F,
    capture_root: *mut std::ffi::c_void,
) -> Result<TaskId, AdmissionError>
where
    F: FnOnce() -> usize + Send + 'static,
{
    with_exec(|exec| exec.spawn_background(body, capture_root))
}

/// Registers a runtime-owned service callback. This Rust-only bridge remains
/// intentionally unavailable through the Zirk sema/IR/ABI surface.
pub fn register_service_current<F>(
    name: impl Into<String>,
    required: bool,
    dependencies: &[ServiceId],
    capture_root: *mut std::ffi::c_void,
    callback: F,
) -> Result<ServiceId, ServiceError>
where
    F: FnOnce(ServiceContext) -> usize + Send + 'static,
{
    with_exec(|exec| exec.register_service(name, required, dependencies, capture_root, callback))
}

/// Registers a source-equivalent service after every earlier required service
/// and schedules it as soon as its dependencies are ready.
pub fn register_service_with_prior_required_current<F>(
    name: impl Into<String>,
    required: bool,
    capture_root: *mut std::ffi::c_void,
    callback: F,
) -> Result<ServiceId, ServiceError>
where
    F: FnOnce(ServiceContext) -> usize + Send + 'static,
{
    with_exec(|exec| {
        let dependencies = exec
            .services
            .services()
            .into_iter()
            .filter_map(|(id, _, was_required, _)| was_required.then_some(id))
            .collect::<Vec<_>>();
        let id = exec.register_service(name, required, &dependencies, capture_root, callback)?;
        exec.start_services()?;
        Ok(id)
    })
}

/// Starts registered runtime services in registration order whenever their
/// dependencies are ready.
pub fn start_services_current() -> Result<(), ServiceError> {
    with_exec(Executor::start_services)
}

pub fn shutdown_services_current() -> Result<(), ServiceError> {
    with_exec(|exec| {
        if CURRENT_TASK.get() != exec.root {
            return Err(ServiceError::WrongExecutorTask);
        }
        let order = exec.services.request_shutdown()?;
        exec.service_drain = order.into();
        exec.discard_stopped_service_callbacks();
        exec.cancel_next_service();
        Ok(())
    })
}

pub fn service_failed_current() -> Result<(), ServiceError> {
    let task = CURRENT_TASK.get().ok_or(ServiceError::WrongExecutorTask)?;
    with_exec(|exec| exec.service_failed(task))
}

pub(crate) fn service_ready_current(id: ServiceId) -> Result<ServiceState, ServiceError> {
    with_exec(|exec| exec.service_ready(id))
}

/// Snapshot supervisor diagnostics from the currently running executor.
pub fn background_diagnostics_current() -> Vec<BackgroundDiagnostic> {
    with_exec(|exec| exec.background_diagnostics())
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
    match try_sleep(delay_nanos) {
        Ok(()) => {}
        Err(crate::timer::TimerError::NegativeDuration) => fatal("sleep with a negative duration"),
        Err(crate::timer::TimerError::CapacityExceeded) => {
            fatal("executor timer capacity exceeded in an internal sleep")
        }
    }
}

/// Sets the currently executing program's background shutdown grace in
/// nanoseconds. Returns false for negative values.
pub fn set_background_shutdown_grace(delay_nanos: i64) -> bool {
    let Ok(nanos) = u64::try_from(delay_nanos) else {
        return false;
    };
    let updated = Executor::set_current_background_shutdown_grace(Duration::from_nanos(nanos));
    if updated {
        crate::host_signal::set_shutdown_grace_nanos(nanos);
    }
    updated
}

pub(crate) fn set_current_managed_background_error_handler(handler: ManagedBackgroundErrorHandler) {
    Executor::set_current_managed_background_error_handler(handler);
}

pub(crate) fn report_current_background_handler_failure(throwable: usize) {
    Executor::report_current_background_handler_failure(throwable);
}

/// Attempts to suspend the current task until a monotonic deadline. Capacity
/// exhaustion is returned before the task is parked.
pub fn try_sleep(delay_nanos: i64) -> Result<(), crate::timer::TimerError> {
    let me = CURRENT_TASK.get().expect("sleep outside a task body");
    // Do not park a task whose cancellation has already been requested. The
    // generated caller observes the cancellation predicate after this call.
    // Cleanup shields defer delivery, but must not make cleanup block on a
    // timer after the owning task was cancelled.
    if current_task_cancel_requested() {
        return Ok(());
    }
    let timer = with_exec(|exec| {
        let reserved = exec
            .registry
            .get(me)
            .is_some_and(|task| task.timer_reservation);
        if reserved {
            exec.timers.arm_reserved(delay_nanos, me.to_bits())
        } else {
            exec.timers.try_arm(delay_nanos, me.to_bits())
        }
    })?;
    suspend_current(WaitReason::Timer(timer.to_bits()));
    Ok(())
}

/// Compatibility safe point. Cancellation is observed as state by generated
/// cleanup code and delivered when a task returns; it must not unwind through
/// language frames, since that would skip `finally` and resource cleanup.
pub fn check_cancelled() {
    let _ = current_task_is_cancelled();
}

/// Whether the currently running task has a deliverable cancellation request.
/// This is a side-effect-free query for generated cleanup code; it does not
/// deliver cancellation or mutate task state.
pub(crate) fn current_task_is_cancelled() -> bool {
    let Some(me) = CURRENT_TASK.get() else {
        return false;
    };
    with_exec(|exec| {
        exec.registry
            .get(me)
            .is_some_and(|tcb| tcb.cancel_requested && tcb.shield_depth == 0)
    })
}

/// Whether the active task has any cancellation request, ignoring cleanup
/// shields. Blocking runtime operations use this to return promptly during
/// cleanup; generated code checks the shielded query before delivering it.
pub(crate) fn current_task_cancel_requested() -> bool {
    let Some(me) = CURRENT_TASK.get() else {
        return false;
    };
    with_exec(|exec| {
        exec.registry
            .get(me)
            .is_some_and(|tcb| tcb.cancel_requested)
    })
}

/// Whether the current OS thread is presently executing one of this
/// executor's cooperative task stacks.
pub(crate) fn in_executor_task() -> bool {
    CURRENT_TASK.get().is_some()
}

/// Temporarily defers delivery while cancellation cleanup runs. Calls outside
/// an executor task are harmless, allowing generated cleanup helpers to share
/// the same runtime ABI in synchronous contexts.
pub(crate) fn enter_cancellation_shield() {
    let Some(me) = CURRENT_TASK.get() else { return };
    with_exec(|exec| {
        let tcb = exec.registry.get_mut(me).expect("the current task");
        tcb.shield_depth = tcb
            .shield_depth
            .checked_add(1)
            .expect("cancellation shield nesting overflow");
    });
}

/// Leaves one temporary cleanup shield. Unbalanced exits are runtime bugs.
pub(crate) fn exit_cancellation_shield() {
    let Some(me) = CURRENT_TASK.get() else { return };
    with_exec(|exec| {
        let tcb = exec.registry.get_mut(me).expect("the current task");
        tcb.shield_depth = tcb
            .shield_depth
            .checked_sub(1)
            .expect("unbalanced cancellation shield exit");
    });
}

/// Runs bounded, non-suspending cleanup while deferring cancellation delivery.
///
/// The compiler must reject suspension effects in `cleanup`; this runtime
/// helper only guarantees that the shield depth is restored if the closure
/// returns or unwinds. Blocking runtime operations continue to observe the raw
/// cancellation request, so this does not make waits cancellation-proof.
pub(crate) fn with_cancellation_shield<R>(cleanup: impl FnOnce() -> R) -> R {
    enter_cancellation_shield();
    let _guard = CancellationShieldGuard;
    cleanup()
}

struct CancellationShieldGuard;

impl Drop for CancellationShieldGuard {
    fn drop(&mut self) {
        exit_cancellation_shield();
    }
}

/// Identifies the branch currently executing a runtime operation.
pub(crate) fn current_task() -> TaskId {
    CURRENT_TASK
        .get()
        .expect("channel operation outside a task body")
}

/// Wakes a task parked by a channel operation. Stale task ids are harmless:
/// registries validate generations before changing a slot.
pub(crate) fn wake_task(task: TaskId) {
    with_exec(|exec| exec.make_ready(task));
}

/// Whether a parked task was cancelled. Channel waiter queues use this to skip
/// an entry whose owner was made ready to observe cancellation.
pub(crate) fn task_cancel_requested(task: TaskId) -> bool {
    with_exec(|exec| {
        exec.registry
            .get(task)
            .is_some_and(|tcb| tcb.cancel_requested)
    })
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

/// Transfers a terminal Zirk task outcome into caller-owned, already-rooted
/// output slots. Native panic payloads are executor failures and are never
/// translated into language-level rejections.
pub(crate) fn job_outcome_take_current(
    target: TaskId,
    value_out: *mut *mut std::ffi::c_void,
    throwable_out: *mut *mut std::ffi::c_void,
) -> crate::task::JobOutcomeStatus {
    assert!(
        in_task_context(),
        "job outcome take outside an executor task"
    );
    let me = CURRENT_TASK.get().expect("task context has a task id");
    if value_out.is_null()
        || throwable_out.is_null()
        || value_out == throwable_out
        || !(value_out as usize).is_multiple_of(std::mem::align_of::<*mut std::ffi::c_void>())
        || !(throwable_out as usize).is_multiple_of(std::mem::align_of::<*mut std::ffi::c_void>())
    {
        fatal("job outcome take received invalid output slots");
    }

    with_exec(|exec| {
        let caller = exec
            .registry
            .get(me)
            .expect("the current task remains registered");
        let mut value_rooted = false;
        let mut throwable_rooted = false;
        for frame in &caller.roots {
            for index in 0..frame.count {
                // SAFETY: task frame root arrays are installed by
                // zirk_rt_push_frame and remain live until their paired pop.
                let root_slot = unsafe { *frame.roots.add(index) };
                value_rooted |= root_slot == value_out.cast();
                throwable_rooted |= root_slot == throwable_out.cast();
            }
        }
        if !value_rooted || !throwable_rooted {
            fatal("job outcome take output slots are not registered GC roots");
        }

        if target == me {
            fatal("job outcome take cannot consume the current task");
        }
        let Some(tcb) = exec.registry.get_mut(target) else {
            fatal("job outcome take received a task ID that is not live");
        };
        if !tcb.state.is_terminal() {
            fatal("job outcome take requires a terminal task");
        }
        if tcb.result_consumed || tcb.waiter.is_some() {
            fatal("job outcome take received an already-consumed task");
        }

        let (status, value, throwable) = match (tcb.state, tcb.outcome.as_ref()) {
            (TaskState::Completed, Some(TaskOutcome::Value(value))) => (
                crate::task::JobOutcomeStatus::Fulfilled,
                *value as *mut std::ffi::c_void,
                std::ptr::null_mut(),
            ),
            (TaskState::Failed, Some(TaskOutcome::Panicked(payload))) => {
                if let Some(failure) = payload.downcast_ref::<BranchFailure>() {
                    if failure.0 == 0 {
                        fatal("job outcome take encountered a rejection without a Throwable");
                    }
                    (
                        crate::task::JobOutcomeStatus::Rejected,
                        std::ptr::null_mut(),
                        failure.0 as *mut std::ffi::c_void,
                    )
                } else {
                    fatal("job outcome take encountered an internal native panic");
                }
            }
            (TaskState::Cancelled, Some(TaskOutcome::Panicked(payload)))
                if payload.is::<Cancelled>() =>
            {
                (
                    crate::task::JobOutcomeStatus::Cancelled,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            }
            _ => fatal("job outcome take found an inconsistent terminal task outcome"),
        };

        // The caller's two slots were verified as roots above. Publish the
        // transferred pointer before dropping the TCB outcome that rooted it.
        unsafe {
            *value_out = value;
            *throwable_out = throwable;
        }
        tcb.result_consumed = true;
        tcb.outcome.take();
        status
    })
}

/// Waits cooperatively for `target` and transfers its outcome into the caller's
/// already-rooted output slots. Caller cancellation returns a distinct status
/// and leaves the target unconsumed.
pub(crate) fn job_outcome_wait_take_current(
    target: TaskId,
    value_out: *mut *mut std::ffi::c_void,
    throwable_out: *mut *mut std::ffi::c_void,
) -> crate::task::JobOutcomeStatus {
    let me = CURRENT_TASK
        .get()
        .expect("job outcome wait outside an executor task");
    assert!(
        in_task_context(),
        "job outcome wait outside an executor task"
    );

    // Validate and claim the result before parking. This prevents another
    // waiter from consuming it while this caller's stack is suspended.
    let ready_now = with_exec(|exec| {
        let caller = exec
            .registry
            .get(me)
            .expect("the current task remains registered");
        let (mut value_rooted, mut throwable_rooted) = (false, false);
        for frame in &caller.roots {
            for index in 0..frame.count {
                // SAFETY: root arrays are installed by zirk_rt_push_frame.
                let slot = unsafe { *frame.roots.add(index) };
                value_rooted |= slot == value_out.cast();
                throwable_rooted |= slot == throwable_out.cast();
            }
        }
        let cancelled = caller.cancel_requested;
        // The consuming operation validates both slots independently. Here
        // reject malformed pointers before any possible suspension as well.
        if value_out.is_null()
            || throwable_out.is_null()
            || value_out == throwable_out
            || !(value_out as usize).is_multiple_of(std::mem::align_of::<*mut std::ffi::c_void>())
            || !(throwable_out as usize)
                .is_multiple_of(std::mem::align_of::<*mut std::ffi::c_void>())
            || !value_rooted
            || !throwable_rooted
        {
            fatal("job outcome wait received invalid or unrooted output slots");
        }
        // The slots are precise roots, so clear any previous contents before
        // returning for caller cancellation or entering a suspension point.
        unsafe {
            *value_out = std::ptr::null_mut();
            *throwable_out = std::ptr::null_mut();
        }
        if target == me {
            fatal("job outcome wait cannot consume the current task");
        }
        if cancelled {
            return None;
        }
        let Some(tcb) = exec.registry.get_mut(target) else {
            fatal("job outcome wait received a task ID that is not live");
        };
        if tcb.result_consumed || tcb.waiter.is_some() {
            fatal("job outcome wait received an already-consumed task");
        }
        if tcb.state.is_terminal() {
            Some(true)
        } else {
            tcb.waiter = Some(me);
            Some(false)
        }
    });

    let Some(ready_now) = ready_now else {
        return crate::task::JobOutcomeStatus::CallerCancelled;
    };
    if !ready_now {
        suspend_current(WaitReason::AwaitingTask(target));
        let cancelled = current_task_cancel_requested();
        if cancelled {
            with_exec(|exec| {
                if let Some(tcb) = exec.registry.get_mut(target)
                    && tcb.waiter == Some(me)
                {
                    tcb.waiter = None;
                }
            });
            return crate::task::JobOutcomeStatus::CallerCancelled;
        }
    }

    // Completion removes the reservation before waking this caller. The
    // immediate-ready path has no reservation to clear.
    job_outcome_take_current(target, value_out, throwable_out)
}

/// Waits for and consumes the earliest-completing unconsumed child in a
/// collect-all scope. The child's registration ordinal is returned so a
/// bounded dynamic combinator can place its payload into input-order storage.
/// Caller cancellation leaves every child result unconsumed.
pub(crate) fn scope_outcome_wait_take_current(
    scope: ScopeId,
    ordinal_out: *mut u64,
    value_out: *mut *mut std::ffi::c_void,
    throwable_out: *mut *mut std::ffi::c_void,
) -> crate::task::ScopeOutcomeStatus {
    use crate::task::ScopeOutcomeStatus as Status;

    assert!(
        in_task_context(),
        "scope outcome wait outside an executor task"
    );
    let me = CURRENT_TASK
        .get()
        .expect("scope outcome wait has a current task");
    enum Selection {
        Pending(Vec<TaskId>),
        Ready(usize, TaskId),
        CallerCancelled,
        Drained,
    }

    fn overlaps(left: usize, left_size: usize, right: usize, right_size: usize) -> bool {
        let Some(left_end) = left.checked_add(left_size) else {
            return true;
        };
        let Some(right_end) = right.checked_add(right_size) else {
            return true;
        };
        left < right_end && right < left_end
    }

    loop {
        let selection = with_exec(|exec| {
            let scope_tcb = exec
                .scopes
                .get(scope)
                .unwrap_or_else(|| fatal("scope outcome wait received an unknown scope"));
            if scope_tcb.mode != ScopeMode::CollectAll {
                fatal("scope outcome wait requires a collect-all scope");
            }
            if scope_tcb.branches.len() > 256 {
                fatal("scope outcome wait exceeds the 256-child safety bound");
            }

            if ordinal_out.is_null()
                || value_out.is_null()
                || throwable_out.is_null()
                || value_out == throwable_out
                || ordinal_out.cast::<std::ffi::c_void>() == value_out.cast()
                || ordinal_out.cast::<std::ffi::c_void>() == throwable_out.cast()
                || !(ordinal_out as usize).is_multiple_of(std::mem::align_of::<u64>())
                || !(value_out as usize)
                    .is_multiple_of(std::mem::align_of::<*mut std::ffi::c_void>())
                || !(throwable_out as usize)
                    .is_multiple_of(std::mem::align_of::<*mut std::ffi::c_void>())
                || overlaps(
                    ordinal_out as usize,
                    std::mem::size_of::<u64>(),
                    value_out as usize,
                    std::mem::size_of::<*mut std::ffi::c_void>(),
                )
                || overlaps(
                    ordinal_out as usize,
                    std::mem::size_of::<u64>(),
                    throwable_out as usize,
                    std::mem::size_of::<*mut std::ffi::c_void>(),
                )
            {
                fatal("scope outcome wait received invalid output slots");
            }

            let caller = exec
                .registry
                .get(me)
                .expect("the current task remains registered");
            let (mut value_rooted, mut throwable_rooted) = (false, false);
            for frame in &caller.roots {
                for index in 0..frame.count {
                    // SAFETY: frames are installed by the generated root ABI
                    // and remain live while this task is running or parked.
                    let slot = unsafe { *frame.roots.add(index) };
                    value_rooted |= slot == value_out.cast();
                    throwable_rooted |= slot == throwable_out.cast();
                }
            }
            if !value_rooted || !throwable_rooted {
                fatal("scope outcome wait output slots are not registered GC roots");
            }
            // Clear old contents before any possible suspension. Both slots
            // are precise roots for the entire operation.
            unsafe {
                *value_out = std::ptr::null_mut();
                *throwable_out = std::ptr::null_mut();
            }
            if caller.cancel_requested {
                return Selection::CallerCancelled;
            }

            let branches = &scope_tcb.branches;
            let mut best: Option<(usize, TaskId, u64)> = None;
            let mut pending = Vec::new();
            for (ordinal, target) in branches.iter().copied().enumerate() {
                let Some(tcb) = exec.registry.get(target) else {
                    // A missing child can only have been reclaimed after its
                    // result was consumed; scope records retain registration
                    // order until scope close.
                    continue;
                };
                if tcb.parent != Some(scope) {
                    fatal("scope outcome wait found a child owned by another scope");
                }
                if tcb.result_consumed {
                    continue;
                }
                if tcb.waiter.is_some() {
                    fatal("scope outcome wait found a child already claimed by another waiter");
                }
                if tcb.state.is_terminal() {
                    let sequence = tcb.completion_sequence.unwrap_or_else(|| {
                        fatal("terminal scope child has no completion sequence")
                    });
                    if best.is_none_or(|(_, _, earliest)| sequence < earliest) {
                        best = Some((ordinal, target, sequence));
                    }
                } else {
                    pending.push(target);
                }
            }
            if let Some((ordinal, target, _)) = best {
                return Selection::Ready(ordinal, target);
            }
            if pending.is_empty() {
                Selection::Drained
            } else {
                Selection::Pending(pending)
            }
        });

        match selection {
            Selection::CallerCancelled => return Status::CallerCancelled,
            Selection::Drained => {
                unsafe { *ordinal_out = u64::MAX };
                return Status::Drained;
            }
            Selection::Pending(targets) => {
                wait_any_current(&targets);
                // Recheck cancellation under the executor lock before taking
                // a result, matching job_outcome_wait_take's precedence.
                if current_task_cancel_requested() {
                    return Status::CallerCancelled;
                }
            }
            Selection::Ready(ordinal, target) => {
                let taken = with_exec(|exec| {
                    let caller = exec
                        .registry
                        .get(me)
                        .expect("the current task remains registered");
                    if caller.cancel_requested {
                        return Some(Status::CallerCancelled);
                    }
                    let Some(tcb) = exec.registry.get_mut(target) else {
                        // Another collector may have consumed and reclaimed
                        // this child after selection released the executor
                        // lock. Restart the scan and select the next result.
                        return None;
                    };
                    if tcb.result_consumed {
                        return None;
                    }
                    if !tcb.state.is_terminal() || tcb.waiter.is_some() {
                        fatal("selected scope child changed before outcome transfer");
                    }
                    let (status, value, throwable) = match (tcb.state, tcb.outcome.as_ref()) {
                        (TaskState::Completed, Some(TaskOutcome::Value(value))) => (
                            Status::Fulfilled,
                            *value as *mut std::ffi::c_void,
                            std::ptr::null_mut(),
                        ),
                        (TaskState::Failed, Some(TaskOutcome::Panicked(payload))) => {
                            let Some(failure) = payload.downcast_ref::<BranchFailure>() else {
                                fatal("scope outcome wait encountered an internal native panic");
                            };
                            if failure.0 == 0 {
                                fatal("scope outcome wait encountered a null Throwable");
                            }
                            (
                                Status::Rejected,
                                std::ptr::null_mut(),
                                failure.0 as *mut std::ffi::c_void,
                            )
                        }
                        (TaskState::Cancelled, Some(TaskOutcome::Panicked(payload)))
                            if payload.is::<Cancelled>() =>
                        {
                            (
                                Status::Cancelled,
                                std::ptr::null_mut(),
                                std::ptr::null_mut(),
                            )
                        }
                        _ => fatal("scope outcome wait found an inconsistent child outcome"),
                    };
                    // The destination roots were checked before selection.
                    // Publish before dropping the TCB's source root.
                    unsafe {
                        *ordinal_out = ordinal as u64;
                        *value_out = value;
                        *throwable_out = throwable;
                    }
                    tcb.result_consumed = true;
                    tcb.outcome.take();
                    Some(status)
                });
                let Some(status) = taken else {
                    continue;
                };
                if status == Status::CallerCancelled {
                    return status;
                }
                return status;
            }
        }
    }
}

/// Requests cancellation of every branch in a live structured scope.
pub fn scope_cancel_current(scope: ScopeId) {
    with_exec(|exec| exec.request_scope_cancel(scope));
}

/// Counts executor-owned task and `AwaitingAny` registrations for runtime
/// churn regressions. Channel and timer registrations are owned by their
/// respective services and are intentionally outside this test-only count.
#[cfg(test)]
pub(crate) fn waiter_registration_count_for_test() -> usize {
    with_exec(|exec| exec.waiter_registration_count())
}

/// Counts live task control blocks for combinator churn regressions.
#[cfg(test)]
pub(crate) fn task_count_for_test() -> usize {
    with_exec(|exec| exec.task_count())
}

/// Counts timers owned by the current executor for combinator cleanup tests.
#[cfg(test)]
pub(crate) fn timer_registration_count_for_test() -> usize {
    with_exec(|exec| exec.timers.active_registrations())
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

/// Waits cooperatively until at least one target completes. Targets remain
/// registered by their outstanding join handles, so callers can inspect the
/// serialized completion order after waking.
pub(crate) fn wait_any_current(targets: &[TaskId]) {
    assert!(in_task_context(), "wait_any outside an executor task");
    assert!(!targets.is_empty(), "wait_any requires at least one target");
    let me = CURRENT_TASK.get().expect("task context has a task id");
    let ready_now = with_exec(|exec| {
        if exec
            .registry
            .get(me)
            .is_some_and(|tcb| tcb.cancel_requested && tcb.shield_depth == 0)
        {
            return true;
        }
        if targets.iter().any(|target| {
            exec.registry
                .get(*target)
                .is_some_and(|tcb| tcb.state.is_terminal())
        }) {
            return true;
        }
        exec.remove_any_waiter(me);
        for target in targets {
            exec.any_waiters.entry(*target).or_default().push(me);
        }
        let tcb = exec
            .registry
            .get_mut(me)
            .expect("current task is registered");
        tcb.wait = WaitReason::AwaitingAny;
        tcb.state = TaskState::Suspended;
        false
    });
    if !ready_now {
        suspend_current(WaitReason::AwaitingAny);
    }
}

/// Validates every `JobWaitAny` target before registering a waiter. Unlike
/// ordinary stale internal wakeups, invalid handles at this ABI boundary are
/// programming errors: returning from this check guarantees the subsequent
/// wait has at least one live, independently joinable target.
pub(crate) fn validate_wait_any_current(targets: &[TaskId]) {
    assert!(in_task_context(), "wait_any outside an executor task");
    let me = CURRENT_TASK.get().expect("task context has a task id");
    with_exec(|exec| {
        for (index, target) in targets.iter().copied().enumerate() {
            if target == me {
                fatal("wait-any cannot wait on the current task");
            }
            if targets[..index].contains(&target) {
                fatal("wait-any received duplicate task IDs");
            }
            let Some(tcb) = exec.registry.get(target) else {
                fatal("wait-any received a task ID that is not live");
            };
            if tcb.result_consumed || tcb.waiter.is_some() {
                fatal("wait-any received a task whose result is already awaited");
            }
        }
    });
}

pub(crate) fn completion_sequence(target: TaskId) -> Option<u64> {
    with_exec(|exec| {
        exec.registry
            .get(target)
            .and_then(|tcb| tcb.completion_sequence)
    })
}

/// Chooses the earliest terminal completion and uses its input position as a
/// deterministic fallback when completion sequences tie.
pub(crate) fn select_first_completion_index(
    completions: impl IntoIterator<Item = (usize, u64)>,
) -> Option<usize> {
    completions
        .into_iter()
        .min_by_key(|&(index, sequence)| (sequence, index))
        .map(|(index, _)| index)
}

pub(crate) fn task_was_cancelled(target: TaskId) -> bool {
    with_exec(|exec| {
        exec.registry
            .get(target)
            .is_some_and(|tcb| tcb.state == TaskState::Cancelled)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_waiter_registry_is_bounded_and_deduplicates_registration() {
        let mut waiters = NetworkWaiters::default();
        let first = (7, TaskId::from_bits(1));
        let second = (7, TaskId::from_bits(2));
        assert_eq!(waiters.register(first, 1), Ok(()));
        assert_eq!(waiters.register(first, 1), Ok(()));
        assert_eq!(waiters.register(second, 1), Err(()));
        waiters.notify(first, 1);
        waiters.notify(first, 1);
        assert_eq!(waiters.take_wakeups(), vec![first]);
        assert_eq!(waiters.register(second, 1), Ok(()));
    }

    #[test]
    fn network_waiter_cancellation_removes_pending_and_queued_wakeups() {
        let mut waiters = NetworkWaiters::default();
        let target = (3, TaskId::from_bits(9));
        waiters.register(target, 1).unwrap();
        waiters.notify(target, 1);
        waiters.unregister(target);
        assert!(waiters.waiting.is_empty());
        assert!(waiters.take_wakeups().is_empty());
    }

    #[test]
    fn reactor_cancellation_wakes_only_the_requested_executor_task() {
        let observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let child_observed = Arc::clone(&observed);
        run(move || {
            let (parked_tx, parked_rx) = std::sync::mpsc::channel();
            let child = spawn(move || {
                parked_tx.send(()).unwrap();
                sleep(10_000_000_000);
                child_observed.store(current_task_cancel_requested(), Ordering::Release);
                0
            });
            let executor_id = current_executor_id();
            let canceller = std::thread::spawn(move || {
                parked_rx.recv().unwrap();
                std::thread::sleep(Duration::from_millis(10));
                assert!(request_external_cancel(executor_id, child));
            });
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| await_task(child)));
            canceller.join().unwrap();
            assert!(outcome.is_err(), "cancelled task must not return a value");
            assert!(observed.load(Ordering::Acquire));
            0
        });
    }

    #[test]
    fn equal_completion_sequences_choose_the_lowest_input_index() {
        assert_eq!(
            select_first_completion_index([(2, 7), (1, 7), (0, 7)]),
            Some(0)
        );
        assert_eq!(
            select_first_completion_index([(0, 9), (1, 8), (2, 10)]),
            Some(1)
        );
    }
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn run<F: FnOnce() -> usize + Send + 'static>(body: F) -> usize {
        match Executor::new().run_with_root(body) {
            TaskOutcome::Value(v) => v,
            TaskOutcome::Panicked(p) => std::panic::resume_unwind(p),
        }
    }

    #[test]
    fn task_quota_rejects_4097_and_reuses_capacity_after_terminal_completion() {
        let result = run(|| {
            for _ in 0..(crate::task::MAX_ACTIVE_TASKS - 1) {
                try_spawn(|| 1).unwrap();
            }
            assert_eq!(
                try_spawn(|| 2),
                Err(crate::task::TaskAdmissionError::CapacityExceeded)
            );
            // Let all 4095 children reach terminal state. Their outcomes may
            // remain registered temporarily, but their active quota is free.
            yield_now();
            assert!(try_spawn(|| 3).is_ok());
            0
        });
        assert_eq!(result, 0);
    }

    #[test]
    fn background_admission_distinguishes_executor_task_capacity() {
        let result = run(|| {
            for _ in 0..(crate::task::MAX_ACTIVE_TASKS - 1) {
                try_spawn(|| 0).unwrap();
            }
            assert_eq!(
                spawn_background(|| 0),
                Err(AdmissionError::TaskCapacityReached)
            );
            0
        });
        assert_eq!(result, 0);
    }

    #[test]
    fn timer_quota_rejection_does_not_publish_a_task_or_leak_either_permit() {
        let result = run(|| {
            let timer_limit = crate::timer::MAX_ACTIVE_TIMERS;
            with_exec(|exec| {
                for _ in 0..timer_limit {
                    exec.timers.reserve().unwrap();
                }
            });

            assert_eq!(
                try_spawn_timer_with_capture_root(|| 7, std::ptr::null_mut(), 0),
                Err(TimerTaskAdmissionError::TimerCapacity)
            );
            with_exec(|exec| {
                assert_eq!(exec.active_tasks, 1, "rejection must not reserve a task");
                assert_eq!(
                    exec.registry.live_ids().count(),
                    1,
                    "rejection must not register a task control block"
                );
                assert!(
                    exec.ready.is_empty(),
                    "rejection must not enqueue a callback"
                );
                for _ in 0..timer_limit {
                    exec.timers.release_reservation();
                }
            });

            assert!(try_spawn_timer_with_capture_root(|| 7, std::ptr::null_mut(), 0).is_ok());
            0
        });
        assert_eq!(result, 0);
    }

    #[test]
    fn terminal_success_failure_and_cancellation_release_quota_with_outcome_retained() {
        for (outcome, state) in [
            (TaskOutcome::Value(7), TaskState::Completed),
            (
                TaskOutcome::Panicked(Box::new("failure")),
                TaskState::Failed,
            ),
            (
                TaskOutcome::Panicked(Box::new(Cancelled)),
                TaskState::Cancelled,
            ),
        ] {
            let mut exec = Executor::new();
            let task = exec.registry.insert(dormant_context());
            exec.timers.reserve().unwrap();
            exec.registry.get_mut(task).unwrap().timer_reservation = true;
            exec.active_tasks = 1;
            exec.complete(task, outcome, state);
            assert_eq!(exec.active_tasks, 0);
            assert!(exec.registry.get(task).unwrap().outcome.is_some());
            // Repeating terminalization cannot double-release the slot.
            exec.complete(task, TaskOutcome::Value(0), state);
            assert_eq!(exec.active_tasks, 0);
            assert!(exec.timers.try_arm(0, 0).is_ok());
        }
    }

    fn dormant_context() -> TaskContext {
        crate::context::spawn_default(|_s| 0)
    }

    #[test]
    fn task_outcome_root_walker_traces_pointer_values_and_branch_throwables() {
        let pointer_value = 0x1234usize;
        let pointer_error = 0x5678usize;
        let mut roots = Vec::new();
        let mut mark = |root| roots.push(root as usize);

        mark_task_outcome_root(&TaskOutcome::Value(pointer_value), &mut mark);
        mark_task_outcome_root(
            &TaskOutcome::Panicked(Box::new(BranchFailure(pointer_error))),
            &mut mark,
        );
        mark_task_outcome_root(&TaskOutcome::Panicked(Box::new(Cancelled)), &mut mark);

        assert_eq!(roots, [pointer_value, pointer_error]);
    }

    #[test]
    fn opted_in_cancel_promotes_cleanup_failures_and_roots_suppressed_chain() {
        let _collector_guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();

        run(|| {
            let first = unsafe {
                crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) as usize
            };
            let second = unsafe {
                crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) as usize
            };
            let ordinary = unsafe {
                crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) as usize
            };

            let promoted = spawn(move || {
                let _ = std::panic::catch_unwind(yield_now);
                with_cancellation_shield(|| {
                    unsafe {
                        crate::exceptions::zirk_rt_throw(first as *const std::ffi::c_void);
                        assert_eq!(
                            crate::exceptions::zirk_rt_take_pending_exception(),
                            first as *const std::ffi::c_void
                        );
                        crate::exceptions::zirk_rt_throw(second as *const std::ffi::c_void);
                        assert_eq!(
                            crate::exceptions::zirk_rt_take_pending_exception(),
                            second as *const std::ffi::c_void
                        );
                    }
                    crate::collector::collect_for_tests();
                    assert!(crate::collector::is_live_for_tests(
                        first as *mut std::ffi::c_void
                    ));
                    assert!(crate::collector::is_live_for_tests(
                        second as *mut std::ffi::c_void
                    ));
                });
                std::panic::panic_any(Cancelled);
            });
            let cancelled = spawn(move || {
                let _ = std::panic::catch_unwind(yield_now);
                // Recording is deliberately inert for ordinary cancellation.
                with_cancellation_shield(|| unsafe {
                    crate::exceptions::zirk_rt_throw(ordinary as *const std::ffi::c_void);
                    assert_eq!(
                        crate::exceptions::zirk_rt_take_pending_exception(),
                        ordinary as *const std::ffi::c_void
                    );
                });
                std::panic::panic_any(Cancelled);
            });

            yield_now();
            cancel_with_cleanup_failure_current(promoted);
            cancel_current(cancelled);
            yield_now();

            with_exec(|exec| {
                let promoted_tcb = exec.registry.get(promoted).unwrap();
                assert_eq!(promoted_tcb.state, TaskState::Failed);
                let Some(TaskOutcome::Panicked(payload)) = &promoted_tcb.outcome else {
                    panic!("promoted cancellation must retain a failed outcome");
                };
                assert_eq!(payload.downcast_ref::<BranchFailure>().unwrap().0, first);
                assert_eq!(promoted_tcb.cancel_cleanup_failure, None);
                let ordinary_tcb = exec.registry.get(cancelled).unwrap();
                assert_eq!(ordinary_tcb.state, TaskState::Cancelled);
                assert_eq!(ordinary_tcb.cancel_cleanup_failure, None);
            });
            assert_eq!(
                crate::exceptions::zirk_rt_suppressed(first as *const std::ffi::c_void),
                second as *mut std::ffi::c_void
            );

            crate::collector::collect_for_tests();
            assert!(crate::collector::is_live_for_tests(
                first as *mut std::ffi::c_void
            ));
            assert!(crate::collector::is_live_for_tests(
                second as *mut std::ffi::c_void
            ));
            assert!(!crate::collector::is_live_for_tests(
                ordinary as *mut std::ffi::c_void
            ));
            0
        });

        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn completed_job_managed_result_survives_collection_until_consumed() {
        let _collector_guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();

        let result = run(move || {
            let object =
                unsafe { crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) };
            let object_bits = object as usize;
            let job = spawn(move || object_bits);
            yield_now();
            let object = object_bits as *mut std::ffi::c_void;
            assert!(crate::collector::is_live_for_tests(object));
            crate::collector::collect_for_tests();
            assert!(
                crate::collector::is_live_for_tests(object),
                "the unconsumed completed Job result must stay rooted"
            );
            assert_eq!(await_task(job), object as usize);
            0
        });

        assert_eq!(result, 0);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn branch_failure_throwable_survives_collection_while_scope_waits() {
        let _collector_guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();

        let result = run(move || {
            let object =
                unsafe { crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) };
            let object_bits = object as usize;
            let job = spawn(move || {
                std::panic::panic_any(BranchFailure(object_bits));
            });
            yield_now();
            let object = object_bits as *mut std::ffi::c_void;
            assert!(crate::collector::is_live_for_tests(object));
            crate::collector::collect_for_tests();
            assert!(
                crate::collector::is_live_for_tests(object),
                "the unconsumed branch throwable must stay rooted"
            );
            let failure = std::panic::catch_unwind(|| await_task(job));
            assert!(failure.is_err());
            0
        });

        assert_eq!(result, 0);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn active_background_capture_root_survives_explicit_collection() {
        let _collector_guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();

        let result = run(move || {
            let object =
                unsafe { crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) };
            let child = object as usize;
            let task = spawn_background_with_capture_root(
                move || {
                    crate::collector::collect_for_tests();
                    assert!(
                        crate::collector::is_live_for_tests(child as *mut std::ffi::c_void),
                        "the collector must trace the captured background root"
                    );
                    0
                },
                object,
            )
            .expect("background task admission");
            yield_now();
            assert!(is_done(task));
            0
        });

        assert_eq!(result, 0);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn unwound_task_drops_shadow_frames_before_it_becomes_collectable() {
        let result = run(move || {
            let task = spawn(|| {
                let mut local = std::ptr::null_mut::<std::ffi::c_void>();
                let mut roots = [&mut local as *mut _ as *mut std::ffi::c_void];
                unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), 1) };
                panic!("unexpected native unwind");
            });
            yield_now();
            assert!(std::panic::catch_unwind(|| await_task(task)).is_err());
            with_exec(|exec| {
                assert!(exec.registry.get(task).unwrap().roots.is_empty());
            });
            0
        });
        assert_eq!(result, 0);
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
        run(|| {
            let baseline = timer_registration_count_for_test();
            let task_baseline = task_count_for_test();
            let child = spawn(|| {
                sleep(1_000_000_000);
                0
            });
            yield_now(); // let the child arm its timer and suspend
            assert_eq!(timer_registration_count_for_test(), baseline + 1);
            with_exec(|exec| exec.request_cancel(child));
            let cancelled =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| await_task(child)));
            assert!(cancelled.is_err());
            assert_eq!(timer_registration_count_for_test(), baseline);
            assert_eq!(task_count_for_test(), task_baseline);
            0
        });
    }

    #[test]
    fn wait_any_wakes_all_registered_waiters_and_orders_completions() {
        assert_eq!(
            run(|| {
                let first = spawn(|| {
                    yield_now();
                    1
                });
                let second = spawn(|| {
                    yield_now();
                    2
                });
                let wakes = Arc::new(AtomicUsize::new(0));
                let waiters = (0..2)
                    .map(|_| {
                        let wakes = Arc::clone(&wakes);
                        spawn(move || {
                            wait_any_current(&[first, second]);
                            wakes.fetch_add(1, Ordering::SeqCst);
                            0
                        })
                    })
                    .collect::<Vec<_>>();
                for waiter in waiters {
                    await_task(waiter);
                }
                assert_eq!(wakes.load(Ordering::SeqCst), 2);
                assert!(is_done(first) && is_done(second));
                let first_sequence = completion_sequence(first).unwrap();
                let second_sequence = completion_sequence(second).unwrap();
                assert_ne!(first_sequence, second_sequence);
                with_exec(|exec| assert!(exec.any_waiters.is_empty()));
                assert_eq!(waiter_registration_count_for_test(), 0);
                1
            }),
            1
        );
    }

    #[test]
    fn cancelling_wait_any_removes_the_waiter_from_every_target() {
        assert_eq!(
            run(|| {
                let first = spawn(|| {
                    sleep(60_000_000_000);
                    1
                });
                let second = spawn(|| {
                    sleep(60_000_000_000);
                    2
                });
                let waiter = spawn(move || {
                    wait_any_current(&[first, second]);
                    assert!(current_task_is_cancelled());
                    0
                });
                yield_now();
                cancel_current(waiter);
                assert!(std::panic::catch_unwind(|| await_task(waiter)).is_err());
                with_exec(|exec| assert!(exec.any_waiters.is_empty()));
                assert_eq!(waiter_registration_count_for_test(), 0);
                cancel_current(first);
                cancel_current(second);
                for target in [first, second] {
                    assert!(std::panic::catch_unwind(|| await_task(target)).is_err());
                }
                1
            }),
            1
        );
    }

    #[test]
    fn normal_return_delivers_pending_cancellation_without_safe_point_unwind() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                with_exec(|exec| exec.request_cancel(me));
                7
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn cleanup_shield_defers_delivery_until_exit_and_supports_nesting() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                with_exec(|exec| exec.request_cancel(me));
                unsafe { crate::resource::zirk_rt_cancel_shield_enter() };
                unsafe { crate::resource::zirk_rt_cancel_shield_enter() };
                assert!(!current_task_is_cancelled());
                unsafe { crate::resource::zirk_rt_cancel_shield_exit() };
                assert!(!current_task_is_cancelled());
                unsafe { crate::resource::zirk_rt_cancel_shield_exit() };
                assert!(current_task_is_cancelled());
                0
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn protected_cleanup_restores_nested_shield_depth_on_return() {
        assert_eq!(
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                with_cancellation_shield(|| {
                    assert_eq!(
                        with_exec(|exec| exec.registry.get(me).unwrap().shield_depth),
                        1
                    );
                    with_cancellation_shield(|| {
                        assert_eq!(
                            with_exec(|exec| exec.registry.get(me).unwrap().shield_depth),
                            2
                        );
                    });
                    assert_eq!(
                        with_exec(|exec| exec.registry.get(me).unwrap().shield_depth),
                        1
                    );
                    42
                })
            }),
            42
        );
    }

    #[test]
    fn protected_cleanup_delivers_pending_cancellation_after_normal_return() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                with_exec(|exec| exec.request_cancel(me));
                with_cancellation_shield(|| {
                    assert!(!current_task_is_cancelled());
                });
                assert!(current_task_is_cancelled());
                0
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn protected_cleanup_restores_shield_depth_during_unwind() {
        assert_eq!(
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    with_cancellation_shield(|| panic!("cleanup failure"));
                }));
                assert!(panic.is_err());
                assert_eq!(
                    with_exec(|exec| exec.registry.get(me).unwrap().shield_depth),
                    0
                );
                assert!(!current_task_is_cancelled());
                7
            }),
            7
        );
    }

    #[test]
    fn protected_cleanup_preserves_pending_cancellation_after_unwind() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                with_exec(|exec| exec.request_cancel(me));
                let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    with_cancellation_shield(|| panic!("cleanup failure"));
                }));
                assert!(panic.is_err());
                assert_eq!(
                    with_exec(|exec| exec.registry.get(me).unwrap().shield_depth),
                    0
                );
                assert!(current_task_is_cancelled());
                0
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn cancelled_sleep_inside_cleanup_shield_resumes_and_finishes_cleanup() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let child = spawn(|| {
                    enter_cancellation_shield();
                    sleep(60_000_000_000);
                    exit_cancellation_shield();
                    7
                });
                yield_now();
                cancel_current(child);
                await_task(child)
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn pending_cancellation_wins_over_return_with_unbalanced_cleanup_shield() {
        let outcome = std::panic::catch_unwind(|| {
            run(|| {
                let me = CURRENT_TASK.get().expect("root task");
                with_exec(|exec| exec.request_cancel(me));
                enter_cancellation_shield();
                // Models a return from finally that bypasses the generated
                // shield exit. Completing the context must still cancel.
                0
            })
        });
        assert!(outcome.is_err());
    }

    #[test]
    fn cancellation_query_tracks_active_task_state_and_shielding() {
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observations = Arc::clone(&seen);
        run(move || {
            let child = spawn({
                let observations = Arc::clone(&observations);
                move || {
                    observations
                        .lock()
                        .unwrap()
                        .push(current_task_is_cancelled());
                    yield_now();
                    observations
                        .lock()
                        .unwrap()
                        .push(current_task_is_cancelled());
                    yield_now();
                    observations
                        .lock()
                        .unwrap()
                        .push(current_task_is_cancelled());
                    0
                }
            });
            yield_now();
            with_exec(|exec| {
                let tcb = exec.registry.get_mut(child).unwrap();
                tcb.cancel_requested = true;
                tcb.shield_depth = 1;
            });
            // The child's next turn observes a shielded request as deferred.
            yield_now();
            with_exec(|exec| {
                let tcb = exec.registry.get_mut(child).unwrap();
                tcb.shield_depth = 0;
            });
            yield_now();
            0
        });
        assert_eq!(*seen.lock().unwrap(), vec![false, false, true]);
    }

    #[test]
    fn a_root_with_no_children_runs_and_returns() {
        assert_eq!(run(|| 42), 42);
    }

    #[test]
    fn background_admission_is_bounded_before_tasks_become_ready() {
        assert_eq!(
            run(|| {
                let mut admitted = 0;
                let mut rejected = 0;
                for _ in 0..=BACKGROUND_JOB_CAPACITY {
                    match spawn_background(|| 0) {
                        Ok(_) => admitted += 1,
                        Err(AdmissionError::CapacityReached) => rejected += 1,
                        Err(error) => panic!("unexpected background admission error: {error:?}"),
                    }
                }
                assert_eq!(rejected, 1);
                with_exec(|exec| {
                    assert_eq!(exec.next_background_job, BACKGROUND_JOB_CAPACITY as u64 + 1);
                    assert_eq!(exec.background_tasks.len(), BACKGROUND_JOB_CAPACITY);
                    assert_eq!(exec.active_tasks, BACKGROUND_JOB_CAPACITY + 1);
                });
                admitted
            }),
            BACKGROUND_JOB_CAPACITY
        );
    }

    #[test]
    fn background_failure_is_recorded_after_the_task_terminates() {
        assert_eq!(
            run(|| {
                spawn_background(|| panic!("detached failure")).unwrap();
                yield_now();
                let diagnostics = background_diagnostics_current();
                assert!(matches!(
                    diagnostics.as_slice(),
                    [BackgroundDiagnostic::JobFailed(failure)]
                        if failure.message == "detached failure"
                ));
                7
            }),
            7
        );
    }

    #[test]
    fn detached_zirk_throwable_stays_rooted_through_deferred_delivery() {
        let _collector_guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        let delivered_rooted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_rooted = std::sync::Arc::clone(&delivered_rooted);
        let throwable_for_callback = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_throwable = std::sync::Arc::clone(&throwable_for_callback);
        let executor =
            Executor::with_background_failure_handler(Duration::from_millis(250), move |failure| {
                assert_eq!(failure.job, BackgroundJobId(1));
                assert_eq!(failure.message, "background task failed");
                // Delivery is outside the completion borrow. A collection in
                // the callback must still see the Throwable in the pending
                // queue, even though the failed task is already terminal.
                crate::collector::collect_for_tests();
                let object = callback_throwable.load(Ordering::SeqCst) as *mut std::ffi::c_void;
                callback_rooted.store(
                    crate::collector::is_live_for_tests(object),
                    Ordering::SeqCst,
                );
            });

        let outcome = executor.run_with_root(move || {
            let throwable =
                unsafe { crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) };
            throwable_for_callback.store(throwable as usize, Ordering::SeqCst);
            let bits = throwable as usize;
            spawn_background(move || std::panic::panic_any(BranchFailure(bits))).unwrap();
            yield_now();
            assert!(delivered_rooted.load(Ordering::SeqCst));
            let (pending, active) = with_exec(|exec| {
                (
                    exec.pending_background_failures.len(),
                    exec.background_delivery_active,
                )
            });
            assert_eq!(pending, 0);
            assert!(!active);
            1
        });

        assert!(matches!(outcome, TaskOutcome::Value(1)));
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn detached_failure_delivery_is_non_reentrant_and_releases_queue_entry() {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let depth = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_depth = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (callback_calls, callback_depth, callback_max_depth) =
            (calls.clone(), depth.clone(), max_depth.clone());
        let executor =
            Executor::with_background_failure_handler(Duration::from_millis(250), move |failure| {
                let current = callback_depth.fetch_add(1, Ordering::SeqCst) + 1;
                callback_max_depth.fetch_max(current, Ordering::SeqCst);
                callback_calls.fetch_add(1, Ordering::SeqCst);
                if failure.job == BackgroundJobId(1) {
                    // Re-enter the runtime while notification is running. The
                    // mutable executor borrow must be gone, but recursive
                    // dispatch must be refused until this callback returns.
                    with_exec(|exec| {
                        exec.background.admit(BackgroundJobId(2)).unwrap();
                        exec.pending_background_failures
                            .push_back(PendingBackgroundFailure {
                                job: BackgroundJobId(2),
                                message: "second deferred failure".to_owned(),
                                throwable: None,
                            });
                    });
                    assert!(!Executor::dispatch_pending_background_failure());
                }
                callback_depth.fetch_sub(1, Ordering::SeqCst);
            });

        let outcome = executor.run_with_root(move || {
            spawn_background(|| panic!("first deferred failure")).unwrap();
            yield_now();
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            assert_eq!(max_depth.load(Ordering::SeqCst), 1);
            with_exec(|exec| {
                assert!(exec.pending_background_failures.is_empty());
                assert!(!exec.background_delivery_active);
            });
            2
        });
        assert!(matches!(outcome, TaskOutcome::Value(2)));
    }

    #[test]
    fn root_completion_cancels_and_joins_cooperative_background_work() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, Ordering::SeqCst);
        assert_eq!(
            run(|| {
                spawn_background(|| {
                    while !current_task_is_cancelled() {
                        sleep(60_000_000_000);
                    }
                    CLEANED.store(true, Ordering::SeqCst);
                    0
                })
                .unwrap();
                yield_now();
                9
            }),
            9
        );
        assert!(CLEANED.load(Ordering::SeqCst));
    }

    #[test]
    fn root_completion_disarms_suspended_background_timer_before_drain_finishes() {
        static TIMER_DISARMED_DURING_CLEANUP: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        TIMER_DISARMED_DURING_CLEANUP.store(false, Ordering::SeqCst);

        let outcome = Executor::with_background_shutdown_grace(Duration::from_millis(250))
            .run_with_root(|| {
                let timer_baseline = timer_registration_count_for_test();
                spawn_background(move || {
                    while !current_task_is_cancelled() {
                        sleep(60_000_000_000);
                    }
                    // Root completion requests cancellation, which must
                    // unregister the suspended sleep before this task runs
                    // cleanup and the shutdown drain can finish.
                    TIMER_DISARMED_DURING_CLEANUP.store(
                        timer_registration_count_for_test() == timer_baseline,
                        Ordering::SeqCst,
                    );
                    0
                })
                .unwrap();

                yield_now();
                assert_eq!(
                    timer_registration_count_for_test(),
                    timer_baseline + 1,
                    "background task must be suspended on its timer before root completion"
                );
                17
            });

        assert!(matches!(outcome, TaskOutcome::Value(17)));
        assert!(
            TIMER_DISARMED_DURING_CLEANUP.load(Ordering::SeqCst),
            "background timer must be unregistered before cooperative drain completes"
        );
    }

    #[test]
    fn configured_background_grace_allows_cooperative_cleanup() {
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        CLEANED.store(false, Ordering::SeqCst);
        let outcome = Executor::with_background_shutdown_grace(Duration::from_millis(100))
            .run_with_root(|| {
                spawn_background(|| {
                    while !current_task_is_cancelled() {
                        sleep(60_000_000_000);
                    }
                    CLEANED.store(true, Ordering::SeqCst);
                    0
                })
                .unwrap();
                yield_now();
                13
            });
        assert!(matches!(outcome, TaskOutcome::Value(13)));
        assert!(CLEANED.load(Ordering::SeqCst));
    }

    #[test]
    fn expired_background_grace_reports_and_stops_before_unstarted_work_runs() {
        static RAN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        RAN.store(false, Ordering::SeqCst);
        let outcome =
            Executor::with_background_shutdown_grace(Duration::ZERO).run_with_root(|| {
                spawn_background(|| {
                    RAN.store(true, Ordering::SeqCst);
                    0
                })
                .unwrap();
                11
            });
        assert!(matches!(outcome, TaskOutcome::Value(11)));
        assert!(!RAN.load(Ordering::SeqCst));
    }

    #[test]
    fn expired_background_grace_joins_parallel_workers_before_heap_shutdown() {
        static CHUNK_STARTED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        static CHUNK_FINISHED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        static RELEASE_CHUNK: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        CHUNK_STARTED.store(false, Ordering::SeqCst);
        CHUNK_FINISHED.store(false, Ordering::SeqCst);
        RELEASE_CHUNK.store(false, Ordering::SeqCst);

        let release = std::thread::spawn(|| {
            while !CHUNK_STARTED.load(Ordering::SeqCst) {
                std::thread::yield_now();
            }
            std::thread::sleep(Duration::from_millis(30));
            RELEASE_CHUNK.store(true, Ordering::SeqCst);
        });

        let outcome =
            Executor::with_background_shutdown_grace(Duration::ZERO).run_with_root(|| {
                spawn_background(|| {
                    let values = crate::pool::parallel_map(
                        crate::pool::CoreBudget::Exact(2),
                        vec![(), ()],
                        |_| {
                            // Keep the registered worker alive while the root
                            // finishes and its zero-length shutdown grace expires.
                            CHUNK_STARTED.store(true, Ordering::SeqCst);
                            while !RELEASE_CHUNK.load(Ordering::SeqCst) {
                                std::thread::yield_now();
                            }
                            CHUNK_FINISHED.store(true, Ordering::SeqCst);
                            1usize
                        },
                    )
                    .unwrap();
                    assert_eq!(values, vec![1]);
                    0
                })
                .unwrap();
                while !CHUNK_STARTED.load(Ordering::SeqCst) {
                    yield_now();
                }
                12
            });

        release.join().unwrap();
        assert!(matches!(outcome, TaskOutcome::Value(12)));
        assert!(CHUNK_FINISHED.load(Ordering::SeqCst));
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

    #[test]
    fn source_service_registration_starts_automatically_after_required_predecessor_ready() {
        use crate::services::ServiceContext;

        let first_ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let second_started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ready_for_first = Arc::clone(&first_ready);
        let started_for_second = Arc::clone(&second_started);
        let first_ready_root = Arc::clone(&first_ready);
        let second_started_root = Arc::clone(&second_started);
        let outcome = Executor::new().run_with_root(move || {
            register_service_with_prior_required_current(
                "source-first",
                true,
                std::ptr::null_mut(),
                move |context: ServiceContext| {
                    context.ready().unwrap();
                    ready_for_first.store(true, Ordering::SeqCst);
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    0
                },
            )
            .unwrap();
            register_service_with_prior_required_current(
                "source-second",
                true,
                std::ptr::null_mut(),
                move |context: ServiceContext| {
                    assert!(context.ready().is_ok());
                    started_for_second.store(true, Ordering::SeqCst);
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    0
                },
            )
            .unwrap();
            while !second_started_root.load(Ordering::SeqCst) {
                yield_now();
            }
            assert!(first_ready_root.load(Ordering::SeqCst));
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
        assert!(second_started.load(Ordering::SeqCst));
    }

    #[test]
    fn shared_shutdown_deadline_reports_and_joins_pending_services() {
        use crate::services::ServiceContext;

        static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        STARTED.store(false, Ordering::SeqCst);
        CLEANED.store(false, Ordering::SeqCst);
        let outcome =
            Executor::with_background_shutdown_grace(Duration::ZERO).run_with_root(|| {
                let service = register_service_with_prior_required_current(
                    "deadline-service",
                    true,
                    std::ptr::null_mut(),
                    |context: ServiceContext| {
                        context.ready().unwrap();
                        STARTED.store(true, Ordering::SeqCst);
                        while !current_task_cancel_requested() {
                            yield_now();
                        }
                        CLEANED.store(true, Ordering::SeqCst);
                        0
                    },
                )
                .unwrap();
                while !STARTED.load(Ordering::SeqCst) {
                    yield_now();
                }
                with_exec(|exec| {
                    exec.background_shutdown_deadline =
                        Some(Instant::now() - Duration::from_millis(1));
                    assert!(exec.background_drain_expired());
                    let report = exec.drain_expired_report(Vec::new());
                    assert_eq!(report.services, [service.0]);
                    assert!(report.jobs.is_empty());
                });
                0
            });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
        assert!(CLEANED.load(Ordering::SeqCst));
    }

    #[test]
    fn service_failure_hook_consumes_startup_exception_and_drains() {
        use crate::services::ServiceContext;
        use std::ffi::c_void;

        static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        REPORTED.store(false, Ordering::SeqCst);
        let outcome = Executor::new().run_with_root(|| {
            let service = register_service_with_prior_required_current(
                "throw-during-startup",
                true,
                std::ptr::null_mut(),
                |_context: ServiceContext| {
                    let throwable = unsafe {
                        crate::collector::synthetic_object_for_tests(std::ptr::null(), 0)
                    };
                    unsafe { crate::exceptions::zirk_rt_throw(throwable.cast::<c_void>()) };
                    assert!(crate::exceptions::zirk_rt_has_pending_exception());
                    assert_eq!(service_failed_current(), Ok(()));
                    assert!(!crate::exceptions::zirk_rt_has_pending_exception());
                    REPORTED.store(true, Ordering::SeqCst);
                    0
                },
            )
            .unwrap();
            while !REPORTED.load(Ordering::SeqCst) {
                yield_now();
            }
            with_exec(|exec| {
                let failures = exec.services.failures();
                assert_eq!(failures.len(), 1);
                assert_eq!(failures[0].service, service);
                assert!(failures[0].during_startup);
                assert_eq!(
                    failures[0].message,
                    "service callback raised a Zirk exception"
                );
            });
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
    }

    #[test]
    fn service_failure_hook_consumes_post_readiness_exception_and_drains() {
        use crate::services::ServiceContext;
        use std::ffi::c_void;

        static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        REPORTED.store(false, Ordering::SeqCst);
        let outcome = Executor::new().run_with_root(|| {
            let service = register_service_with_prior_required_current(
                "throw-after-ready",
                true,
                std::ptr::null_mut(),
                |context: ServiceContext| {
                    context.ready().unwrap();
                    let throwable = unsafe {
                        crate::collector::synthetic_object_for_tests(std::ptr::null(), 0)
                    };
                    unsafe { crate::exceptions::zirk_rt_throw(throwable.cast::<c_void>()) };
                    assert_eq!(service_failed_current(), Ok(()));
                    assert!(!crate::exceptions::zirk_rt_has_pending_exception());
                    REPORTED.store(true, Ordering::SeqCst);
                    0
                },
            )
            .unwrap();
            while !REPORTED.load(Ordering::SeqCst) {
                yield_now();
            }
            with_exec(|exec| {
                let failures = exec.services.failures();
                assert_eq!(failures.len(), 1);
                assert_eq!(failures[0].service, service);
                assert!(!failures[0].during_startup);
            });
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
    }

    #[test]
    fn services_gate_dependencies_and_join_cleanup_in_reverse_start_order() {
        use crate::services::ServiceContext;
        use std::sync::{Arc, Mutex};

        let started = Arc::new(AtomicUsize::new(0));
        let cleaned = Arc::new(Mutex::new(Vec::new()));
        let started_root = Arc::clone(&started);
        let cleaned_root = Arc::clone(&cleaned);
        let outcome = Executor::new().run_with_root(move || {
            let first_started = Arc::clone(&started_root);
            let first_cleaned = Arc::clone(&cleaned_root);
            let first = register_service_current(
                "first",
                true,
                &[],
                std::ptr::null_mut(),
                move |ctx: ServiceContext| {
                    let id = ctx.id();
                    ctx.ready().unwrap();
                    first_started.fetch_add(1, Ordering::SeqCst);
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    first_cleaned.lock().unwrap().push(id);
                    0
                },
            )
            .unwrap();
            let second_started = Arc::clone(&started_root);
            let second_cleaned = Arc::clone(&cleaned_root);
            register_service_current(
                "second",
                true,
                &[first],
                std::ptr::null_mut(),
                move |ctx: ServiceContext| {
                    let id = ctx.id();
                    ctx.ready().unwrap();
                    second_started.fetch_add(1, Ordering::SeqCst);
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    second_cleaned.lock().unwrap().push(id);
                    0
                },
            )
            .unwrap();
            start_services_current().unwrap();
            while started_root.load(Ordering::SeqCst) != 2 {
                yield_now();
            }
            7
        });
        assert!(matches!(outcome, TaskOutcome::Value(7)));
        let stopped = cleaned.lock().unwrap();
        assert_eq!(stopped.len(), 2);
        assert!(
            stopped[0] > stopped[1],
            "cleanup follows reverse actual start order: {stopped:?}"
        );
    }

    #[test]
    fn service_return_before_ready_fails_startup_and_cleans_started_dependencies() {
        use crate::services::ServiceContext;
        use std::sync::{Arc, Mutex};

        let returned = Arc::new(AtomicUsize::new(0));
        let cleaned = Arc::new(Mutex::new(false));
        let returned_root = Arc::clone(&returned);
        let cleaned_root = Arc::clone(&cleaned);
        let outcome = Executor::new().run_with_root(move || {
            let cleanup = Arc::clone(&cleaned_root);
            let first = register_service_current(
                "ready-first",
                true,
                &[],
                std::ptr::null_mut(),
                move |ctx: ServiceContext| {
                    ctx.ready().unwrap();
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    *cleanup.lock().unwrap() = true;
                    0
                },
            )
            .unwrap();
            let returned = Arc::clone(&returned_root);
            register_service_current(
                "early-return",
                true,
                &[first],
                std::ptr::null_mut(),
                move |_ctx: ServiceContext| {
                    returned.store(1, Ordering::SeqCst);
                    0
                },
            )
            .unwrap();
            start_services_current().unwrap();
            while returned_root.load(Ordering::SeqCst) == 0 {
                yield_now();
            }
            // Let the failed service's terminal transition and startup drain
            // run before the root itself requests application shutdown.
            yield_now();
            9
        });
        assert!(matches!(outcome, TaskOutcome::Value(9)));
        assert!(
            *cleaned.lock().unwrap(),
            "started dependency cleanup must finish before executor return"
        );
    }

    #[test]
    fn service_admission_checks_executor_task_quota_before_inserting_tcb() {
        use crate::services::ServiceContext;

        let outcome = Executor::new().run_with_root(|| {
            register_service_current(
                "quota",
                true,
                &[],
                std::ptr::null_mut(),
                |_ctx: ServiceContext| 0,
            )
            .unwrap();
            // Simulate the 4,096-active-task boundary without allocating 4,095
            // coroutine stacks just to exercise the admission guard.
            with_exec(|exec| exec.active_tasks = crate::task::MAX_ACTIVE_TASKS);
            assert_eq!(
                start_services_current(),
                Err(ServiceError::TaskCapacityReached)
            );
            with_exec(|exec| {
                assert!(
                    exec.service_tasks.is_empty(),
                    "quota rejection must precede TCB insertion"
                );
                assert!(exec.service_callbacks.is_empty());
                assert!(exec.timers.is_empty());
                exec.active_tasks = 1;
            });
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
    }

    #[test]
    fn dependency_waiting_service_capture_is_gc_rooted_until_cleanup_finishes() {
        let _collector_guard = crate::collector::test_support::test_guard();
        crate::collector::reset_state_for_tests();
        use crate::services::ServiceContext;
        use std::sync::Arc;

        let dependent_started = Arc::new(AtomicUsize::new(0));
        let cleanup_kept_capture = Arc::new(AtomicUsize::new(0));
        let dependent_started_root = Arc::clone(&dependent_started);
        let cleanup_kept_capture_root = Arc::clone(&cleanup_kept_capture);
        let outcome = Executor::new().run_with_root(move || {
            let capture =
                unsafe { crate::collector::synthetic_object_for_tests(std::ptr::null(), 0) };
            let first = register_service_current(
                "gated-prerequisite",
                true,
                &[],
                std::ptr::null_mut(),
                move |context: ServiceContext| {
                    // Leave the dependent unstarted for one forced collection.
                    yield_now();
                    context.ready().unwrap();
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    0
                },
            )
            .unwrap();
            let dependent_started = Arc::clone(&dependent_started_root);
            let cleanup_kept_capture = Arc::clone(&cleanup_kept_capture_root);
            let capture_bits = capture as usize;
            register_service_current(
                "capture-waiter",
                true,
                &[first],
                capture,
                move |context: ServiceContext| {
                    context.ready().unwrap();
                    dependent_started.store(1, Ordering::SeqCst);
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    crate::collector::collect_for_tests();
                    assert!(
                        crate::collector::is_live_for_tests(capture_bits as *mut std::ffi::c_void,),
                        "the capture root must stay live throughout cooperative cleanup"
                    );
                    cleanup_kept_capture.store(1, Ordering::SeqCst);
                    0
                },
            )
            .unwrap();
            start_services_current().unwrap();
            yield_now();
            crate::collector::collect_for_tests();
            assert!(
                crate::collector::is_live_for_tests(capture),
                "a dependency-blocked service capture must be traced by the executor"
            );
            while dependent_started_root.load(Ordering::SeqCst) == 0 {
                yield_now();
            }
            0
        });
        assert!(matches!(outcome, TaskOutcome::Value(0)));
        assert_eq!(cleanup_kept_capture.load(Ordering::SeqCst), 1);
        crate::collector::reset_state_for_tests();
    }

    #[test]
    fn service_panic_before_ready_records_startup_failure_and_drains_dependencies() {
        use crate::services::ServiceContext;
        use std::sync::Arc;

        let dependency_cleaned = Arc::new(AtomicUsize::new(0));
        let dependency_cleaned_root = Arc::clone(&dependency_cleaned);
        let outcome = Executor::new().run_with_root(move || {
            let cleaned = Arc::clone(&dependency_cleaned_root);
            let dependency = register_service_current(
                "dependency",
                true,
                &[],
                std::ptr::null_mut(),
                move |context: ServiceContext| {
                    context.ready().unwrap();
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    cleaned.store(1, Ordering::SeqCst);
                    0
                },
            )
            .unwrap();
            let failed = register_service_current(
                "startup-panic",
                true,
                &[dependency],
                std::ptr::null_mut(),
                |_context: ServiceContext| -> usize { panic!("injected before ready") },
            )
            .unwrap();
            start_services_current().unwrap();
            while matches!(
                with_exec(|exec| exec.services.lifecycle()),
                ServiceState::Starting | ServiceState::Ready
            ) {
                yield_now();
            }
            with_exec(|exec| {
                let failures = exec.services.failures();
                assert_eq!(failures.len(), 1);
                assert_eq!(failures[0].service, failed);
                assert!(failures[0].during_startup);
            });
            while with_exec(|exec| exec.services.lifecycle()) != ServiceState::Stopped {
                yield_now();
            }
            with_exec(|exec| {
                assert_eq!(
                    exec.active_tasks, 1,
                    "only root remains active after service cleanup"
                );
                assert!(exec.service_tasks.is_empty());
                assert!(exec.service_task_ids.is_empty());
                assert!(
                    exec.service_callbacks.is_empty(),
                    "never-started and drained callback roots are released"
                );
                assert!(exec.timers.is_empty());
                assert_eq!(exec.timers.active_registrations(), 0);
            });
            1
        });
        assert!(matches!(outcome, TaskOutcome::Value(1)));
        assert_eq!(dependency_cleaned.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn service_panic_after_ready_records_runtime_failure_and_drains_dependencies() {
        use crate::services::ServiceContext;
        use std::sync::Arc;

        let dependency_cleaned = Arc::new(AtomicUsize::new(0));
        let dependency_cleaned_root = Arc::clone(&dependency_cleaned);
        let outcome = Executor::new().run_with_root(move || {
            let cleaned = Arc::clone(&dependency_cleaned_root);
            let dependency = register_service_current(
                "ready-dependency",
                true,
                &[],
                std::ptr::null_mut(),
                move |context: ServiceContext| {
                    context.ready().unwrap();
                    while !current_task_cancel_requested() {
                        yield_now();
                    }
                    cleaned.store(1, Ordering::SeqCst);
                    0
                },
            )
            .unwrap();
            let failed = register_service_current(
                "runtime-panic",
                true,
                &[dependency],
                std::ptr::null_mut(),
                |context: ServiceContext| -> usize {
                    context.ready().unwrap();
                    panic!("injected after ready")
                },
            )
            .unwrap();
            start_services_current().unwrap();
            while matches!(
                with_exec(|exec| exec.services.lifecycle()),
                ServiceState::Starting | ServiceState::Ready
            ) {
                yield_now();
            }
            with_exec(|exec| {
                let failures = exec.services.failures();
                assert_eq!(failures.len(), 1);
                assert_eq!(failures[0].service, failed);
                assert!(!failures[0].during_startup);
            });
            while with_exec(|exec| exec.services.lifecycle()) != ServiceState::Stopped {
                yield_now();
            }
            with_exec(|exec| {
                assert_eq!(exec.active_tasks, 1);
                assert!(exec.service_tasks.is_empty());
                assert!(exec.service_task_ids.is_empty());
                assert!(exec.service_callbacks.is_empty());
                assert!(exec.timers.is_empty());
                assert_eq!(exec.timers.active_registrations(), 0);
            });
            2
        });
        assert!(matches!(outcome, TaskOutcome::Value(2)));
        assert_eq!(dependency_cleaned.load(Ordering::SeqCst), 1);
    }
}
