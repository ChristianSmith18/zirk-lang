//! Exercises the runtime-only outcome-transfer ABI and its GC root contract.

use std::ffi::c_void;
use std::process::{Command, Stdio};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_branch_fail, zirk_rt_init, zirk_rt_job_outcome_take,
    zirk_rt_job_outcome_wait_take, zirk_rt_job_wait, zirk_rt_pop_frame, zirk_rt_push_frame,
    zirk_rt_run_main, zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_spawn,
};

const FULFILLED: u32 = 0;
const REJECTED: u32 = 1;
const CANCELLED: u32 = 2;
const CALLER_CANCELLED: u32 = 3;
const LONG_SLEEP_NANOS: i64 = 86_400_000_000_000;

extern "C-unwind" fn return_capture(capture: *mut c_void) -> usize {
    capture as usize
}

extern "C-unwind" fn return_allocated_after_yield(_: *mut c_void) -> usize {
    let object = unsafe { zirk_rt_alloc(64, 8) };
    unsafe { *((object as *mut usize).add(3)) = 0x0ddc_0ffe };
    let mut rooted = object;
    let mut roots = [(&mut rooted as *mut *mut c_void).cast()];
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
    unsafe { zirk_rt_sleep(0) };
    zirk_rt_pop_frame();
    object as usize
}

extern "C-unwind" fn allocation_pressure(_: *mut c_void) -> usize {
    unsafe { zirk_rt_alloc(1 << 20, 8) as usize }
}

extern "C-unwind" fn reject_allocated(_: *mut c_void) -> usize {
    let object = unsafe { zirk_rt_alloc(64, 8) };
    unsafe { *((object as *mut usize).add(3)) = 0x0ddc_0ffe };
    unsafe { zirk_rt_branch_fail(object) }
}

extern "C-unwind" fn reject_null(_: *mut c_void) -> usize {
    unsafe { zirk_rt_branch_fail(std::ptr::null()) }
}

extern "C-unwind" fn wait_for_cancellation(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    0
}

extern "C-unwind" fn native_panic(_: *mut c_void) -> usize {
    panic!("native runtime panic payload")
}

fn push_out_roots(
    value: &mut *mut c_void,
    throwable: &mut *mut c_void,
    addresses: &mut [*mut c_void; 2],
) {
    *addresses = [
        (value as *mut *mut c_void).cast(),
        (throwable as *mut *mut c_void).cast(),
    ];
    unsafe { zirk_rt_push_frame(addresses.as_mut_ptr(), addresses.len() as i64) };
}

extern "C-unwind" fn protocol_driver() {
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut addresses = [
        (&mut value as *mut *mut c_void).cast(),
        (&mut throwable as *mut *mut c_void).cast(),
    ];
    unsafe { zirk_rt_push_frame(addresses.as_mut_ptr(), addresses.len() as i64) };

    // The returned allocation is rooted first by its TCB, then atomically
    // transferred into `value`; the next allocation crosses the default GC
    // threshold and proves that output slot is now the only root.
    let fulfilled = unsafe { zirk_rt_spawn(return_allocated_after_yield, std::ptr::null_mut()) };
    let _pressure = unsafe { zirk_rt_spawn(allocation_pressure, std::ptr::null_mut()) };
    throwable = std::ptr::dangling_mut::<c_void>();
    assert_eq!(
        unsafe { zirk_rt_job_outcome_wait_take(fulfilled, &mut value, &mut throwable) },
        FULFILLED
    );
    assert!(!value.is_null());
    assert!(throwable.is_null());
    let _pressure = unsafe { zirk_rt_alloc(1 << 20, 8) };
    assert_eq!(unsafe { *((value as *mut usize).add(3)) }, 0x0ddc_0ffe);
    zirk_rt_pop_frame();

    // Rejection transfers only an actual BranchFailure Throwable pointer.
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut addresses = [std::ptr::null_mut(); 2];
    push_out_roots(&mut value, &mut throwable, &mut addresses);
    let rejected = unsafe { zirk_rt_spawn(reject_allocated, std::ptr::null_mut()) };
    unsafe { zirk_rt_sleep(0) };
    value = std::ptr::dangling_mut::<c_void>();
    assert_eq!(
        unsafe { zirk_rt_job_outcome_wait_take(rejected, &mut value, &mut throwable) },
        REJECTED
    );
    assert!(value.is_null());
    assert!(!throwable.is_null());
    let _pressure = unsafe { zirk_rt_alloc(1 << 20, 8) };
    assert_eq!(unsafe { *((throwable as *mut usize).add(3)) }, 0x0ddc_0ffe);
    zirk_rt_pop_frame();

    // A cooperative cancellation is distinct from both fulfillment and
    // language rejection, and writes null to both rooted slots.
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut addresses = [std::ptr::null_mut(); 2];
    push_out_roots(&mut value, &mut throwable, &mut addresses);
    let cancelled = unsafe { zirk_rt_spawn(wait_for_cancellation, std::ptr::null_mut()) };
    unsafe {
        zirk_rt_sleep(0);
        zirk_runtime::zirk_rt_cancel(cancelled);
        zirk_rt_sleep(0);
    }
    assert_eq!(
        unsafe { zirk_rt_job_outcome_wait_take(cancelled, &mut value, &mut throwable) },
        CANCELLED
    );
    assert!(value.is_null());
    assert!(throwable.is_null());
    zirk_rt_pop_frame();
}

