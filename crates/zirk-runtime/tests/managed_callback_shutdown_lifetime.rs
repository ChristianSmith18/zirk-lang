//! A managed native callback must finish before run_main can release the
//! executor heap and the caller task stack that its ABI slots refer to.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use zirk_runtime::{
    MANAGED_CALL_OK, zirk_rt_alloc, zirk_rt_app_set_shutdown_grace, zirk_rt_background_spawn,
    zirk_rt_init, zirk_rt_pop_frame, zirk_rt_push_frame, zirk_rt_run_main, zirk_rt_safepoint_poll,
    zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_thread_run,
};

static CALLBACK_STARTED: AtomicBool = AtomicBool::new(false);
static ROOT_FINISHING: AtomicBool = AtomicBool::new(false);
static CALLBACK_RELEASE: AtomicBool = AtomicBool::new(false);
static CALLBACK_FINISHED: AtomicBool = AtomicBool::new(false);
const GC_SENTINEL: u64 = 0x071a_20b1_e0c7;

unsafe extern "C-unwind" fn held_managed_callback(
    _target: *mut c_void,
    _captures: *mut c_void,
    result: *mut c_void,
    _exception: *mut c_void,
) -> u32 {
    let mut managed_object = unsafe { zirk_rt_alloc(32, 8) };
    let mut object_roots = [(&mut managed_object as *mut *mut c_void).cast()];
    unsafe { zirk_rt_push_frame(object_roots.as_mut_ptr(), 1) };
    let payload = unsafe {
        managed_object
            .cast::<u8>()
            .add(3 * std::mem::size_of::<usize>())
    };
    unsafe { payload.cast::<u64>().write(GC_SENTINEL) };
    CALLBACK_STARTED.store(true, Ordering::Release);
    while !CALLBACK_RELEASE.load(Ordering::Acquire) {
        unsafe { zirk_rt_safepoint_poll() };
        thread::sleep(Duration::from_millis(1));
    }
    // This slot belongs to the suspended caller's native stack. Writing a GC
    // object after pressure and grace expiry proves that both its root and
    // the caller frame stayed alive until the callback joined.
    unsafe { *result.cast::<usize>() = managed_object as usize };
    zirk_rt_pop_frame();
    CALLBACK_FINISHED.store(true, Ordering::Release);
    MANAGED_CALL_OK
}

extern "C-unwind" fn background_thread_caller(_: *mut c_void) -> usize {
    let mut result = 0usize;
    let mut exception: *mut c_void = std::ptr::null_mut();
    let mut roots = [
        (&mut result as *mut usize).cast(),
        (&mut exception as *mut *mut c_void).cast(),
    ];
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    let status = unsafe {
        zirk_rt_thread_run(
            held_managed_callback,
            (&CALLBACK_STARTED as *const AtomicBool).cast_mut().cast(),
            std::ptr::null_mut(),
            (&mut result as *mut usize).cast(),
            (&mut exception as *mut *mut c_void).cast(),
        )
    };
    assert_eq!(status, MANAGED_CALL_OK);
    assert!(exception.is_null());
    let object = result as *const u8;
    let payload = unsafe {
        object
            .add(3 * std::mem::size_of::<usize>())
            .cast::<u64>()
            .read()
    };
    assert_eq!(payload, GC_SENTINEL);
    zirk_rt_pop_frame();
    result
}

extern "C-unwind" fn root_returns_while_managed_callback_is_held() {
    assert_eq!(unsafe { zirk_rt_app_set_shutdown_grace(1_000_000) }, 0);
    assert_eq!(
        unsafe { zirk_rt_background_spawn(background_thread_caller, std::ptr::null_mut()) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    while !CALLBACK_STARTED.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline, "managed callback did not start");
        unsafe { zirk_rt_sleep(1_000_000) };
    }
    // Trigger real collections while the callback is parked in native code.
    // Its registered worker root must keep the object alive until result
    // publication into the caller's rooted slot.
    for _ in 0..32 {
        let garbage = unsafe { zirk_rt_alloc(32, 8) };
        assert!(!garbage.is_null());
    }
    ROOT_FINISHING.store(true, Ordering::Release);
}

#[test]
fn run_main_keeps_heap_and_caller_stack_until_managed_callback_joins() {
    CALLBACK_STARTED.store(false, Ordering::Release);
    ROOT_FINISHING.store(false, Ordering::Release);
    CALLBACK_RELEASE.store(false, Ordering::Release);
    CALLBACK_FINISHED.store(false, Ordering::Release);

    let release = thread::spawn(|| {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !ROOT_FINISHING.load(Ordering::Acquire) {
            assert!(
                Instant::now() < deadline,
                "root did not finish its GC pressure phase"
            );
            thread::sleep(Duration::from_millis(1));
        }
        // Ensure the executor's one-millisecond shutdown grace expires while
        // the worker still owns the raw result/exception slot addresses.
        thread::sleep(Duration::from_millis(50));
        CALLBACK_RELEASE.store(true, Ordering::Release);
    });

    unsafe { std::env::set_var("ZIRK_GC_THRESHOLD", "1") };
    unsafe { zirk_rt_init() };
    unsafe { zirk_rt_run_main(root_returns_while_managed_callback_is_held) };
    let callback_finished_before_run_main_returned = CALLBACK_FINISHED.load(Ordering::Acquire);
    unsafe { zirk_rt_shutdown() };
    release.join().unwrap();

    assert!(
        callback_finished_before_run_main_returned,
        "run_main returned while a managed worker still retained caller stack slots"
    );
}
