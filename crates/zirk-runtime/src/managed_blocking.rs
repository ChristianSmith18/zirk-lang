//! Bounded pool for blocking native work.
//!
//! Rust jobs are owned closures. The compiler-gated callback path below is
//! separate: it accepts managed addresses only while a registered worker runs
//! and the executor caller cooperatively joins with its shadow roots live.

use std::collections::VecDeque;
use std::ffi::c_void;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

/// Per-pool upper bound on persistent native workers.
pub const MAX_WORKERS: usize = 64;
/// Process-wide upper bound on workers across all managed blocking pools.
pub const MAX_GLOBAL_WORKERS: usize = 64;
/// Hard upper bound on pending jobs in one pool.
pub const MAX_QUEUE_CAPACITY: usize = 1024;

static ACTIVE_POOL_WORKERS: AtomicUsize = AtomicUsize::new(0);
static MANAGED_POOL: std::sync::OnceLock<Mutex<Option<ManagedBlockingPool>>> =
    std::sync::OnceLock::new();

/// Stable status values shared with generated LLVM code for one managed
/// callback. The callback stores its typed result or exception in caller-owned
/// slots before publishing this status with Release ordering.
pub const MANAGED_CALL_PENDING: u32 = u32::MAX;
pub const MANAGED_CALL_OK: u32 = 0;
pub const MANAGED_CALL_THROWN: u32 = 1;
pub const MANAGED_CALL_PANICKED: u32 = 2;

/// Uniform adapter emitted by LLVM for one exact `Callable<T>` layout.
///
/// # Safety
/// `target`, `captures`, `result`, and `exception` must remain valid until this
/// function returns. `result` and `exception` are rooted slots in the parked
/// caller's shadow frame. The caller must join the submitted job before those
/// slots or the captured callable cease to be live.
pub type ManagedCallThunk = unsafe extern "C-unwind" fn(
    target: *mut c_void,
    captures: *mut c_void,
    result: *mut c_void,
    exception: *mut c_void,
) -> u32;

struct ManagedCall {
    thunk: ManagedCallThunk,
    target: usize,
    captures: usize,
    result: usize,
    exception: usize,
    status: Arc<AtomicU32>,
}

// SAFETY: this payload is admitted only by `run_managed_callback`, which
// keeps its Zirk caller suspended (and its stack/root frame stable) until the
// worker publishes completion. The callback/capture pointers are used only by
// a registered mutator and remain rooted by that caller frame until join.
unsafe impl Send for ManagedCall {}

impl ManagedCall {
    unsafe fn execute(self) {
        let status = Arc::clone(&self.status);
        let mutator = crate::collector::register_managed_mutator();
        let callback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: caller's IR verifier and generated adapter establish the
            // exact typed layout; this wrapper preserves it through join.
            unsafe {
                (self.thunk)(
                    self.target as *mut c_void,
                    self.captures as *mut c_void,
                    self.result as *mut c_void,
                    self.exception as *mut c_void,
                )
            }
        }));
        let callback_status = match callback {
            Ok(MANAGED_CALL_OK) => MANAGED_CALL_OK,
            Ok(MANAGED_CALL_THROWN) => MANAGED_CALL_THROWN,
            Ok(_) | Err(_) => {
                // A native unwind may have invalidated generated shadow-frame
                // addresses. Clear them before the final poll/unregister.
                crate::collector::clear_unwound_mutator_roots();
                MANAGED_CALL_PANICKED
            }
        };
        crate::collector::safepoint_poll();
        drop(mutator);
        status.store(callback_status, Ordering::Release);
        crate::executor::notify_pool_event();
    }
}

fn with_managed_pool<R>(f: impl FnOnce(&ManagedBlockingPool) -> R) -> R {
    let pool = MANAGED_POOL.get_or_init(|| Mutex::new(None));
    let mut pool = pool.lock().unwrap_or_else(|error| error.into_inner());
    if pool.is_none() {
        let workers = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .clamp(1, MAX_WORKERS);
        *pool = Some(
            ManagedBlockingPool::new(workers, MAX_QUEUE_CAPACITY).unwrap_or_else(|_| {
                crate::failure::fatal("managed thread pool initialization failed")
            }),
        );
    }
    f(pool.as_ref().expect("managed pool initialized"))
}

/// Stops accepting managed callbacks at runtime shutdown, drains every
/// accepted callback, joins the fixed workers, and releases the process-wide
/// worker reservation. An already-running callback cannot be forcibly stopped.
pub fn shutdown_managed_pool() {
    let Some(pool) = MANAGED_POOL.get() else {
        return;
    };
    let mut pool = pool.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(mut active) = pool.take() {
        let _ = active.shutdown();
    }
}

/// Submit one compiler-verified Zirk callback and cooperatively join it.
/// Queue saturation yields the caller to the executor and retries admission;
/// it never creates another OS thread. Cancellation does not abandon accepted
/// work: the caller keeps yielding until the worker unregisters and publishes
/// completion, so its roots and runtime world remain valid through the join.
///
/// # Safety
/// The adapter and four addresses must satisfy [`ManagedCallThunk`]'s
/// lifetime contract. `result` and `exception` must be correctly sized and
/// rooted output slots; the target signature must match the adapter layout.
pub unsafe fn run_managed_callback(
    thunk: ManagedCallThunk,
    target: *mut c_void,
    captures: *mut c_void,
    result: *mut c_void,
    exception: *mut c_void,
) -> u32 {
    if target.is_null() || exception.is_null() {
        crate::failure::fatal("invalid managed thread callback ABI arguments");
    }
    if !crate::executor::in_executor_task() {
        crate::failure::fatal("managed thread callback requires an executor task");
    }
    let status = Arc::new(AtomicU32::new(MANAGED_CALL_PENDING));
    let mut call = Some(ManagedCall {
        thunk,
        target: target as usize,
        captures: captures as usize,
        result: result as usize,
        exception: exception as usize,
        status: Arc::clone(&status),
    });
    loop {
        let epoch = crate::executor::pool_event_epoch();
        let submitted = call.take().expect("managed callback submitted once");
        match with_managed_pool(|pool| pool.try_submit(move || unsafe { submitted.execute() })) {
            Ok(()) => break,
            Err(SubmitError::QueueFull) => {
                call = Some(ManagedCall {
                    thunk,
                    target: target as usize,
                    captures: captures as usize,
                    result: result as usize,
                    exception: exception as usize,
                    status: Arc::clone(&status),
                });
                // Do not abandon an unaccepted submission on cancellation;
                // the caller parks with its roots live until a queue slot opens.
                crate::executor::park_current_for_pool_event_if_unchanged(epoch);
            }
            Err(SubmitError::ShuttingDown) => {
                crate::failure::fatal("managed thread pool is shutting down")
            }
        }
    }
    while status.load(Ordering::Acquire) == MANAGED_CALL_PENDING {
        let epoch = crate::executor::pool_event_epoch();
        if status.load(Ordering::Acquire) == MANAGED_CALL_PENDING {
            crate::executor::park_current_for_pool_event_if_unchanged(epoch);
        }
    }
    status.load(Ordering::Acquire)
}

