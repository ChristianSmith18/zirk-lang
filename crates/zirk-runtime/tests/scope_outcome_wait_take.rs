//! Runtime ABI coverage for dynamically consuming children from a collect-all
//! scope in completion order while writing GC references into registered roots.

use std::ffi::c_void;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use zirk_runtime::{
    zirk_rt_alloc, zirk_rt_branch_fail, zirk_rt_cancel, zirk_rt_init, zirk_rt_job_wait,
    zirk_rt_pop_frame, zirk_rt_push_frame, zirk_rt_run_main, zirk_rt_scope_cancel,
    zirk_rt_scope_enter_collect_all, zirk_rt_scope_exit_collect_all,
    zirk_rt_scope_outcome_wait_take, zirk_rt_shutdown, zirk_rt_sleep, zirk_rt_spawn,
};

const FULFILLED: u32 = 0;
const REJECTED: u32 = 1;
const CANCELLED: u32 = 2;
const CALLER_CANCELLED: u32 = 3;
const DRAINED: u32 = 4;

extern "C-unwind" fn slow_value(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(20_000_000) };
    11
}

extern "C-unwind" fn fast_value(_: *mut c_void) -> usize {
    22
}

extern "C-unwind" fn argument_value(arg: *mut c_void) -> usize {
    arg as usize
}

extern "C-unwind" fn rejection(_: *mut c_void) -> usize {
    let throwable = unsafe { zirk_rt_alloc(64, 8) };
    unsafe { *((throwable as *mut usize).add(3)) = 0x510c_0ffe };
    unsafe { zirk_rt_branch_fail(throwable) }
}

extern "C-unwind" fn long_sleep(_: *mut c_void) -> usize {
    unsafe { zirk_rt_sleep(86_400_000_000_000) };
    33
}

static CANCELLED_WAITER_STATUS: AtomicU32 = AtomicU32::new(u32::MAX);

fn rooted_slots(
    value: &mut *mut c_void,
    throwable: &mut *mut c_void,
    roots: &mut [*mut c_void; 2],
) {
    *roots = [
        (value as *mut *mut c_void).cast(),
        (throwable as *mut *mut c_void).cast(),
    ];
    unsafe { zirk_rt_push_frame(roots.as_mut_ptr(), roots.len() as i64) };
}

extern "C-unwind" fn completion_order_driver() {
    let scope = unsafe { zirk_rt_scope_enter_collect_all() };
    let slow = unsafe { zirk_rt_spawn(slow_value, std::ptr::null_mut()) };
    let fast = unsafe { zirk_rt_spawn(fast_value, std::ptr::null_mut()) };
    let rejected = unsafe { zirk_rt_spawn(rejection, std::ptr::null_mut()) };
    unsafe {
        zirk_runtime::zirk_rt_branch_register(scope, slow);
        zirk_runtime::zirk_rt_branch_register(scope, fast);
        zirk_runtime::zirk_rt_branch_register(scope, rejected);
    }

    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [std::ptr::null_mut(); 2];
    rooted_slots(&mut value, &mut throwable, &mut roots);
    let mut ordinal = u64::MAX;
    let mut outcomes = Vec::new();
    for _ in 0..3 {
        let status = unsafe {
            zirk_rt_scope_outcome_wait_take(scope, &mut ordinal, &mut value, &mut throwable)
        };
        outcomes.push((ordinal, status, value as usize, !throwable.is_null()));
        value = std::ptr::null_mut();
        throwable = std::ptr::null_mut();
    }
    assert_eq!(outcomes[0], (1, FULFILLED, 22, false));
    assert!(outcomes.contains(&(0, FULFILLED, 11, false)));
    assert!(
        outcomes
            .iter()
            .any(|row| row.0 == 2 && row.1 == REJECTED && row.3)
    );
    assert_eq!(
        unsafe { zirk_rt_scope_outcome_wait_take(scope, &mut ordinal, &mut value, &mut throwable) },
        DRAINED
    );
    assert_eq!(ordinal, u64::MAX);
    unsafe {
        zirk_rt_pop_frame();
        assert!(zirk_rt_scope_exit_collect_all(scope));
    }
}

extern "C-unwind" fn waiting_scope_reader(scope: *mut c_void) -> usize {
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [std::ptr::null_mut(); 2];
    rooted_slots(&mut value, &mut throwable, &mut roots);
    let mut ordinal = u64::MAX;
    let status = unsafe {
        zirk_rt_scope_outcome_wait_take(
            scope as usize as u64,
            &mut ordinal,
            &mut value,
            &mut throwable,
        )
    };
    CANCELLED_WAITER_STATUS.store(status, Ordering::SeqCst);
    zirk_rt_pop_frame();
    0
}

extern "C-unwind" fn one_shot_scope_reader(scope: *mut c_void) -> usize {
    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [std::ptr::null_mut(); 2];
    rooted_slots(&mut value, &mut throwable, &mut roots);
    let mut ordinal = u64::MAX;
    let status = unsafe {
        zirk_rt_scope_outcome_wait_take(
            scope as usize as u64,
            &mut ordinal,
            &mut value,
            &mut throwable,
        )
    };
    assert_eq!(status, FULFILLED);
    assert!(throwable.is_null());
    let result = (ordinal as usize) * 100 + value as usize;
    zirk_rt_pop_frame();
    result
}

