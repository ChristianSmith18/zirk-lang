//! Deterministic cancellation, GC-pressure, and runtime shutdown integration.
//!
//! This exercises only the public runtime ABI. The current shutdown ABI flushes
//! streams after the executor has already drained at run_main's boundary; it
//! does not expose a post-shutdown pending-task query.
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_background_spawn, zirk_rt_branch_register, zirk_rt_cancel, zirk_rt_init,
    zirk_rt_job_wait, zirk_rt_run_main, zirk_rt_scope_cancel, zirk_rt_scope_enter,
    zirk_rt_scope_exit, zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_spawn,
    zirk_rt_timer_every_checked,
};

const WORKERS: usize = 4;
const ROUNDS: usize = 8;
const LONG_SLEEP_NANOS: i64 = 86_400_000_000_000;
const GC_ALLOCATIONS: usize = 384;
const GC_ALLOCATION_SIZE: usize = 4096;
const CANCELLED: usize = usize::MAX;
const BACKGROUND_ROUNDS: usize = 4;
const BACKGROUND_ALLOCATIONS: usize = 384;
const BACKGROUND_ALLOCATION_SIZE: usize = 4096;
const CAPTURED_VALUE: usize = 0x5eed_cafe;
const BACKGROUND_CAPACITY: usize = 256;
const BACKGROUND_POLL_NANOS: i64 = 1_000_000;
const BACKGROUND_QUOTA_POLL_LIMIT: usize = 30_000;
const TIMER_CYCLES: usize = 32;
const TIMER_CAPTURE_VALUE: usize = 0x71ae_2026;

static QUOTA_RELEASE: AtomicUsize = AtomicUsize::new(0);
static QUOTA_STARTED: AtomicUsize = AtomicUsize::new(0);
static QUOTA_COMPLETED: AtomicUsize = AtomicUsize::new(0);
static QUOTA_ADMITTED: AtomicUsize = AtomicUsize::new(0);
static QUOTA_REJECTIONS: AtomicUsize = AtomicUsize::new(0);
static QUOTA_REUSE_STARTED: AtomicUsize = AtomicUsize::new(0);
static QUOTA_REUSE_COMPLETED: AtomicUsize = AtomicUsize::new(0);

static BACKGROUND_STARTED: [AtomicUsize; BACKGROUND_ROUNDS] =
    [const { AtomicUsize::new(0) }; BACKGROUND_ROUNDS];
static BACKGROUND_AFTER_GC: [AtomicUsize; BACKGROUND_ROUNDS] =
    [const { AtomicUsize::new(0) }; BACKGROUND_ROUNDS];
static BACKGROUND_CLEANED: [AtomicUsize; BACKGROUND_ROUNDS] =
    [const { AtomicUsize::new(0) }; BACKGROUND_ROUNDS];
static TIMER_COUNTS: [AtomicUsize; TIMER_CYCLES] = [const { AtomicUsize::new(0) }; TIMER_CYCLES];

extern "C-unwind" fn captured_timer(capture: *mut c_void) -> usize {
    // A timer callback's capture is retained by its task control block. Force
    // collections before reading it so this checks the live-root side of the
    // timer lifecycle as well as cancellation below.
    for _ in 0..GC_ALLOCATIONS {
        let allocation = unsafe { zirk_rt_alloc(GC_ALLOCATION_SIZE, 8) };
        assert!(!allocation.is_null());
    }
    let cycle = unsafe { *((capture as *const usize).add(2)) };
    let value = unsafe { *((capture as *const usize).add(3)) };
    assert!(cycle < TIMER_CYCLES);
    assert_eq!(value, TIMER_CAPTURE_VALUE);
    TIMER_COUNTS[cycle].fetch_add(1, Ordering::SeqCst);
    0
}

