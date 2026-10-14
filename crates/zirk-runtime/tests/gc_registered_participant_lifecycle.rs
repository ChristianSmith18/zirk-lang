//! Public-API regression for GC pressure while executor and parallel-pool
//! participants move through the lifecycle states currently exposed by the
//! runtime. This external integration test cannot exercise `InNative`: the
//! registration and blocking-native guard are `pub(crate)` and are only safe
//! when native code neither touches managed values nor calls back into Zirk.
//! The deterministic internal regression
//! `collector::stw::tests::blocked_native_mutator_keeps_stable_roots_visible_to_collection`
//! covers that transition, checks that collection completes while the
//! participant remains `InNative`, and verifies its stable root chain remains
//! registered. This test separately covers public executor/pool lifecycle
//! behavior under allocation pressure.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_branch_register, zirk_rt_cancel, zirk_rt_init, zirk_rt_run_main,
    zirk_rt_scope_enter, zirk_rt_scope_exit, zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_spawn,
};

const PARTICIPANTS: usize = 4;
const PARALLEL_ITEMS: usize = 4096;
const ALLOCATION_BYTES: usize = 4096;
const LONG_SLEEP_NANOS: i64 = 86_400_000_000_000;
const START_POLL_NANOS: i64 = 1_000_000;
const START_POLL_LIMIT: usize = 10_000;

static VISITS: [AtomicUsize; PARALLEL_ITEMS] = [const { AtomicUsize::new(0) }; PARALLEL_ITEMS];
static CANCELLED_WORKERS_STARTED: AtomicUsize = AtomicUsize::new(0);
static CANCELLED_WORKERS_CLEANED: AtomicUsize = AtomicUsize::new(0);

struct CleanupCounter;

impl Drop for CleanupCounter {
    fn drop(&mut self) {
        CANCELLED_WORKERS_CLEANED.fetch_add(1, Ordering::SeqCst);
    }
}

unsafe extern "C" {
    #[link_name = "zirk_rt_parallel_for"]
    fn parallel_for(
        kind: u8,
        a: i64,
        b: i64,
        count: i64,
        body: extern "C-unwind" fn(*mut c_void, i64) -> usize,
        capture: *mut c_void,
    );
}

extern "C-unwind" fn allocate_in_parallel(_: *mut c_void, index: i64) -> usize {
    // Allocations are intentionally dead. They force repeated collections in
    // registered pool workers without creating untracked managed references.
    let allocation = unsafe { zirk_rt_alloc(ALLOCATION_BYTES, 8) };
    assert!(!allocation.is_null());
    std::hint::black_box(allocation);
    VISITS[index as usize].fetch_add(1, Ordering::Relaxed);
    0
}

extern "C-unwind" fn cancellable_parked_worker(_: *mut c_void) -> usize {
    CANCELLED_WORKERS_STARTED.fetch_add(1, Ordering::SeqCst);
    let _cleanup = CleanupCounter;
    // Cancellation wakes this executor park. The deadline is only a fallback.
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    0
}

extern "C-unwind" fn lifecycle_driver() {
    // Public parallel-region ABI registers pool participants; allocations in
    // callbacks trigger collection and each iteration reaches a safepoint.
    unsafe {
        parallel_for(
            1,
            PARTICIPANTS as i64,
            0,
            PARALLEL_ITEMS as i64,
            allocate_in_parallel,
            std::ptr::null_mut(),
        );
    }

    let scope = unsafe { zirk_rt_scope_enter() };
    let mut workers = [0_u64; 2];
    for worker in &mut workers {
        *worker = unsafe { zirk_rt_spawn(cancellable_parked_worker, std::ptr::null_mut()) };
        unsafe { zirk_rt_branch_register(scope, *worker) };
    }

    let mut all_started = false;
    for _ in 0..START_POLL_LIMIT {
        if CANCELLED_WORKERS_STARTED.load(Ordering::SeqCst) == workers.len() {
            all_started = true;
            break;
        }
        unsafe { zirk_rt_sleep(START_POLL_NANOS) };
    }
    assert!(all_started, "executor workers did not reach their park");

    // Apply pressure while both task participants are suspended, then cancel
    // and join them through the public scope ABI.
    for _ in 0..128 {
        let allocation = unsafe { zirk_rt_alloc(ALLOCATION_BYTES, 8) };
        assert!(!allocation.is_null());
    }
    for worker in workers {
        unsafe { zirk_rt_cancel(worker) };
    }
    assert!(unsafe { zirk_rt_scope_exit(scope) });
}

#[test]
fn registered_participants_survive_gc_pressure_through_park_cancel_join_shutdown() {
    // This integration-test executable contains one test, so these variables
    // are set before lazy initialization of the process-wide parallel pool.
    unsafe {
        std::env::set_var("ZIRK_PARALLEL_CORES", PARTICIPANTS.to_string());
        std::env::set_var("ZIRK_GC_THRESHOLD", "1");
    }

    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(lifecycle_driver);
        zirk_rt_shutdown();
    }

    for (index, visits) in VISITS.iter().enumerate() {
        assert_eq!(
            visits.load(Ordering::Relaxed),
            1,
            "parallel index {index} did not complete exactly once"
        );
    }
    assert_eq!(
        CANCELLED_WORKERS_STARTED.load(Ordering::SeqCst),
        2,
        "both executor participants must reach their cooperative park"
    );
    assert_eq!(
        CANCELLED_WORKERS_CLEANED.load(Ordering::SeqCst),
        2,
        "scope exit must wait for cancellation cleanup"
    );
}