/// Outcome when an executor branch waits for an owned native job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedJobError {
    Cancelled,
    Panicked,
    ShuttingDown,
    NotInTask,
}

#[derive(Clone)]
pub struct OwnedJobCancellation(Arc<std::sync::atomic::AtomicBool>);

impl OwnedJobCancellation {
    /// Whether the waiting Zirk branch requested cancellation while this
    /// native job was running. Jobs should poll between interruptible steps;
    /// a syscall already in progress still has to finish safely.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

const OWNED_JOB_QUEUED: u8 = 0;
const OWNED_JOB_RUNNING: u8 = 1;
const OWNED_JOB_COMPLETE: u8 = 2;
const OWNED_JOB_CANCELLED: u8 = 3;

/// Admits an owned native operation to the shared bounded blocking pool and
/// suspends only the calling Zirk task until its result is ready. `job` must
/// own all data it uses; it may not retain managed Zirk pointers. A queued job
/// can be cancelled before dispatch. Once a worker has begun the operation,
/// the caller joins it before returning so the result and any native resource
/// are either delivered or dropped safely.
pub fn run_owned_job<T: Send + 'static>(
    job: impl FnOnce() -> T + Send + 'static,
) -> Result<T, OwnedJobError> {
    run_owned_job_cancellable(move |_| job())
}

pub fn run_owned_job_cancellable<T: Send + 'static>(
    job: impl FnOnce(OwnedJobCancellation) -> T + Send + 'static,
) -> Result<T, OwnedJobError> {
    if !crate::executor::in_executor_task() {
        return Err(OwnedJobError::NotInTask);
    }
    let task = crate::executor::current_task();
    let state = Arc::new(AtomicU32::new(u32::from(OWNED_JOB_QUEUED)));
    let cancellation = OwnedJobCancellation(Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let job = Arc::new(Mutex::new(Some(job)));
    let result = Arc::new(Mutex::new(None::<Result<T, ()>>));

    loop {
        if crate::executor::task_cancel_requested(task) {
            if state
                .compare_exchange(
                    u32::from(OWNED_JOB_QUEUED),
                    u32::from(OWNED_JOB_CANCELLED),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                cancellation.0.store(true, Ordering::Release);
                job.lock().unwrap_or_else(|error| error.into_inner()).take();
                return Err(OwnedJobError::Cancelled);
            }
        }

        let epoch = crate::executor::pool_event_epoch();
        let worker_state = Arc::clone(&state);
        let worker_cancellation = cancellation.clone();
        let worker_job = Arc::clone(&job);
        let worker_result = Arc::clone(&result);
        let submitted = with_managed_pool(|pool| {
            pool.try_submit(move || {
                if worker_state
                    .compare_exchange(
                        u32::from(OWNED_JOB_QUEUED),
                        u32::from(OWNED_JOB_RUNNING),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_err()
                {
                    return;
                }
                let Some(job) = worker_job
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .take()
                else {
                    *worker_result
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = Some(Err(()));
                    worker_state.store(u32::from(OWNED_JOB_COMPLETE), Ordering::Release);
                    crate::executor::notify_pool_event();
                    return;
                };
                let outcome =
                    catch_unwind(AssertUnwindSafe(|| job(worker_cancellation))).map_err(|_| ());
                *worker_result
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(outcome);
                worker_state.store(u32::from(OWNED_JOB_COMPLETE), Ordering::Release);
                crate::executor::notify_pool_event();
            })
        });

        match submitted {
            Ok(()) => break,
            Err(SubmitError::QueueFull) => {
                if crate::executor::task_cancel_requested(task) {
                    continue;
                }
                crate::executor::park_current_for_pool_event_if_unchanged(epoch);
            }
            Err(SubmitError::ShuttingDown) => return Err(OwnedJobError::ShuttingDown),
        }
    }

    loop {
        match state.load(Ordering::Acquire) as u8 {
            OWNED_JOB_COMPLETE => {
                let outcome = result
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .take()
                    .expect("completed owned job publishes a result");
                if crate::executor::task_cancel_requested(task) {
                    return Err(OwnedJobError::Cancelled);
                }
                return outcome.map_err(|()| OwnedJobError::Panicked);
            }
            OWNED_JOB_CANCELLED => return Err(OwnedJobError::Cancelled),
            OWNED_JOB_QUEUED if crate::executor::task_cancel_requested(task) => {
                if state
                    .compare_exchange(
                        u32::from(OWNED_JOB_QUEUED),
                        u32::from(OWNED_JOB_CANCELLED),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    cancellation.0.store(true, Ordering::Release);
                    job.lock().unwrap_or_else(|error| error.into_inner()).take();
                    return Err(OwnedJobError::Cancelled);
                }
            }
            OWNED_JOB_RUNNING if crate::executor::task_cancel_requested(task) => {
                cancellation.0.store(true, Ordering::Release);
            }
            _ => {}
        }
        let epoch = crate::executor::pool_event_epoch();
        if state.load(Ordering::Acquire) == u32::from(OWNED_JOB_COMPLETE) {
            continue;
        }
        crate::executor::park_current_for_pool_event_if_unchanged(epoch);
    }
}

/// C ABI used by compiler-generated adapters for source-level `Thread.run`.
///
/// # Safety
/// The adapter and slots must satisfy [`run_managed_callback`]'s safety
/// contract, and every managed address must remain rooted until this call
/// returns.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn zirk_rt_thread_run(
    thunk: ManagedCallThunk,
    target: *mut c_void,
    captures: *mut c_void,
    result: *mut c_void,
    exception: *mut c_void,
) -> u32 {
    // SAFETY: this exported ABI is only emitted for verifier-checked
    // ThreadRun instructions; the generated frame retains both closure and
    // output roots across this cooperative join.
    unsafe { run_managed_callback(thunk, target, captures, result, exception) }
}

/// Invalid pool configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    ZeroWorkers,
    TooManyWorkers,
    ZeroQueueCapacity,
    QueueCapacityTooLarge,
}

