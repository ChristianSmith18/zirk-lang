//! Public ABI tests for collect-all child scopes.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_branch_fail, zirk_rt_branch_register, zirk_rt_cancel, zirk_rt_init,
    zirk_rt_job_outcome_wait_take, zirk_rt_run_main, zirk_rt_scope_enter,
    zirk_rt_scope_enter_collect_all, zirk_rt_scope_exit, zirk_rt_scope_exit_collect_all,
    zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_spawn,
};

unsafe extern "C" {
    fn zirk_rt_has_pending_exception() -> bool;
    fn zirk_rt_take_pending_exception() -> *const c_void;
}

const FULFILLED: u32 = 0;
const REJECTED: u32 = 1;
const CANCELLED: u32 = 2;
const LONG_SLEEP_NANOS: i64 = 86_400_000_000_000;

static SLOW_CHILD_FINISHED: AtomicBool = AtomicBool::new(false);
static FAIL_FAST_SIBLING_CANCELLED: AtomicBool = AtomicBool::new(false);
static COLLECT_CHILD_CLEANED: AtomicBool = AtomicBool::new(false);

extern "C-unwind" fn early_failure(_: *mut c_void) -> usize {
    let error = unsafe { zirk_rt_alloc(32, 8) };
    unsafe { zirk_rt_branch_fail(error) }
}

extern "C-unwind" fn slow_success(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(2_000_000) };
    SLOW_CHILD_FINISHED.store(true, Ordering::SeqCst);
    41
}

extern "C-unwind" fn cancelled_child(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    7
}

extern "C-unwind" fn collect_child_with_cleanup(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    COLLECT_CHILD_CLEANED.store(true, Ordering::SeqCst);
    9
}

extern "C-unwind" fn cancelled_collect_scope_waiter(_: *mut c_void) -> usize {
    let scope = unsafe { zirk_rt_scope_enter_collect_all() };
    let child = unsafe { zirk_rt_spawn(collect_child_with_cleanup, std::ptr::null_mut()) };
    register(scope, child);
    assert!(unsafe { zirk_rt_scope_exit_collect_all(scope) });
    0
}

extern "C-unwind" fn another_failure(_: *mut c_void) -> usize {
    let error = unsafe { zirk_rt_alloc(32, 8) };
    unsafe { zirk_rt_branch_fail(error) }
}

extern "C-unwind" fn fail_fast_sibling(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(LONG_SLEEP_NANOS) };
    FAIL_FAST_SIBLING_CANCELLED.store(true, Ordering::SeqCst);
    0
}

fn register(scope: u64, child: u64) {
    unsafe { zirk_rt_branch_register(scope, child) };
}

fn assert_outcome(child: u64, expected: u32) -> *mut c_void {
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [
        (&mut value as *mut *mut c_void).cast(),
        (&mut throwable as *mut *mut c_void).cast(),
    ];
    unsafe {
        zirk_runtime::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64);
        let status = zirk_rt_job_outcome_wait_take(child, &mut value, &mut throwable);
        zirk_runtime::zirk_rt_pop_frame();
        assert_eq!(status, expected);
    }
    if expected == REJECTED {
        assert!(!throwable.is_null());
    }
    throwable
}

extern "C-unwind" fn collect_all_driver() {
    SLOW_CHILD_FINISHED.store(false, Ordering::SeqCst);
    let scope = unsafe { zirk_rt_scope_enter_collect_all() };
    let failed = unsafe { zirk_rt_spawn(early_failure, std::ptr::null_mut()) };
    let slow = unsafe { zirk_rt_spawn(slow_success, std::ptr::null_mut()) };
    register(scope, failed);
    register(scope, slow);

    assert!(unsafe { zirk_rt_scope_exit_collect_all(scope) });
    assert!(SLOW_CHILD_FINISHED.load(Ordering::SeqCst));
    assert!(!assert_outcome(failed, REJECTED).is_null());
    assert_eq!(assert_outcome(slow, FULFILLED), std::ptr::null_mut());
}

