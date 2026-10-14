//! The public checked ABI rejects saturation without producing fake handles.

use std::ffi::c_void;

use zirk_runtime::{
    zirk_rt_cancel, zirk_rt_job_wait, zirk_rt_run_main, zirk_rt_scope_enter, zirk_rt_scope_exit,
    zirk_rt_sleep_checked, zirk_rt_spawn_checked, zirk_rt_timer_after_checked,
};

extern "C-unwind" fn completed_task(_: *mut c_void) -> usize {
    41
}

extern "C-unwind" fn sleeping_task(_: *mut c_void) -> usize {
    // SAFETY: this thunk is only started by checked spawn while running in an
    // executor task. Cancellation wakes the registered timer wait.
    let _ = unsafe { zirk_rt_sleep_checked(60_000_000_000) };
    7
}

extern "C-unwind" fn timer_task(_: *mut c_void) -> usize {
    19
}

extern "C-unwind" fn root() {
    // The root itself consumes one of the executor's 4096 active-task slots.
    // Keep the children ready until the 4096th total slot is occupied.
    for _ in 0..4095 {
        let mut status = u32::MAX;
        // SAFETY: this runs as the executor root; the thunk and null capture
        // satisfy the checked spawn ABI, and status is a writable local.
        let id =
            unsafe { zirk_rt_spawn_checked(completed_task, std::ptr::null_mut(), &mut status) };
        assert_eq!(status, 0);
        assert_ne!(id, 0);
    }

    let mut status = u32::MAX;
    // SAFETY: same root/thunk preconditions; the output slot is writable.
    let rejected =
        unsafe { zirk_rt_spawn_checked(completed_task, std::ptr::null_mut(), &mut status) };
    assert_eq!(status, 1, "saturation must be a typed ABI status");
    assert_eq!(rejected, 0, "a failed admission must not return a task ID");

    // Yield so every admitted child reaches terminal state. Its retained
    // outcome does not consume active-task capacity.
    // SAFETY: called from the executor root; zero duration is valid.
    assert_eq!(unsafe { zirk_rt_sleep_checked(0) }, 0);

    let mut status = u32::MAX;
    // SAFETY: the previous children are terminal and this is still the root.
    let accepted =
        unsafe { zirk_rt_spawn_checked(completed_task, std::ptr::null_mut(), &mut status) };
    assert_eq!(status, 0);
    // SAFETY: `accepted` was just returned by the checked spawn ABI.
    assert_eq!(unsafe { zirk_rt_job_wait(accepted) }, 41);
}

#[test]
fn checked_spawn_abi_rejects_4097th_active_task_and_reuses_capacity() {
    // SAFETY: the test supplies one valid root function and the runtime owns
    // the executor lifetime for the duration of this call.
    unsafe { zirk_rt_run_main(root) };
}