/// Explicit nonblocking admission result. `QueueFull` applies while every
/// worker is occupied and the bounded FIFO queue has reached capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError {
    QueueFull,
    ShuttingDown,
}

/// Stable snapshot of work completed during graceful shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownReport {
    pub completed: u64,
    pub panicked: u64,
}

type Job = Box<dyn FnOnce() + Send + 'static>;

struct State {
    queue: VecDeque<Job>,
    shutting_down: bool,
    completed: u64,
    panicked: u64,
}

struct Shared {
    state: Mutex<State>,
    available: Condvar,
    queue_capacity: usize,
}

/// Fixed-size worker pool with bounded FIFO admission and graceful shutdown.
///
/// `try_submit` never waits for capacity: callers receive `QueueFull` and
/// choose their own backpressure policy (retry, reject, or defer). Shutdown
/// stops admission, drains accepted jobs in FIFO order, then joins every
/// worker. A running closure cannot be forcibly stopped, so shutdown may wait
/// indefinitely if a closure never returns. Each job panic is contained and
/// counted; it does not terminate its worker or prevent queued jobs running.
pub struct ManagedBlockingPool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
    worker_reservation: Option<WorkerReservation>,
    configured_workers: usize,
    joined: bool,
}

impl ManagedBlockingPool {
    /// Starts exactly `worker_count` persistent workers. No worker is created
    /// as a consequence of submitting work.
    pub fn new(worker_count: usize, queue_capacity: usize) -> Result<Self, NewPoolError> {
        if worker_count == 0 {
            return Err(NewPoolError::Invalid(ConfigError::ZeroWorkers));
        }
        if worker_count > MAX_WORKERS {
            return Err(NewPoolError::Invalid(ConfigError::TooManyWorkers));
        }
        if queue_capacity == 0 {
            return Err(NewPoolError::Invalid(ConfigError::ZeroQueueCapacity));
        }
        if queue_capacity > MAX_QUEUE_CAPACITY {
            return Err(NewPoolError::Invalid(ConfigError::QueueCapacityTooLarge));
        }
        let worker_reservation = WorkerReservation::acquire(worker_count)?;

        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                queue: VecDeque::with_capacity(queue_capacity),
                shutting_down: false,
                completed: 0,
                panicked: 0,
            }),
            available: Condvar::new(),
            queue_capacity,
        });
        let mut workers = Vec::with_capacity(worker_count);
        for index in 0..worker_count {
            let worker_shared = Arc::clone(&shared);
            match thread::Builder::new()
                .name(format!("zirk-blocking-{index}"))
                .spawn(move || worker_loop(worker_shared))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    {
                        let mut state = lock(&shared.state);
                        state.shutting_down = true;
                        shared.available.notify_all();
                    }
                    for worker in workers {
                        let _ = worker.join();
                    }
                    return Err(NewPoolError::Spawn(error));
                }
            }
        }
        Ok(Self {
            shared,
            workers,
            worker_reservation: Some(worker_reservation),
            configured_workers: worker_count,
            joined: false,
        })
    }

    /// Enqueues one owned job, or returns immediately when the queue is full
    /// or shutdown has begun. Successful calls are ordered by the state-lock
    /// acquisition order, which is the pool's FIFO admission order.
    pub fn try_submit(&self, job: impl FnOnce() + Send + 'static) -> Result<(), SubmitError> {
        let mut state = lock(&self.shared.state);
        if state.shutting_down {
            return Err(SubmitError::ShuttingDown);
        }
        if state.queue.len() == self.shared.queue_capacity {
            return Err(SubmitError::QueueFull);
        }
        state.queue.push_back(Box::new(job));
        self.shared.available.notify_one();
        Ok(())
    }

    /// Stops admission, drains accepted jobs, and joins all workers. Repeated
    /// calls are harmless and return the same final counters.
    pub fn shutdown(&mut self) -> ShutdownReport {
        if !self.joined {
            {
                let mut state = lock(&self.shared.state);
                state.shutting_down = true;
                self.shared.available.notify_all();
            }
            for worker in self.workers.drain(..) {
                // Worker entry points contain job panics. A join error would
                // indicate an unexpected worker-loop panic, not a job panic.
                let _ = worker.join();
            }
            self.joined = true;
            drop(self.worker_reservation.take());
        }
        let state = lock(&self.shared.state);
        ShutdownReport {
            completed: state.completed,
            panicked: state.panicked,
        }
    }

    /// Configured persistent worker count.
    pub fn worker_count(&self) -> usize {
        self.configured_workers
    }

    /// Current queued jobs (does not include running jobs).
    pub fn queued(&self) -> usize {
        lock(&self.shared.state).queue.len()
    }
}

