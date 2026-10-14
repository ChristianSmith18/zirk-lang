//! Managed roots held by parallel workers must remain visible while other
//! workers drive stop-the-world collections under allocation pressure.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_branch_register, zirk_rt_cancel, zirk_rt_init, zirk_rt_pop_frame,
    zirk_rt_push_frame, zirk_rt_run_main, zirk_rt_scope_enter, zirk_rt_scope_exit,
    zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_spawn,
};

const WORKERS: usize = 4;
const ALLOCATIONS_PER_WORKER: usize = 2048;
static COMPLETED: AtomicUsize = AtomicUsize::new(0);
static ROOT_FAILURES: AtomicUsize = AtomicUsize::new(0);
static NULL_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ROOT_MISMATCHES: AtomicUsize = AtomicUsize::new(0);
static PARKED_WORKER_STARTED: AtomicUsize = AtomicUsize::new(0);
static PARKED_WORKER_ROOT_VALID: AtomicUsize = AtomicUsize::new(0);
const LONG_SLEEP_NANOS: i64 = 86_400_000_000_000;
const POLL_NANOS: i64 = 1_000_000;
const POLL_LIMIT: usize = 10_000;

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

extern "C-unwind" fn allocate_while_rooted(_: *mut c_void, index: i64) -> usize {
    let object = unsafe { zirk_rt_alloc(40, 8) };
    if object.is_null() {
        ROOT_FAILURES.fetch_add(1, Ordering::SeqCst);
        return 1;
    }

    let payload = unsafe { object.cast::<u8>().add(3 * std::mem::size_of::<usize>()) };
    unsafe {
        payload
            .cast::<u64>()
            .write(0x5a00_0000_0000_0000 | index as u64)
    };
    let mut object_slot = object;
    let mut roots = [&mut object_slot as *mut *mut c_void as *mut c_void];
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };

    for _ in 0..ALLOCATIONS_PER_WORKER {
        let garbage = unsafe { zirk_rt_alloc(32, 8) };
        if garbage.is_null() {
            NULL_ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
            ROOT_FAILURES.fetch_add(1, Ordering::SeqCst);
            zirk_rt_pop_frame();
            return 1;
        }
        if unsafe {
            object_slot
                .cast::<u8>()
                .add(3 * std::mem::size_of::<usize>())
                .cast::<u64>()
                .read()
        } != (0x5a00_0000_0000_0000 | index as u64)
        {
            ROOT_MISMATCHES.fetch_add(1, Ordering::SeqCst);
            ROOT_FAILURES.fetch_add(1, Ordering::SeqCst);
            zirk_rt_pop_frame();
            return 1;
        }
        std::hint::black_box(garbage);
    }

    zirk_rt_pop_frame();
    COMPLETED.fetch_add(1, Ordering::SeqCst);
    0
}

extern "C-unwind" fn run_root_stress() {
    unsafe {
        parallel_for(
            1,
            WORKERS as i64,
            0,
            WORKERS as i64,
            allocate_while_rooted,
            std::ptr::null_mut(),
        );
    }

    let scope = unsafe { zirk_rt_scope_enter() };
    let worker = unsafe { zirk_rt_spawn(rooted_cancellable_worker, std::ptr::null_mut()) };
    unsafe { zirk_rt_branch_register(scope, worker) };

    let mut started = false;
    for _ in 0..POLL_LIMIT {
        if PARKED_WORKER_STARTED.load(Ordering::SeqCst) == 1 {
            started = true;
            break;
        }
        unsafe { zirk_rt_sleep(POLL_NANOS) };
    }
    assert!(started, "rooted task did not reach its timer park");

    for _ in 0..128 {
        let garbage = unsafe { zirk_rt_alloc(48, 8) };
        assert!(!garbage.is_null());
    }
    unsafe { zirk_rt_cancel(worker) };
    assert!(unsafe { zirk_rt_scope_exit(scope) });
}

extern "C-unwind" fn rooted_cancellable_worker(_: *mut c_void) -> usize {
    let object = unsafe { zirk_rt_alloc(40, 8) };
    if object.is_null() {
        ROOT_FAILURES.fetch_add(1, Ordering::SeqCst);
        return 1;
    }
    let value = 0x6b00_0000_0000_0001_u64;
    let payload = unsafe { object.cast::<u8>().add(3 * std::mem::size_of::<usize>()) };
    unsafe { payload.cast::<u64>().write(value) };
    let mut object_slot = object;
    let mut roots = [&mut object_slot as *mut *mut c_void as *mut c_void];
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    PARKED_WORKER_STARTED.store(1, Ordering::SeqCst);

    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    let observed = unsafe {
        object_slot
            .cast::<u8>()
            .add(3 * std::mem::size_of::<usize>())
            .cast::<u64>()
            .read()
    };
    if observed == value {
        PARKED_WORKER_ROOT_VALID.store(1, Ordering::SeqCst);
    } else {
        ROOT_FAILURES.fetch_add(1, Ordering::SeqCst);
    }
    zirk_rt_pop_frame();
    0
}

#[test]
fn parallel_worker_roots_survive_repeated_real_collections() {
    unsafe {
        std::env::set_var("ZIRK_PARALLEL_CORES", WORKERS.to_string());
        std::env::set_var("ZIRK_GC_THRESHOLD", "1");
        zirk_rt_init();
        zirk_rt_run_main(run_root_stress);
        zirk_rt_shutdown();
    }

    assert_eq!(
        ROOT_FAILURES.load(Ordering::SeqCst),
        0,
        "null allocations={}, root mismatches={}",
        NULL_ALLOCATIONS.load(Ordering::SeqCst),
        ROOT_MISMATCHES.load(Ordering::SeqCst)
    );
    assert_eq!(COMPLETED.load(Ordering::SeqCst), WORKERS);
    assert_eq!(PARKED_WORKER_ROOT_VALID.load(Ordering::SeqCst), 1);
}