extern "C-unwind" fn cancel_root() {
    // Fill every child slot with a task suspended on a real timer, retaining
    // each handle so cancellation traverses the public ABI for all children.
    let mut jobs = Vec::with_capacity(4095);
    for _ in 0..4095 {
        let mut status = u32::MAX;
        // SAFETY: the checked ABI receives a valid thunk and writable status.
        let id = unsafe { zirk_rt_spawn_checked(sleeping_task, std::ptr::null_mut(), &mut status) };
        assert_eq!(status, 0);
        jobs.push(id);
    }

    for &job in &jobs {
        // SAFETY: each ID was returned by checked spawn above and is still live.
        unsafe { zirk_rt_cancel(job) };
    }

    // Timer-backed tasks reserve both an active-task permit and a timer slot.
    // Retry while task capacity is still occupied by cancellation cleanup;
    // once a slot is available, timer admission must succeed too. Status 2
    // here would expose a timer-permit leak after cancellation.
    // SAFETY: this root task opens a valid structured scope.
    let scope = unsafe { zirk_rt_scope_enter() };
    let mut timer_retries = 0;
    loop {
        let mut status = u32::MAX;
        // SAFETY: `scope` is live, callback is valid, capture is null, and the
        // status local is writable for the duration of the ABI call.
        let timer = unsafe {
            zirk_rt_timer_after_checked(
                scope,
                1_000_000,
                timer_task,
                std::ptr::null_mut(),
                &mut status,
            )
        };
        if status == 0 {
            assert_ne!(timer, 0);
            break;
        }
        assert_eq!(status, 1, "timer quota leaked after task cancellation");
        timer_retries += 1;
        assert!(
            timer_retries <= 100_000,
            "cancelled tasks did not release quota"
        );
        // SAFETY: called from the executor root; zero-duration sleep yields.
        assert_eq!(unsafe { zirk_rt_sleep_checked(0) }, 0);
    }
    // The timer may finish naturally or as scope cleanup starts; either way,
    // the scope close joins it before we reuse the remaining task slots.
    // SAFETY: `scope` is live and owned by this root; close waits for children.
    assert!(unsafe { zirk_rt_scope_exit(scope) });

    // Refill all released permits with tasks that remain suspended on timers,
    // so capacity cannot be reused accidentally because a fast callback
    // completes while this root is filling the quota.
    let mut refill_jobs = Vec::with_capacity(4095);
    let mut accepted = 0;
    let mut retries = 0;
    while accepted < 4095 {
        let mut status = u32::MAX;
        // SAFETY: the root task and output slot satisfy checked spawn's ABI.
        let id = unsafe { zirk_rt_spawn_checked(sleeping_task, std::ptr::null_mut(), &mut status) };
        if status == 0 {
            assert_ne!(id, 0);
            accepted += 1;
            refill_jobs.push(id);
        } else {
            assert_eq!(status, 1, "unexpected checked-spawn rejection");
            retries += 1;
            assert!(retries <= 100_000, "cancelled tasks did not release quota");
            // SAFETY: called from the executor root; zero-duration sleep yields.
            assert_eq!(unsafe { zirk_rt_sleep_checked(0) }, 0);
        }
    }

    let mut status = u32::MAX;
    // SAFETY: the executor is full again; this is a valid capacity probe.
    let rejected =
        unsafe { zirk_rt_spawn_checked(completed_task, std::ptr::null_mut(), &mut status) };
    assert_eq!(
        status, 1,
        "quota should be full after admitting {accepted} refill jobs"
    );
    assert_eq!(rejected, 0);

    for &job in &refill_jobs {
        // SAFETY: each ID is a live task returned by checked spawn above.
        unsafe { zirk_rt_cancel(job) };
    }

    // Require a new timer-backed admission after cancelling the refill batch.
    // A task-capacity rejection is expected until cleanup runs; a timer-capacity
    // rejection after task capacity opens would prove the timer permit leaked.
    // SAFETY: this root opens a new live scope for the timer admission probe.
    let scope = unsafe { zirk_rt_scope_enter() };
    let mut timer_retries = 0;
    loop {
        let mut status = u32::MAX;
        // SAFETY: valid scope, callback, capture, and writable status output.
        let timer = unsafe {
            zirk_rt_timer_after_checked(
                scope,
                1_000_000,
                timer_task,
                std::ptr::null_mut(),
                &mut status,
            )
        };
        if status == 0 {
            assert_ne!(timer, 0);
            break;
        }
        assert_eq!(status, 1, "timer permit leaked after cancellation refill");
        timer_retries += 1;
        assert!(
            timer_retries <= 100_000,
            "cancelled refill tasks did not release quota"
        );
        // SAFETY: called from the executor root; zero-duration sleep yields.
        assert_eq!(unsafe { zirk_rt_sleep_checked(0) }, 0);
    }
    // SAFETY: the live scope belongs to this root; closing it requests
    // cancellation of the ambient timer task.
    assert!(unsafe { zirk_rt_scope_exit(scope) });

    // Drain the timer callback and verify a task slot is reusable once its
    // completion releases the active permit.
    let mut completion_retries = 0;
    let final_job = loop {
        let mut status = u32::MAX;
        // SAFETY: checked spawn receives a valid thunk and writable status.
        let job =
            unsafe { zirk_rt_spawn_checked(completed_task, std::ptr::null_mut(), &mut status) };
        if status == 0 {
            break job;
        }
        assert_eq!(status, 1);
        completion_retries += 1;
        assert!(
            completion_retries <= 100_000,
            "cancelled tasks did not drain"
        );
        // SAFETY: called from the executor root; zero-duration sleep yields.
        assert_eq!(unsafe { zirk_rt_sleep_checked(0) }, 0);
    };
    // SAFETY: `final_job` is a live, unconsumed handle returned above.
    assert_eq!(unsafe { zirk_rt_job_wait(final_job) }, 41);
}

#[test]
fn checked_spawn_quota_is_released_after_public_abi_cancellation() {
    // SAFETY: the runtime owns the executor lifetime and `cancel_root` is a
    // valid root thunk for the duration of this call.
    unsafe { zirk_rt_run_main(cancel_root) };
}