impl Drop for ManagedBlockingPool {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

/// Pool construction error, preserving OS thread creation diagnostics.
#[derive(Debug)]
pub enum NewPoolError {
    Invalid(ConfigError),
    Spawn(io::Error),
    GlobalWorkerLimitReached {
        active: usize,
        requested: usize,
        maximum: usize,
    },
}

impl From<ConfigError> for NewPoolError {
    fn from(error: ConfigError) -> Self {
        Self::Invalid(error)
    }
}

struct WorkerReservation {
    count: usize,
}

impl WorkerReservation {
    fn acquire(count: usize) -> Result<Self, NewPoolError> {
        let mut active = ACTIVE_POOL_WORKERS.load(Ordering::Acquire);
        loop {
            let Some(next) = active.checked_add(count) else {
                return Err(NewPoolError::GlobalWorkerLimitReached {
                    active,
                    requested: count,
                    maximum: MAX_GLOBAL_WORKERS,
                });
            };
            if next > MAX_GLOBAL_WORKERS {
                return Err(NewPoolError::GlobalWorkerLimitReached {
                    active,
                    requested: count,
                    maximum: MAX_GLOBAL_WORKERS,
                });
            }
            match ACTIVE_POOL_WORKERS.compare_exchange_weak(
                active,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(Self { count }),
                Err(observed) => active = observed,
            }
        }
    }
}

impl Drop for WorkerReservation {
    fn drop(&mut self) {
        let previous = ACTIVE_POOL_WORKERS.fetch_sub(self.count, Ordering::AcqRel);
        debug_assert!(previous >= self.count);
    }
}

fn worker_loop(shared: Arc<Shared>) {
    loop {
        let job = {
            let mut state = lock(&shared.state);
            loop {
                if let Some(job) = state.queue.pop_front() {
                    break Some(job);
                }
                if state.shutting_down {
                    break None;
                }
                state = shared
                    .available
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        let Some(job) = job else { return };
        // Popping frees one bounded admission slot for parked Thread.run callers.
        crate::executor::notify_pool_event();
        let panicked = catch_unwind(AssertUnwindSafe(job)).is_err();
        let mut state = lock(&shared.state);
        state.completed = state.completed.saturating_add(1);
        if panicked {
            state.panicked = state.panicked.saturating_add(1);
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    include!("managed_blocking_bench.rs");

    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    static POOL_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_pool_tests() -> std::sync::MutexGuard<'static, ()> {
        POOL_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    #[test]
    fn owned_blocking_job_suspends_only_its_branch_and_returns_owned_result() {
        let _serial = lock_pool_tests();
        let sibling_ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sibling_state = Arc::clone(&sibling_ran);
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            crate::executor::spawn(move || {
                sibling_state.store(true, Ordering::Release);
                0
            });
            let result = run_owned_job(|| {
                thread::sleep(Duration::from_millis(25));
                String::from("owned-result")
            });
            assert_eq!(result, Ok(String::from("owned-result")));
            assert!(sibling_ran.load(Ordering::Acquire));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        shutdown_managed_pool();
    }

    #[test]
    fn owned_blocking_job_joins_running_work_before_reporting_cancellation() {
        let _serial = lock_pool_tests();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_state = Arc::clone(&completed);
            let result_dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
            struct DropProbe(Arc<std::sync::atomic::AtomicBool>);
            impl Drop for DropProbe {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::Release);
                }
            }
            let dropped_state = Arc::clone(&result_dropped);
            let parent = crate::executor::current_task();
            crate::executor::spawn(move || {
                crate::executor::cancel_current(parent);
                0
            });
            let result = run_owned_job(move || {
                thread::sleep(Duration::from_millis(25));
                worker_state.store(true, Ordering::Release);
                DropProbe(dropped_state)
            });
            assert!(matches!(result, Err(OwnedJobError::Cancelled)));
            assert!(completed.load(Ordering::Acquire));
            assert!(result_dropped.load(Ordering::Acquire));
            0
        });
        assert!(matches!(
            outcome,
            crate::task::TaskOutcome::Panicked(payload)
                if payload.is::<crate::executor::Cancelled>()
        ));
        shutdown_managed_pool();
    }

    #[test]
    fn owned_blocking_job_is_removed_when_cancelled_before_dispatch() {
        let _serial = lock_pool_tests();
        let job_ran = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let job_ran_after = Arc::clone(&job_ran);
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            let workers = with_managed_pool(ManagedBlockingPool::worker_count);
            let (started_tx, started_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let release_rx = Arc::new(Mutex::new(release_rx));
            with_managed_pool(|pool| {
                for _ in 0..workers {
                    let started = started_tx.clone();
                    let release = Arc::clone(&release_rx);
                    pool.try_submit(move || {
                        started.send(()).unwrap();
                        lock(&release).recv().unwrap();
                    })
                    .expect("fill every worker without exceeding the shared queue");
                }
            });
            for _ in 0..workers {
                started_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("each pool worker is occupied before queueing the cancellable job");
            }

            let parent = crate::executor::current_task();
            crate::executor::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while with_managed_pool(ManagedBlockingPool::queued) == 0 {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "owned job was not queued behind occupied workers"
                    );
                    crate::executor::yield_now();
                }
                crate::executor::cancel_current(parent);
                0
            });