extern "C-unwind" fn timer_cancellation_driver() {
    let cycle = ROUND.load(Ordering::SeqCst);
    let capture = unsafe { zirk_rt_alloc(4 * std::mem::size_of::<usize>(), 8) };
    assert!(!capture.is_null());
    unsafe {
        *((capture as *mut usize).add(2)) = cycle;
        *((capture as *mut usize).add(3)) = TIMER_CAPTURE_VALUE;
    }

    let scope = unsafe { zirk_rt_scope_enter() };
    let mut status = u32::MAX;
    unsafe {
        zirk_rt_timer_every_checked(scope, 1_000_000, captured_timer, capture, &mut status);
    }
    assert_eq!(status, 0, "timer admission failed in cycle {cycle}");
    // Give the periodic callback a deterministic opportunity to run, then
    // cancel its scope. Scope exit joins timer cleanup before root completion.
    unsafe { zirk_rt_sleep(10_000_000) };
    assert!(TIMER_COUNTS[cycle].load(Ordering::SeqCst) > 0);
    unsafe { zirk_rt_scope_cancel(scope) };
    assert!(unsafe { zirk_rt_scope_exit(scope) });
}

extern "C-unwind" fn captured_background(capture: *mut c_void) -> usize {
    let round = ROUND.load(Ordering::SeqCst) % BACKGROUND_ROUNDS;
    BACKGROUND_STARTED[round].fetch_add(1, Ordering::SeqCst);
    for _ in 0..BACKGROUND_ALLOCATIONS {
        let allocation = unsafe { zirk_rt_alloc(BACKGROUND_ALLOCATION_SIZE, 8) };
        assert!(!allocation.is_null());
    }

    // The generated-code ABI roots the capture block in the task control
    // block. Read its payload only after allocation-triggered GC pressure.
    let value = unsafe { *((capture as *const usize).add(3)) };
    assert_eq!(value, CAPTURED_VALUE);
    BACKGROUND_AFTER_GC[round].fetch_add(1, Ordering::SeqCst);

    // Root completion requests cancellation. Sleep is a cooperative point and
    // returns when the supervisor cancels this background job.
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    BACKGROUND_CLEANED[round].fetch_add(1, Ordering::SeqCst);
    0
}

extern "C-unwind" fn capture_root_driver() {
    let capture = unsafe { zirk_rt_alloc(4 * std::mem::size_of::<usize>(), 8) };
    assert!(!capture.is_null());
    unsafe {
        *((capture as *mut usize).add(3)) = CAPTURED_VALUE;
        assert_eq!(zirk_rt_background_spawn(captured_background, capture), 0);
    }
    // Let the job reach the post-GC checkpoint and cooperative wait before
    // root completion requests cancellation.
    unsafe { zirk_rt_sleep(1_000_000) };
    // Returning from the root starts background cancellation and drain.
}

extern "C-unwind" fn quota_background(_: *mut c_void) -> usize {
    QUOTA_STARTED.fetch_add(1, Ordering::SeqCst);
    while QUOTA_RELEASE.load(Ordering::SeqCst) == 0 {
        unsafe { zirk_rt_sleep(BACKGROUND_POLL_NANOS) };
    }
    QUOTA_COMPLETED.fetch_add(1, Ordering::SeqCst);
    0
}

extern "C-unwind" fn quota_reuse_background(_: *mut c_void) -> usize {
    QUOTA_REUSE_STARTED.fetch_add(1, Ordering::SeqCst);
    QUOTA_REUSE_COMPLETED.fetch_add(1, Ordering::SeqCst);
    0
}

extern "C-unwind" fn background_quota_driver() {
    for _ in 0..BACKGROUND_CAPACITY {
        let status = unsafe { zirk_rt_background_spawn(quota_background, std::ptr::null_mut()) };
        assert_eq!(
            status, 0,
            "one of the first 256 background jobs was rejected"
        );
        QUOTA_ADMITTED.fetch_add(1, Ordering::SeqCst);
    }

    let overflow = unsafe { zirk_rt_background_spawn(quota_background, std::ptr::null_mut()) };
    assert_eq!(
        overflow, 1,
        "the 257th active background job must hit the quota"
    );
    QUOTA_REJECTIONS.fetch_add(1, Ordering::SeqCst);
    assert_eq!(QUOTA_ADMITTED.load(Ordering::SeqCst), BACKGROUND_CAPACITY);
    assert_eq!(QUOTA_REJECTIONS.load(Ordering::SeqCst), 1);

    QUOTA_RELEASE.store(1, Ordering::SeqCst);
    let mut drained = false;
    for _ in 0..BACKGROUND_QUOTA_POLL_LIMIT {
        if QUOTA_COMPLETED.load(Ordering::SeqCst) == BACKGROUND_CAPACITY {
            drained = true;
            break;
        }
        unsafe { zirk_rt_sleep(BACKGROUND_POLL_NANOS) };
    }
    assert!(drained, "the 256 admitted background jobs did not complete");

    // Completion counters are incremented immediately before each thunk
    // returns. Retry cooperatively if the last return has not yet released its
    // supervisor slot; success is the observable public-ABI proof of reuse.
    let mut later_status;
    let mut reused = false;
    for _ in 0..BACKGROUND_QUOTA_POLL_LIMIT {
        later_status =
            unsafe { zirk_rt_background_spawn(quota_reuse_background, std::ptr::null_mut()) };
        if later_status == 0 {
            reused = true;
            break;
        }
        assert_eq!(later_status, 1, "unexpected background admission status");
        unsafe { zirk_rt_sleep(BACKGROUND_POLL_NANOS) };
    }
    assert!(reused, "a completed job did not release its quota slot");

    let mut reuse_completed = false;
    for _ in 0..BACKGROUND_QUOTA_POLL_LIMIT {
        if QUOTA_REUSE_COMPLETED.load(Ordering::SeqCst) == 1 {
            reuse_completed = true;
            break;
        }
        unsafe { zirk_rt_sleep(BACKGROUND_POLL_NANOS) };
    }
    assert!(
        reuse_completed,
        "the re-admitted background job did not finish"
    );
}