static CALLER_WAIT_STATUS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(u32::MAX);

extern "C-unwind" fn cancelled_waiter(target: *mut c_void) -> usize {
    let target = target as usize as u64;
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut addresses = [
        (&mut value as *mut *mut c_void).cast(),
        (&mut throwable as *mut *mut c_void).cast(),
    ];
    unsafe { zirk_rt_push_frame(addresses.as_mut_ptr(), addresses.len() as i64) };
    let status = unsafe { zirk_rt_job_outcome_wait_take(target, &mut value, &mut throwable) };
    CALLER_WAIT_STATUS.store(status, std::sync::atomic::Ordering::SeqCst);
    zirk_rt_pop_frame();
    0
}

extern "C-unwind" fn caller_cancellation_driver() {
    CALLER_WAIT_STATUS.store(u32::MAX, std::sync::atomic::Ordering::SeqCst);
    let target = unsafe { zirk_rt_spawn(wait_for_cancellation, std::ptr::null_mut()) };
    let waiter = unsafe { zirk_rt_spawn(cancelled_waiter, target as usize as *mut c_void) };
    unsafe {
        zirk_rt_sleep(0);
        zirk_runtime::zirk_rt_cancel(waiter);
        let _ = std::panic::catch_unwind(|| zirk_rt_job_wait(waiter));
        zirk_runtime::zirk_rt_cancel(target);
    }
    assert_eq!(
        CALLER_WAIT_STATUS.load(std::sync::atomic::Ordering::SeqCst),
        CALLER_CANCELLED,
        "cancelling the waiter must return the caller-cancelled status"
    );
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut addresses = [
        (&mut value as *mut *mut c_void).cast(),
        (&mut throwable as *mut *mut c_void).cast(),
    ];
    unsafe { zirk_rt_push_frame(addresses.as_mut_ptr(), addresses.len() as i64) };
    assert_eq!(
        unsafe { zirk_rt_job_outcome_wait_take(target, &mut value, &mut throwable) },
        CANCELLED,
        "the target outcome must remain consumable after caller cancellation"
    );
    zirk_rt_pop_frame();
}

#[test]
fn job_outcome_wait_take_preserves_caller_cancellation_and_target_result() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(caller_cancellation_driver);
        zirk_rt_shutdown();
    }
}

#[test]
fn job_outcome_take_transfers_status_and_gc_roots() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(protocol_driver);
        zirk_rt_shutdown();
    }
}

extern "C-unwind" fn misuse_driver() {
    let case = std::env::var("ZIRK_JOB_OUTCOME_TAKE_MISUSE").unwrap();
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut addresses = [std::ptr::null_mut(); 2];
    if case != "unrooted" {
        push_out_roots(&mut value, &mut throwable, &mut addresses);
    }
    let id = match case.as_str() {
        "stale" => u64::MAX,
        "nonterminal" => unsafe { zirk_rt_spawn(wait_for_cancellation, std::ptr::null_mut()) },
        "consumed" => {
            let id = unsafe { zirk_rt_spawn(return_capture, std::ptr::null_mut()) };
            unsafe {
                zirk_rt_sleep(0);
                let _ = zirk_rt_job_outcome_take(id, &mut value, &mut throwable);
            }
            id
        }
        "native_panic" => {
            let id = unsafe { zirk_rt_spawn(native_panic, std::ptr::null_mut()) };
            unsafe { zirk_rt_sleep(0) };
            id
        }
        "null_throwable" => {
            let id = unsafe { zirk_rt_spawn(reject_null, std::ptr::null_mut()) };
            unsafe { zirk_rt_sleep(0) };
            id
        }
        "unrooted" => {
            let id = unsafe { zirk_rt_spawn(return_capture, std::ptr::null_mut()) };
            unsafe { zirk_rt_sleep(0) };
            id
        }
        other => panic!("unknown misuse case: {other}"),
    };
    unsafe { zirk_rt_job_outcome_wait_take(id, &mut value, &mut throwable) };
    panic!("misuse case {case} unexpectedly returned");
}

#[test]
fn job_outcome_take_rejects_misuse_and_native_panics_fail_closed() {
    if std::env::var_os("ZIRK_JOB_OUTCOME_TAKE_MISUSE").is_some() {
        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(misuse_driver);
        }
        panic!("misuse unexpectedly returned");
    }

    for (case, diagnostic) in [
        ("stale", "task ID that is not live"),
        ("consumed", "already-consumed task"),
        ("native_panic", "internal native panic"),
        (
            "null_throwable",
            "branch failure requires a non-null Throwable",
        ),
        ("unrooted", "unrooted output slots"),
    ] {
        let output = Command::new(std::env::current_exe().expect("test executable path"))
            .args([
                "--exact",
                "job_outcome_take_rejects_misuse_and_native_panics_fail_closed",
                "--nocapture",
            ])
            .env("ZIRK_JOB_OUTCOME_TAKE_MISUSE", case)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run misuse case in isolated process");
        assert_eq!(
            output.status.code(),
            Some(70),
            "{case} did not fail closed; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "missing {case} diagnostic: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