extern "C-unwind" fn concurrent_scope_readers_driver() {
    let scope = unsafe { zirk_rt_scope_enter_collect_all() };
    for value in [11, 22] {
        let child = unsafe { zirk_rt_spawn(argument_value, value as *mut c_void) };
        unsafe { zirk_runtime::zirk_rt_branch_register(scope, child) };
    }
    let reader_a = unsafe { zirk_rt_spawn(one_shot_scope_reader, scope as usize as *mut c_void) };
    let reader_b = unsafe { zirk_rt_spawn(one_shot_scope_reader, scope as usize as *mut c_void) };
    let result_a = unsafe { zirk_rt_job_wait(reader_a) };
    let result_b = unsafe { zirk_rt_job_wait(reader_b) };
    let mut values = [result_a, result_b];
    values.sort_unstable();
    assert_eq!(values, [11, 122]);
    assert!(unsafe { zirk_rt_scope_exit_collect_all(scope) });
}

extern "C-unwind" fn caller_cancellation_driver() {
    CANCELLED_WAITER_STATUS.store(u32::MAX, Ordering::SeqCst);
    let scope = unsafe { zirk_rt_scope_enter_collect_all() };
    let child = unsafe { zirk_rt_spawn(long_sleep, std::ptr::null_mut()) };
    unsafe { zirk_runtime::zirk_rt_branch_register(scope, child) };
    let waiter = unsafe { zirk_rt_spawn(waiting_scope_reader, scope as usize as *mut c_void) };
    unsafe {
        zirk_rt_sleep(0);
        zirk_rt_cancel(waiter);
        let _ = std::panic::catch_unwind(|| zirk_rt_job_wait(waiter));
    }
    assert_eq!(
        CANCELLED_WAITER_STATUS.load(Ordering::SeqCst),
        CALLER_CANCELLED,
        "caller cancellation must not consume a child outcome"
    );
    unsafe { zirk_rt_scope_cancel(scope) };

    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [std::ptr::null_mut(); 2];
    rooted_slots(&mut value, &mut throwable, &mut roots);
    let mut ordinal = u64::MAX;
    assert_eq!(
        unsafe { zirk_rt_scope_outcome_wait_take(scope, &mut ordinal, &mut value, &mut throwable) },
        CANCELLED,
        "the child's cancellation result must remain available after caller cancellation"
    );
    assert_eq!(ordinal, 0);
    assert!(value.is_null());
    assert!(throwable.is_null());
    zirk_rt_pop_frame();
    assert!(unsafe { zirk_rt_scope_exit_collect_all(scope) });
}

extern "C-unwind" fn misuse_driver() {
    let case = std::env::var("ZIRK_SCOPE_OUTCOME_MISUSE").unwrap();
    let scope = match case.as_str() {
        "stale_scope" => u64::MAX,
        "wrong_mode" => unsafe { zirk_runtime::zirk_rt_scope_enter() },
        _ => unsafe { zirk_rt_scope_enter_collect_all() },
    };
    if case == "too_many" {
        for _ in 0..257 {
            let child = unsafe { zirk_rt_spawn(fast_value, std::ptr::null_mut()) };
            unsafe { zirk_runtime::zirk_rt_branch_register(scope, child) };
        }
    } else if case == "unrooted" {
        let child = unsafe { zirk_rt_spawn(fast_value, std::ptr::null_mut()) };
        unsafe { zirk_runtime::zirk_rt_branch_register(scope, child) };
    }

    let mut value = std::ptr::null_mut();
    let mut throwable = std::ptr::null_mut();
    let mut roots = [std::ptr::null_mut(); 2];
    if case != "unrooted" {
        rooted_slots(&mut value, &mut throwable, &mut roots);
    }
    let mut ordinal = u64::MAX;
    unsafe {
        zirk_rt_scope_outcome_wait_take(scope, &mut ordinal, &mut value, &mut throwable);
    }
    panic!("misuse case {case} unexpectedly returned");
}

#[test]
fn scope_outcome_wait_rejects_invalid_scope_roots_and_child_counts() {
    if std::env::var_os("ZIRK_SCOPE_OUTCOME_MISUSE").is_some() {
        unsafe {
            zirk_rt_init();
            zirk_rt_run_main(misuse_driver);
            zirk_rt_shutdown();
        }
        panic!("scope misuse process unexpectedly returned");
    }

    for (case, expected) in [
        ("stale_scope", "unknown scope"),
        ("wrong_mode", "requires a collect-all scope"),
        ("unrooted", "not registered GC roots"),
        ("too_many", "256-child safety bound"),
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "scope_outcome_wait_rejects_invalid_scope_roots_and_child_counts",
                "--nocapture",
            ])
            .env("ZIRK_SCOPE_OUTCOME_MISUSE", case)
            .stdout(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(70), "case {case}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "case {case} should fail closed for {expected}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn scope_outcome_wait_take_preserves_completion_order_roots_and_cancellation() {
    unsafe {
        zirk_rt_init();
        zirk_rt_run_main(completion_order_driver);
        zirk_rt_run_main(concurrent_scope_readers_driver);
        zirk_rt_run_main(caller_cancellation_driver);
        zirk_rt_shutdown();
    }
}