static ROUND: AtomicUsize = AtomicUsize::new(0);
static RUNTIME_TEST_LOCK: Mutex<()> = Mutex::new(());
static STARTS: [AtomicUsize; ROUNDS * WORKERS] = [const { AtomicUsize::new(0) }; ROUNDS * WORKERS];
static RETURNS: [AtomicUsize; ROUNDS * WORKERS] = [const { AtomicUsize::new(0) }; ROUNDS * WORKERS];
static WAITERS: [AtomicUsize; ROUNDS * WORKERS] = [const { AtomicUsize::new(0) }; ROUNDS * WORKERS];
static WAITER_RETURNS: [AtomicUsize; ROUNDS * WORKERS] =
    [const { AtomicUsize::new(0) }; ROUNDS * WORKERS];
static WAITER_RESULTS: [AtomicUsize; ROUNDS * WORKERS] =
    [const { AtomicUsize::new(0) }; ROUNDS * WORKERS];
static BARRIER_RETURNS: [AtomicUsize; ROUNDS] = [const { AtomicUsize::new(0) }; ROUNDS];

fn slot(worker: usize) -> usize {
    ROUND.load(Ordering::SeqCst) * WORKERS + worker
}

fn spawn(body: extern "C-unwind" fn(*mut c_void) -> usize, scope: u64) -> u64 {
    let id = unsafe { zirk_rt_spawn(body, std::ptr::null_mut()) };
    unsafe { zirk_rt_branch_register(scope, id) };
    id
}

fn park(worker: usize) -> usize {
    let index = slot(worker);
    STARTS[index].fetch_add(1, Ordering::SeqCst);
    // Cancellation wakes this cooperative wait. The long deadline is only a
    // fallback; no assertion relies on elapsed wall-clock time.
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    RETURNS[index].fetch_add(1, Ordering::SeqCst);
    0xcafe_0000 + worker
}

macro_rules! worker_thunk {
    ($name:ident, $index:expr) => {
        extern "C-unwind" fn $name(_: *mut c_void) -> usize {
            park($index)
        }
    };
}
worker_thunk!(worker_0, 0);
worker_thunk!(worker_1, 1);
worker_thunk!(worker_2, 2);
worker_thunk!(worker_3, 3);
const WORKER_BODIES: [extern "C-unwind" fn(*mut c_void) -> usize; WORKERS] =
    [worker_0, worker_1, worker_2, worker_3];

extern "C-unwind" fn started_barrier(_: *mut c_void) -> usize {
    let round = ROUND.load(Ordering::SeqCst);
    // Cooperative predicate polling establishes that every worker reached its
    // cancellation point before the driver starts canceling.
    for _ in 0..100_000 {
        if (0..WORKERS).all(|worker| STARTS[round * WORKERS + worker].load(Ordering::SeqCst) == 1) {
            BARRIER_RETURNS[round].fetch_add(1, Ordering::SeqCst);
            return WORKERS;
        }
        unsafe { zirk_rt_sleep(1_000_000) };
    }
    panic!("workers did not reach the cancellation point within the bounded poll budget");
}