extern "C-unwind" fn collect_multiple_terminal_states_driver() {
    let scope = unsafe { zirk_rt_scope_enter_collect_all() };
    let failed_a = unsafe { zirk_rt_spawn(early_failure, std::ptr::null_mut()) };
    let failed_b = unsafe { zirk_rt_spawn(another_failure, std::ptr::null_mut()) };
    let cancelled = unsafe { zirk_rt_spawn(cancelled_child, std::ptr::null_mut()) };
    let slow = unsafe { zirk_rt_spawn(slow_success, std::ptr::null_mut()) };
    for child in [failed_a, failed_b, cancelled, slow] {
        register(scope, child);
    }

    // Run both failures and park the cancellation target before requesting
    // cancellation. Collect-all must treat that child as a terminal outcome.
    unsafe {
        zirk_rt_sleep(0);
        zirk_rt_cancel(cancelled);
        assert!(zirk_rt_scope_exit_collect_all(scope));
    }
    assert!(!assert_outcome(failed_a, REJECTED).is_null());
    assert!(!assert_outcome(failed_b, REJECTED).is_null());
    assert_eq!(assert_outcome(cancelled, CANCELLED), std::ptr::null_mut());
    assert_eq!(assert_outcome(slow, FULFILLED), std::ptr::null_mut());
}

extern "C-unwind" fn caller_cancellation_driver() {
    COLLECT_CHILD_CLEANED.store(false, Ordering::SeqCst);
    let waiter = unsafe { zirk_rt_spawn(cancelled_collect_scope_waiter, std::ptr::null_mut()) };
    unsafe { zirk_rt_sleep(0) };
    unsafe { zirk_rt_cancel(waiter) };

    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [
        (&mut value as *mut *mut c_void).cast(),
        (&mut throwable as *mut *mut c_void).cast(),
    ];
    unsafe {
        zirk_runtime::zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64);
        let status = zirk_rt_job_outcome_wait_take(waiter, &mut value, &mut throwable);
        zirk_runtime::zirk_rt_pop_frame();
        assert_eq!(status, CANCELLED);
    }
    assert!(
        COLLECT_CHILD_CLEANED.load(Ordering::SeqCst),
        "caller cancellation must cancel and join children after cleanup"
    );
}

extern "C-unwind" fn fail_fast_driver() {
    FAIL_FAST_SIBLING_CANCELLED.store(false, Ordering::SeqCst);
    let scope = unsafe { zirk_rt_scope_enter() };
    let failed = unsafe { zirk_rt_spawn(early_failure, std::ptr::null_mut()) };
    let sibling = unsafe { zirk_rt_spawn(fail_fast_sibling, std::ptr::null_mut()) };
    register(scope, failed);
    register(scope, sibling);
    assert!(unsafe { zirk_rt_scope_exit(scope) });
    assert!(unsafe { zirk_rt_has_pending_exception() });
    assert!(!unsafe { zirk_rt_take_pending_exception() }.is_null());
}

extern "C-unwind" fn invalid_scope_driver() {
    match std::env::var("ZIRK_COLLECT_SCOPE_INVALID").as_deref() {
        Ok("null") => unsafe { zirk_rt_scope_exit_collect_all(0) },
        Ok("stale") => unsafe { zirk_rt_scope_exit_collect_all(u64::MAX) },
        Ok("closed") => {
            let scope = unsafe { zirk_rt_scope_enter_collect_all() };
            assert!(unsafe { zirk_rt_scope_exit_collect_all(scope) });
            unsafe { zirk_rt_scope_exit_collect_all(scope) }
        }
        Ok("wrongmode") => {
            let scope = unsafe { zirk_rt_scope_enter() };
            unsafe { zirk_rt_scope_exit_collect_all(scope) }
        }
        other => panic!("unexpected invalid-scope test case: {other:?}"),
    };
}

#[test]
fn collect_all_joins_slow_sibling_after_early_failure_without_propagating() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(collect_all_driver);
        zirk_rt_shutdown();
    }
}

#[test]
fn collect_all_preserves_multiple_failures_and_child_cancellation() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(collect_multiple_terminal_states_driver);
        zirk_rt_shutdown();
    }
}

#[test]
fn caller_cancellation_cancels_and_joins_collect_all_children() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(caller_cancellation_driver);
        zirk_rt_shutdown();
    }
}

#[test]
fn ordinary_scope_remains_fail_fast() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(fail_fast_driver);
        zirk_rt_shutdown();
    }
    assert!(FAIL_FAST_SIBLING_CANCELLED.load(Ordering::SeqCst));
}

#[test]
fn collect_scope_rejects_null_stale_and_closed_handles() {
    if let Ok(case) = std::env::var("ZIRK_COLLECT_SCOPE_INVALID") {
        assert!(matches!(
            case.as_str(),
            "null" | "stale" | "closed" | "wrongmode"
        ));
        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(invalid_scope_driver);
            zirk_rt_shutdown();
        }
        return;
    }

    for case in ["null", "stale", "closed", "wrongmode"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "collect_scope_rejects_null_stale_and_closed_handles",
                "--nocapture",
            ])
            .env("ZIRK_COLLECT_SCOPE_INVALID", case)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "invalid {case} scope handle unexpectedly returned successfully"
        );
    }
}