            let job_ran_in_worker = Arc::clone(&job_ran);
            assert_eq!(
                run_owned_job(move || job_ran_in_worker.store(true, Ordering::Release)),
                Err(OwnedJobError::Cancelled)
            );
            for _ in 0..workers {
                release_tx.send(()).unwrap();
            }
            0
        });
        assert!(matches!(
            outcome,
            crate::task::TaskOutcome::Panicked(payload)
                if payload.is::<crate::executor::Cancelled>()
        ));
        shutdown_managed_pool();
        assert!(!job_ran_after.load(Ordering::Acquire));
        assert_eq!(ACTIVE_POOL_WORKERS.load(Ordering::Acquire), 0);
    }

    unsafe extern "C-unwind" fn write_managed_result(
        target: *mut c_void,
        _captures: *mut c_void,
        result: *mut c_void,
        exception: *mut c_void,
    ) -> u32 {
        // SAFETY: the test passes a live usize address and correctly-sized
        // output slots, all on the parked root task's stable coroutine stack.
        unsafe {
            *result.cast::<usize>() = *target.cast::<usize>() + 1;
            *exception.cast::<*mut c_void>() = std::ptr::null_mut();
        }
        MANAGED_CALL_OK
    }

    unsafe extern "C-unwind" fn mark_slow_callback_complete(
        target: *mut c_void,
        _captures: *mut c_void,
        _result: *mut c_void,
        exception: *mut c_void,
    ) -> u32 {
        let completed = unsafe { &*target.cast::<std::sync::atomic::AtomicBool>() };
        thread::sleep(Duration::from_millis(30));
        completed.store(true, Ordering::Release);
        unsafe { *exception.cast::<*mut c_void>() = std::ptr::null_mut() };
        MANAGED_CALL_OK
    }

    unsafe extern "C-unwind" fn publish_test_exception(
        target: *mut c_void,
        _captures: *mut c_void,
        _result: *mut c_void,
        exception: *mut c_void,
    ) -> u32 {
        unsafe {
            *exception.cast::<*mut c_void>() = target;
        }
        MANAGED_CALL_THROWN
    }

    unsafe extern "C-unwind" fn panic_managed_callback(
        _target: *mut c_void,
        _captures: *mut c_void,
        _result: *mut c_void,
        _exception: *mut c_void,
    ) -> u32 {
        panic!("injected managed callback panic");
    }

    struct GcResultArgs {
        descriptor: *const c_void,
    }

    struct GcCancellationArgs {
        descriptor: usize,
        collection_complete: std::sync::atomic::AtomicBool,
        release_callback: std::sync::atomic::AtomicBool,
        callback_returning: std::sync::atomic::AtomicBool,
        capture_survived: std::sync::atomic::AtomicBool,
        result_survived: std::sync::atomic::AtomicBool,
        status: AtomicUsize,
        capture_live_after_join: std::sync::atomic::AtomicBool,
        result_live_after_join: std::sync::atomic::AtomicBool,
        exception_null: std::sync::atomic::AtomicBool,
        capture_live_after_unregistration_collection: std::sync::atomic::AtomicBool,
        result_live_after_unregistration_collection: std::sync::atomic::AtomicBool,
    }

    struct ManagedAdmissionArgs {
        release: std::sync::atomic::AtomicBool,
        started: AtomicUsize,
        callbacks_finished: AtomicUsize,
        callers_finished: AtomicUsize,
    }

    struct ManagedAdmissionJob {
        shared: Arc<ManagedAdmissionArgs>,
        started: std::sync::atomic::AtomicBool,
        task_id: Mutex<Option<crate::task::TaskId>>,
    }

    unsafe extern "C-unwind" fn hold_managed_callback_until_release(
        target: *mut c_void,
        _captures: *mut c_void,
        _result: *mut c_void,
        exception: *mut c_void,
    ) -> u32 {
        let job = unsafe { &*target.cast::<ManagedAdmissionJob>() };
        job.started.store(true, Ordering::Release);
        job.shared.started.fetch_add(1, Ordering::AcqRel);
        while !job.shared.release.load(Ordering::Acquire) {
            thread::yield_now();
        }
        job.shared.callbacks_finished.fetch_add(1, Ordering::AcqRel);
        unsafe { *exception.cast::<*mut c_void>() = std::ptr::null_mut() };
        MANAGED_CALL_OK
    }

    unsafe extern "C-unwind" fn publish_managed_result_then_force_gc(
        target: *mut c_void,
        _captures: *mut c_void,
        result: *mut c_void,
        exception: *mut c_void,
    ) -> u32 {
        let args = unsafe { &*target.cast::<GcResultArgs>() };
        let object = unsafe {
            crate::memory::zirk_rt_alloc(
                crate::collector::HEADER_BYTES,
                std::mem::align_of::<usize>(),
            )
        };
        unsafe {
            *object.cast::<*mut c_void>() = args.descriptor.cast_mut();
            *result.cast::<*mut c_void>() = object;
        }
        // The next allocation crosses this deliberately tiny threshold. The
        // worker parks in its allocation safepoint with the result already in
        // the caller's rooted output slot, forcing STW to trace that slot.
        crate::collector::set_threshold_for_tests(1);
        let _ = unsafe {
            crate::memory::zirk_rt_alloc(
                crate::collector::HEADER_BYTES,
                std::mem::align_of::<usize>(),
            )
        };
        unsafe { *exception.cast::<*mut c_void>() = std::ptr::null_mut() };
        MANAGED_CALL_OK
    }

    unsafe extern "C-unwind" fn collect_then_wait_for_cancellation(
        target: *mut c_void,
        captures: *mut c_void,
        result: *mut c_void,
        exception: *mut c_void,
    ) -> u32 {
        let args = unsafe { &*target.cast::<GcCancellationArgs>() };
        let capture = captures;
        let object = unsafe {
            crate::memory::zirk_rt_alloc(
                crate::collector::HEADER_BYTES,
                std::mem::align_of::<usize>(),
            )
        };
        unsafe {
            *object.cast::<*mut c_void>() = args.descriptor as *mut c_void;
            *result.cast::<*mut c_void>() = object;
        }

        // Exercise the executor heap's allocation-triggered stop-the-world
        // collection path on the registered worker. The worker must request
        // collection and park; it cannot collect locally because task roots
        // belong to the executor coordinator.
        crate::collector::set_threshold_for_tests(1);
        for _ in 0..8 {
            let _ = unsafe {
                crate::memory::zirk_rt_alloc(
                    crate::collector::HEADER_BYTES,
                    std::mem::align_of::<usize>(),
                )
            };
        }
        args.capture_survived.store(
            crate::collector::is_live_for_tests(capture),
            Ordering::Release,
        );
        args.result_survived.store(
            crate::collector::is_live_for_tests(unsafe { *result.cast::<*mut c_void>() }),
            Ordering::Release,
        );
        unsafe { *exception.cast::<*mut c_void>() = std::ptr::null_mut() };
        args.collection_complete.store(true, Ordering::Release);

        while !args.release_callback.load(Ordering::Acquire) {
            thread::yield_now();
        }
        args.callback_returning.store(true, Ordering::Release);
        MANAGED_CALL_OK
    }

    #[test]
    fn managed_callback_runs_on_registered_worker_and_publishes_result_before_join() {
        let _serial = lock_pool_tests();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let input = 41usize;
            let mut result = 0usize;
            let mut exception: *mut c_void = std::ptr::null_mut();
            // SAFETY: all pointers refer to this task's stable stack and remain
            // live until the call has joined the managed worker.
            let status = unsafe {
                run_managed_callback(
                    write_managed_result,
                    (&input as *const usize).cast_mut().cast(),
                    std::ptr::null_mut(),
                    (&mut result as *mut usize).cast(),
                    (&mut exception as *mut *mut c_void).cast(),
                )
            };
            assert_eq!(status, MANAGED_CALL_OK);
            assert_eq!(result, 42);
            assert!(exception.is_null());
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        shutdown_managed_pool();
    }

    #[test]
    fn caller_cancellation_does_not_return_until_managed_worker_joins() {
        let _serial = lock_pool_tests();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let callback_state = Arc::clone(&completed);
            let parent = crate::executor::current_task();
            crate::executor::spawn(move || {
                crate::executor::cancel_current(parent);
                0
            });
            let mut exception: *mut c_void = std::ptr::null_mut();
            // SAFETY: the Arc's root-task-owned strong reference and the
            // exception slot remain live until the worker has joined.
            let status = unsafe {
                run_managed_callback(
                    mark_slow_callback_complete,
                    Arc::as_ptr(&callback_state).cast_mut().cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    (&mut exception as *mut *mut c_void).cast(),
                )
            };
            assert_eq!(status, MANAGED_CALL_OK);
            assert!(completed.load(Ordering::Acquire));
            0
        });
        assert!(matches!(
            outcome,
            crate::task::TaskOutcome::Panicked(payload)
                if payload.is::<crate::executor::Cancelled>()
        ));
        shutdown_managed_pool();
    }

    #[test]
    fn managed_callback_publishes_exception_before_join_returns() {
        let _serial = lock_pool_tests();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let exception_object = 0x1234usize as *mut c_void;
            let mut exception: *mut c_void = std::ptr::null_mut();
            let status = unsafe {
                run_managed_callback(
                    publish_test_exception,
                    exception_object,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    (&mut exception as *mut *mut c_void).cast(),
                )
            };
            assert_eq!(status, MANAGED_CALL_THROWN);
            assert_eq!(exception, exception_object);
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        shutdown_managed_pool();
    }

    #[test]
    fn managed_callback_panic_is_contained_and_reported_to_the_caller() {
        let _serial = lock_pool_tests();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let target = 1usize;
            let mut exception: *mut c_void = std::ptr::null_mut();
            let status = unsafe {
                run_managed_callback(
                    panic_managed_callback,
                    (&target as *const usize).cast_mut().cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    (&mut exception as *mut *mut c_void).cast(),
                )
            };
            assert_eq!(status, MANAGED_CALL_PANICKED);
            assert!(exception.is_null());
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        shutdown_managed_pool();
    }

    #[test]
    fn managed_reference_result_survives_forced_collection_before_join() {
        let _serial = lock_pool_tests();
        let _collector = crate::collector::test_support::test_guard();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
            let args = GcResultArgs {
                descriptor: descriptor.as_ptr().cast(),
            };
            let mut result: *mut c_void = std::ptr::null_mut();
            let mut exception: *mut c_void = std::ptr::null_mut();
            let mut roots = [
                (&mut result as *mut *mut c_void).cast::<c_void>(),
                (&mut exception as *mut *mut c_void).cast::<c_void>(),
            ];
            unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
            let status = unsafe {
                run_managed_callback(
                    publish_managed_result_then_force_gc,
                    (&args as *const GcResultArgs).cast_mut().cast(),
                    std::ptr::null_mut(),
                    (&mut result as *mut *mut c_void).cast(),
                    (&mut exception as *mut *mut c_void).cast(),
                )
            };
            assert_eq!(status, MANAGED_CALL_OK);
            assert!(!result.is_null());
            assert!(crate::collector::is_live_for_tests(result));
            assert!(exception.is_null());
            crate::collector::zirk_rt_pop_frame();
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        shutdown_managed_pool();
    }

    #[test]
    fn cancellation_joins_active_managed_callback_across_gc_pressure_with_roots_live() {
        let _serial = lock_pool_tests();
        let _collector = crate::collector::test_support::test_guard();
        let descriptor = crate::collector::descriptor_with_fields_for_tests(&[]);
        let args = Arc::new(GcCancellationArgs {
            descriptor: descriptor.as_ptr() as usize,
            collection_complete: std::sync::atomic::AtomicBool::new(false),
            release_callback: std::sync::atomic::AtomicBool::new(false),
            callback_returning: std::sync::atomic::AtomicBool::new(false),
            capture_survived: std::sync::atomic::AtomicBool::new(false),
            result_survived: std::sync::atomic::AtomicBool::new(false),
            status: AtomicUsize::new(usize::MAX),
            capture_live_after_join: std::sync::atomic::AtomicBool::new(false),
            result_live_after_join: std::sync::atomic::AtomicBool::new(false),
            exception_null: std::sync::atomic::AtomicBool::new(false),
            capture_live_after_unregistration_collection: std::sync::atomic::AtomicBool::new(false),
            result_live_after_unregistration_collection: std::sync::atomic::AtomicBool::new(false),
        });
        let observed_args = Arc::clone(&args);
        let run_args = Arc::clone(&args);
        let outcome = crate::executor::Executor::new().run_with_root(move || {
            let capture = unsafe {
                crate::memory::zirk_rt_alloc(
                    crate::collector::HEADER_BYTES,
                    std::mem::align_of::<usize>(),
                )
            };
            unsafe { *capture.cast::<*mut c_void>() = descriptor.as_ptr().cast_mut().cast() };
            let mut captured = capture;
            let mut result: *mut c_void = std::ptr::null_mut();
            let mut exception: *mut c_void = std::ptr::null_mut();
            let mut roots = [
                (&mut captured as *mut *mut c_void).cast::<c_void>(),
                (&mut result as *mut *mut c_void).cast::<c_void>(),
                (&mut exception as *mut *mut c_void).cast::<c_void>(),
            ];
            unsafe { crate::collector::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };

            let parent = crate::executor::current_task();
            let cancel_args = Arc::as_ptr(&run_args) as usize;
            crate::executor::spawn(move || {
                let args = unsafe { &*(cancel_args as *const GcCancellationArgs) };
                while !args.collection_complete.load(Ordering::Acquire) {
                    crate::executor::yield_now();
                }
                crate::executor::cancel_current(parent);
                args.release_callback.store(true, Ordering::Release);
                0
            });

            // The second executor task cancels only after the callback has
            // collected and is waiting on its release barrier. Return from
            // this call therefore proves cancellation drained the worker.
            let status = unsafe {
                run_managed_callback(
                    collect_then_wait_for_cancellation,
                    Arc::as_ptr(&run_args).cast_mut().cast(),
                    captured,
                    (&mut result as *mut *mut c_void).cast(),
                    (&mut exception as *mut *mut c_void).cast(),
                )
            };
            run_args.status.store(status as usize, Ordering::Release);
            run_args.capture_live_after_join.store(
                crate::collector::is_live_for_tests(captured),
                Ordering::Release,
            );
            run_args.result_live_after_join.store(
                crate::collector::is_live_for_tests(result),
                Ordering::Release,
            );
            run_args
                .exception_null
                .store(exception.is_null(), Ordering::Release);

            // Completion is published after the worker unregisters. A fresh
            // stop-the-world collection verifies the worker has left the
            // participant set while the caller's roots remain live.
            crate::collector::collect_for_tests();
            run_args.capture_live_after_unregistration_collection.store(
                crate::collector::is_live_for_tests(captured),
                Ordering::Release,
            );
            run_args.result_live_after_unregistration_collection.store(
                crate::collector::is_live_for_tests(result),
                Ordering::Release,
            );
            crate::collector::zirk_rt_pop_frame();
            0
        });
        assert!(matches!(
            outcome,
            crate::task::TaskOutcome::Panicked(payload)
                if payload.is::<crate::executor::Cancelled>()
        ));
        assert_eq!(
            observed_args.status.load(Ordering::Acquire),
            MANAGED_CALL_OK as usize
        );
        assert!(observed_args.callback_returning.load(Ordering::Acquire));
        assert!(observed_args.capture_survived.load(Ordering::Acquire));
        assert!(observed_args.result_survived.load(Ordering::Acquire));
        assert!(
            observed_args
                .capture_live_after_join
                .load(Ordering::Acquire)
        );
        assert!(observed_args.result_live_after_join.load(Ordering::Acquire));
        assert!(observed_args.exception_null.load(Ordering::Acquire));
        assert!(
            args.capture_live_after_unregistration_collection
                .load(Ordering::Acquire)
        );
        assert!(
            args.result_live_after_unregistration_collection
                .load(Ordering::Acquire)
        );
        shutdown_managed_pool();
        assert!(
            !crate::collector::test_support::has_registered_participants_for_tests(),
            "managed worker participant remained registered after pool shutdown"
        );
    }

    #[test]
    fn managed_thread_admission_stays_within_worker_and_queue_ceilings() {
        let _serial = lock_pool_tests();
        let _collector = crate::collector::test_support::test_guard();
        let outcome = crate::executor::Executor::new().run_with_root(|| {
            let shared = Arc::new(ManagedAdmissionArgs {
                release: std::sync::atomic::AtomicBool::new(false),
                started: AtomicUsize::new(0),
                callbacks_finished: AtomicUsize::new(0),
                callers_finished: AtomicUsize::new(0),
            });
            let worker_limit = with_managed_pool(ManagedBlockingPool::worker_count);
            let admitted_count = worker_limit + MAX_QUEUE_CAPACITY;
            let callback_count = admitted_count + 1;
            let make_job = || {
                Arc::new(ManagedAdmissionJob {
                    shared: Arc::clone(&shared),
                    started: std::sync::atomic::AtomicBool::new(false),
                    task_id: Mutex::new(None),
                })
            };
            let submit = |job: Arc<ManagedAdmissionJob>| {
                let caller_job = Arc::clone(&job);
                crate::executor::spawn(move || {
                    *caller_job
                        .task_id
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) =
                        Some(crate::executor::current_task());
                    let mut exception: *mut c_void = std::ptr::null_mut();
                    let status = unsafe {
                        run_managed_callback(
                            hold_managed_callback_until_release,
                            Arc::as_ptr(&caller_job).cast_mut().cast(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            (&mut exception as *mut *mut c_void).cast(),
                        )
                    };
                    assert_eq!(status, MANAGED_CALL_OK);
                    assert!(exception.is_null());
                    caller_job
                        .shared
                        .callers_finished
                        .fetch_add(1, Ordering::AcqRel);
                    0
                });
            };
            let mut jobs = (0..admitted_count).map(|_| make_job()).collect::<Vec<_>>();

            for job in &jobs {
                submit(Arc::clone(job));
            }

            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                let (workers, queued) =
                    with_managed_pool(|pool| (pool.worker_count(), pool.queued()));
                if queued == MAX_QUEUE_CAPACITY && shared.started.load(Ordering::Acquire) == workers
                {
                    assert_eq!(workers, worker_limit);
                    assert_eq!(ACTIVE_POOL_WORKERS.load(Ordering::Acquire), worker_limit);
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "managed Thread.run admission did not reach the documented queue ceiling"
                );
                crate::executor::yield_now();
            }

            let queued_jobs = jobs
                .iter()
                .filter(|job| !job.started.load(Ordering::Acquire));
            let queued_jobs = queued_jobs.collect::<Vec<_>>();
            assert_eq!(queued_jobs.len(), MAX_QUEUE_CAPACITY);
            let victim = Arc::clone(queued_jobs[0]);
            drop(queued_jobs);
            let victim_id = victim
                .task_id
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .expect("every admitted callback has a task id");

            // The extra caller must wait for a queue slot. Cancel an already
            // admitted callback before its worker starts; its frame must stay
            // live until the accepted native callback is joined.
            let overflow_job = make_job();
            submit(Arc::clone(&overflow_job));
            jobs.push(overflow_job);
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while jobs.last().unwrap().task_id.lock().unwrap().is_none() {
                assert!(std::time::Instant::now() < deadline);
                crate::executor::yield_now();
            }
            crate::executor::cancel_current(victim_id);
            shared.release.store(true, Ordering::Release);
            while shared.callers_finished.load(Ordering::Acquire) != callback_count {
                assert!(
                    std::time::Instant::now() < deadline,
                    "managed Thread.run callers did not drain after capacity was released"
                );
                crate::executor::yield_now();
            }
            assert_eq!(shared.started.load(Ordering::Acquire), callback_count);
            assert_eq!(
                shared.callbacks_finished.load(Ordering::Acquire),
                callback_count
            );
            assert!(victim.started.load(Ordering::Acquire));
            0
        });
        assert!(matches!(outcome, crate::task::TaskOutcome::Value(0)));
        shutdown_managed_pool();
        assert_eq!(ACTIVE_POOL_WORKERS.load(Ordering::Acquire), 0);
        assert!(!crate::collector::test_support::has_registered_participants_for_tests());
    }

    #[test]
    fn validates_worker_and_queue_bounds() {
        let _serial = lock_pool_tests();
        assert!(matches!(
            ManagedBlockingPool::new(0, 1),
            Err(NewPoolError::Invalid(ConfigError::ZeroWorkers))
        ));
        assert!(matches!(
            ManagedBlockingPool::new(MAX_WORKERS + 1, 1),
            Err(NewPoolError::Invalid(ConfigError::TooManyWorkers))
        ));
        assert!(matches!(
            ManagedBlockingPool::new(1, 0),
            Err(NewPoolError::Invalid(ConfigError::ZeroQueueCapacity))
        ));
        assert!(matches!(
            ManagedBlockingPool::new(1, MAX_QUEUE_CAPACITY + 1),
            Err(NewPoolError::Invalid(ConfigError::QueueCapacityTooLarge))
        ));
        let pool = ManagedBlockingPool::new(MAX_WORKERS, 1).unwrap();
        assert_eq!(pool.worker_count(), MAX_WORKERS);
        drop(pool);
    }

    #[test]
    fn bounded_admission_rejects_saturation_and_contains_panics() {
        let _serial = lock_pool_tests();
        let mut pool = ManagedBlockingPool::new(2, 3).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        for _ in 0..2 {
            let started = started_tx.clone();
            let release = Arc::clone(&release_rx);
            pool.try_submit(move || {
                started.send(()).unwrap();
                lock(&release).recv().unwrap();
            })
            .unwrap();
        }
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        for _ in 0..3 {
            pool.try_submit(|| {}).unwrap();
        }
        assert_eq!(pool.queued(), 3);
        assert_eq!(pool.try_submit(|| {}), Err(SubmitError::QueueFull));

        release_tx.send(()).unwrap();
        release_tx.send(()).unwrap();
        assert_eq!(
            pool.shutdown(),
            ShutdownReport {
                completed: 5,
                panicked: 0
            }
        );
        assert_eq!(pool.try_submit(|| {}), Err(SubmitError::ShuttingDown));

        // A queued panic must not kill a worker or prevent the next job.
        let mut panic_pool = ManagedBlockingPool::new(1, 2).unwrap();
        let (done_tx, done_rx) = mpsc::channel();
        panic_pool
            .try_submit(|| panic!("contained job panic"))
            .unwrap();
        panic_pool
            .try_submit(move || done_tx.send(()).unwrap())
            .unwrap();
        assert_eq!(
            panic_pool.shutdown(),
            ShutdownReport {
                completed: 2,
                panicked: 1
            }
        );
        done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    }

    #[test]
    fn maximum_queue_capacity_accepts_exactly_the_documented_pending_job_limit() {
        let _serial = lock_pool_tests();
        let mut pool = ManagedBlockingPool::new(1, MAX_QUEUE_CAPACITY).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        pool.try_submit(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        // Keep the sole worker occupied so these submissions remain queued.
        for _ in 0..MAX_QUEUE_CAPACITY {
            pool.try_submit(|| {}).unwrap();
        }
        assert_eq!(pool.queued(), MAX_QUEUE_CAPACITY);
        assert_eq!(pool.try_submit(|| {}), Err(SubmitError::QueueFull));
        assert_eq!(pool.queued(), MAX_QUEUE_CAPACITY);

        release_tx.send(()).unwrap();
        assert_eq!(
            pool.shutdown(),
            ShutdownReport {
                completed: (MAX_QUEUE_CAPACITY + 1) as u64,
                panicked: 0,
            }
        );
        assert_eq!(pool.queued(), 0);
    }

    #[test]
    fn shutdown_drains_fifo_queue_and_joins_workers() {
        let _serial = lock_pool_tests();
        let mut pool = ManagedBlockingPool::new(1, 4).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        pool.try_submit(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let (order_tx, order_rx) = mpsc::channel();
        for value in 0..4 {
            let tx = order_tx.clone();
            pool.try_submit(move || tx.send(value).unwrap()).unwrap();
        }
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let shutdown_thread = thread::spawn(move || {
            shutdown_tx.send(pool.shutdown()).unwrap();
            pool
        });
        assert_eq!(
            shutdown_rx.recv_timeout(Duration::from_millis(30)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        release_tx.send(()).unwrap();
        let report = shutdown_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            report,
            ShutdownReport {
                completed: 5,
                panicked: 0
            }
        );
        let order = (0..4)
            .map(|_| order_rx.recv_timeout(Duration::from_secs(1)).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(order, vec![0, 1, 2, 3]);
        let mut joined_pool = shutdown_thread.join().unwrap();
        assert_eq!(joined_pool.shutdown(), report);
    }

    #[test]
    fn global_worker_limit_is_exact_and_released_after_shutdown() {
        let _serial = lock_pool_tests();
        let mut pool = ManagedBlockingPool::new(MAX_GLOBAL_WORKERS, 1).unwrap();
        assert!(matches!(
            ManagedBlockingPool::new(1, 1),
            Err(NewPoolError::GlobalWorkerLimitReached {
                active: MAX_GLOBAL_WORKERS,
                requested: 1,
                maximum: MAX_GLOBAL_WORKERS,
            })
        ));
        assert_eq!(pool.worker_count(), MAX_GLOBAL_WORKERS);
        let _ = pool.shutdown();

        let next = ManagedBlockingPool::new(1, 1).unwrap();
        assert_eq!(next.worker_count(), 1);
    }

    #[test]
    fn concurrent_pool_creation_never_exceeds_the_global_worker_limit() {
        let _serial = lock_pool_tests();
        const POOL_COUNT: usize = 8;
        const WORKERS_PER_POOL: usize = MAX_GLOBAL_WORKERS / POOL_COUNT;
        let ready = Arc::new(std::sync::Barrier::new(POOL_COUNT + 1));
        let release = Arc::new(std::sync::Barrier::new(POOL_COUNT + 1));
        let mut creators = Vec::with_capacity(POOL_COUNT);

        for _ in 0..POOL_COUNT {
            let ready = Arc::clone(&ready);
            let release = Arc::clone(&release);
            creators.push(thread::spawn(move || {
                let pool = ManagedBlockingPool::new(WORKERS_PER_POOL, 1).unwrap();
                ready.wait();
                release.wait();
                drop(pool);
            }));
        }

        ready.wait();
        assert_eq!(
            ACTIVE_POOL_WORKERS.load(Ordering::Acquire),
            MAX_GLOBAL_WORKERS
        );
        release.wait();
        for creator in creators {
            creator.join().unwrap();
        }
        assert_eq!(ACTIVE_POOL_WORKERS.load(Ordering::Acquire), 0);
    }
}