fn waiter(worker: usize) -> usize {
    let index = slot(worker);
    WAITERS[index].fetch_add(1, Ordering::SeqCst);
    // The task id is supplied as an integer-sized opaque capture, not a GC
    // pointer. The driver writes it before this task can run.
    let target = WAITER_TARGETS[index].load(Ordering::SeqCst) as u64;
    let result = catch_unwind(AssertUnwindSafe(|| unsafe { zirk_rt_job_wait(target) }));
    let observed = result.unwrap_or(CANCELLED);
    WAITER_RESULTS[index].store(observed, Ordering::SeqCst);
    WAITER_RETURNS[index].fetch_add(1, Ordering::SeqCst);
    observed
}

static WAITER_TARGETS: [AtomicUsize; ROUNDS * WORKERS] =
    [const { AtomicUsize::new(0) }; ROUNDS * WORKERS];

fn allocate_gc_pressure() {
    for _ in 0..GC_ALLOCATIONS {
        let pointer = unsafe { zirk_rt_alloc(GC_ALLOCATION_SIZE, 8) };
        assert!(!pointer.is_null());
    }
}

extern "C-unwind" fn driver() {
    for round in 0..ROUNDS {
        ROUND.store(round, Ordering::SeqCst);
        let scope = unsafe { zirk_rt_scope_enter() };
        let mut targets = [0_u64; WORKERS];
        let mut waiters = [0_u64; WORKERS];

        for worker in 0..WORKERS {
            targets[worker] = spawn(WORKER_BODIES[worker], scope);
        }
        let barrier = spawn(started_barrier, scope);
        assert_eq!(unsafe { zirk_rt_job_wait(barrier) }, WORKERS);
        assert_eq!(BARRIER_RETURNS[round].load(Ordering::SeqCst), 1);

        for worker in 0..WORKERS {
            let index = round * WORKERS + worker;
            WAITER_TARGETS[index].store(targets[worker] as usize, Ordering::SeqCst);
            // A tiny ABI trampoline passes the worker index to the waiter.
            waiters[worker] = spawn(WAITER_BODIES[worker], scope);
        }

        // Interleave allocation-triggered collections with a fixed cancel
        // order. Each batch exceeds the collector's default 1 MiB threshold.
        for worker in 0..WORKERS {
            allocate_gc_pressure();
            unsafe { zirk_rt_cancel(targets[(worker + round) % WORKERS]) };
        }
        allocate_gc_pressure();

        // Wait for each waiter to consume its target outcome, then observe the
        // terminal task states before scope exit. The scope itself joins all
        // registered children before the executor can reach shutdown.
        for (worker, waiter_id) in waiters.iter().enumerate() {
            let result = unsafe { zirk_rt_job_wait(*waiter_id) };
            assert_eq!(result, CANCELLED);
            let index = round * WORKERS + worker;
            assert_eq!(WAITER_RESULTS[index].load(Ordering::SeqCst), CANCELLED);
            assert_eq!(WAITERS[index].load(Ordering::SeqCst), 1);
            assert_eq!(WAITER_RETURNS[index].load(Ordering::SeqCst), 1);
        }
        for worker in 0..WORKERS {
            let index = round * WORKERS + worker;
            assert_eq!(STARTS[index].load(Ordering::SeqCst), 1);
            // Cancellation is cooperative: sleep returns to generated cleanup
            // code, then the task is normalized to Cancelled when it finishes.
            assert_eq!(RETURNS[index].load(Ordering::SeqCst), 1);
        }
        assert!(unsafe { zirk_rt_scope_exit(scope) });
    }
}

macro_rules! waiter_thunk {
    ($name:ident, $index:expr) => {
        extern "C-unwind" fn $name(_: *mut c_void) -> usize {
            waiter($index)
        }
    };
}
waiter_thunk!(waiter_0, 0);
waiter_thunk!(waiter_1, 1);
waiter_thunk!(waiter_2, 2);
waiter_thunk!(waiter_3, 3);
const WAITER_BODIES: [extern "C-unwind" fn(*mut c_void) -> usize; WORKERS] =
    [waiter_0, waiter_1, waiter_2, waiter_3];

