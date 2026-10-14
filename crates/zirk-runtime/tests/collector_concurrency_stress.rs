//! Public-ABI stress coverage for parallel pool participation during GC churn.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use zirk_runtime::zirk_rt_run_main;

const CYCLES: usize = 6;
const ITEMS_PER_CYCLE: usize = 1024;
const ALLOCATION_BYTES: usize = 4096;

static VISITS: [AtomicUsize; CYCLES * ITEMS_PER_CYCLE] =
    [const { AtomicUsize::new(0) }; CYCLES * ITEMS_PER_CYCLE];
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static CURRENT_CYCLE: AtomicUsize = AtomicUsize::new(0);

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

extern "C-unwind" fn allocate_and_record(_: *mut c_void, index: i64) -> usize {
    // Each allocation has no pointer fields and is deliberately discarded, so
    // the public allocator drives reclamation without test-only collector APIs.
    let allocation = unsafe { zirk_runtime::zirk_rt_alloc(ALLOCATION_BYTES, 8) };
    std::hint::black_box(allocation);
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    let cycle = CURRENT_CYCLE.load(Ordering::Relaxed);
    VISITS[cycle * ITEMS_PER_CYCLE + index as usize].fetch_add(1, Ordering::Relaxed);
    0
}

extern "C-unwind" fn stress_driver() {
    for cycle in 0..CYCLES {
        CURRENT_CYCLE.store(cycle, Ordering::Relaxed);
        // SAFETY: this generated-code ABI call is made from the executor root;
        // the callback is static, receives disjoint indices, and retains no
        // capture pointer. Every region joins before this call returns.
        unsafe {
            parallel_for(
                1,
                4,
                0,
                ITEMS_PER_CYCLE as i64,
                allocate_and_record,
                std::ptr::null_mut(),
            )
        };
    }
}

#[test]
fn parallel_for_runs_each_index_once_across_gc_allocation_pressure_cycles() {
    // Keep worker admission deterministic across hosts while using the pool
    // path (the runtime clamps this request to its configured pool capacity).
    // SAFETY: this integration-test process has one test and sets the variable
    // before the runtime lazily creates its process-wide worker pool.
    unsafe { std::env::set_var("ZIRK_PARALLEL_CORES", "4") };
    // Force allocation pressure to cross the collector threshold immediately
    // so the region repeatedly coordinates worker safepoints with collection.
    unsafe { std::env::set_var("ZIRK_GC_THRESHOLD", "1") };

    // SAFETY: the callback is a valid no-argument runtime entrypoint and each
    // parallel region synchronously joins all callbacks before returning.
    unsafe { zirk_rt_run_main(stress_driver) };

    for (position, visits) in VISITS.iter().enumerate() {
        assert_eq!(
            visits.load(Ordering::Relaxed),
            1,
            "cycle {} index {} did not execute exactly once",
            position / ITEMS_PER_CYCLE,
            position % ITEMS_PER_CYCLE
        );
    }
    assert_eq!(
        ALLOCATIONS.load(Ordering::Relaxed),
        CYCLES * ITEMS_PER_CYCLE,
        "every parallel iteration must allocate under GC pressure"
    );
}