#[test]
fn cancellation_and_shutdown_drain_under_gc_pressure() {
    let _runtime_guard = RUNTIME_TEST_LOCK.lock().unwrap();
    unsafe { zirk_rt_init() };
    unsafe { zirk_rt_run_main(driver) };
    // The supported lifecycle calls shutdown only after run_main has joined
    // the root and all structured children.
    unsafe { zirk_rt_shutdown() };

    for (round, barrier_return) in BARRIER_RETURNS.iter().enumerate() {
        assert_eq!(barrier_return.load(Ordering::SeqCst), 1);
        for worker in 0..WORKERS {
            let index = round * WORKERS + worker;
            assert_eq!(STARTS[index].load(Ordering::SeqCst), 1);
            assert_eq!(RETURNS[index].load(Ordering::SeqCst), 1);
            assert_eq!(WAITERS[index].load(Ordering::SeqCst), 1);
            assert_eq!(WAITER_RETURNS[index].load(Ordering::SeqCst), 1);
            assert_eq!(WAITER_RESULTS[index].load(Ordering::SeqCst), CANCELLED);
        }
    }
}

#[test]
fn captured_background_root_survives_gc_pressure_and_root_completion_drains_cancellation() {
    let _runtime_guard = RUNTIME_TEST_LOCK.lock().unwrap();
    unsafe { zirk_rt_init() };
    for round in 0..BACKGROUND_ROUNDS {
        ROUND.store(round, Ordering::SeqCst);
        unsafe { zirk_rt_run_main(capture_root_driver) };
        assert_eq!(BACKGROUND_STARTED[round].load(Ordering::SeqCst), 1);
        assert_eq!(BACKGROUND_AFTER_GC[round].load(Ordering::SeqCst), 1);
        assert_eq!(
            BACKGROUND_CLEANED[round].load(Ordering::SeqCst),
            1,
            "root completion returned before cooperative background cancellation drained"
        );
    }
    unsafe { zirk_rt_shutdown() };
}

#[test]
fn public_background_abi_enforces_quota_and_reuses_completed_slots() {
    let _runtime_guard = RUNTIME_TEST_LOCK.lock().unwrap();
    QUOTA_RELEASE.store(0, Ordering::SeqCst);
    QUOTA_STARTED.store(0, Ordering::SeqCst);
    QUOTA_COMPLETED.store(0, Ordering::SeqCst);
    QUOTA_ADMITTED.store(0, Ordering::SeqCst);
    QUOTA_REJECTIONS.store(0, Ordering::SeqCst);
    QUOTA_REUSE_STARTED.store(0, Ordering::SeqCst);
    QUOTA_REUSE_COMPLETED.store(0, Ordering::SeqCst);

    unsafe { zirk_rt_init() };
    unsafe { zirk_rt_run_main(background_quota_driver) };
    unsafe { zirk_rt_shutdown() };

    assert_eq!(QUOTA_ADMITTED.load(Ordering::SeqCst), BACKGROUND_CAPACITY);
    assert_eq!(QUOTA_STARTED.load(Ordering::SeqCst), BACKGROUND_CAPACITY);
    assert_eq!(QUOTA_COMPLETED.load(Ordering::SeqCst), BACKGROUND_CAPACITY);
    assert_eq!(QUOTA_REJECTIONS.load(Ordering::SeqCst), 1);
    assert_eq!(QUOTA_REUSE_STARTED.load(Ordering::SeqCst), 1);
    assert_eq!(QUOTA_REUSE_COMPLETED.load(Ordering::SeqCst), 1);
}

#[test]
fn repeated_timer_cancellation_keeps_capture_root_live_then_drains_before_shutdown() {
    let _runtime_guard = RUNTIME_TEST_LOCK.lock().unwrap();
    unsafe { zirk_rt_init() };
    for (cycle, timer_count) in TIMER_COUNTS.iter().enumerate() {
        ROUND.store(cycle, Ordering::SeqCst);
        unsafe { zirk_rt_run_main(timer_cancellation_driver) };
        let after_shutdown_boundary = timer_count.load(Ordering::SeqCst);
        assert!(
            after_shutdown_boundary > 0,
            "timer never ran in cycle {cycle}"
        );
        // Each run_main owns a fresh executor. The canceled repeating timer
        // must be terminal before the next lifecycle iteration begins.
        assert_eq!(timer_count.load(Ordering::SeqCst), after_shutdown_boundary);
    }
    unsafe { zirk_rt_shutdown() };
}
