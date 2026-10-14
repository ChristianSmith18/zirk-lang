use std::process::Command;

use zirk_runtime::{
    MANAGED_CALL_OK, ManagedCallThunk, spawn_background, zirk_rt_app_set_shutdown_grace,
    zirk_rt_run_main_cli, zirk_rt_thread_run,
};

unsafe extern "C-unwind" fn never_returns(
    _target: *mut std::ffi::c_void,
    _captures: *mut std::ffi::c_void,
    _result: *mut std::ffi::c_void,
    _exception: *mut std::ffi::c_void,
) -> u32 {
    loop {
        std::thread::park();
    }
}

extern "C-unwind" fn stuck_background_root() {
    assert_eq!(unsafe { zirk_rt_app_set_shutdown_grace(0) }, 0);
    spawn_background(|| -> usize {
        static mut TARGET: u8 = 0;
        let mut exception = 0usize;
        // SAFETY: the callback deliberately never returns or dereferences its
        // arguments. The runtime keeps its registered worker and caller roots
        // alive until the CLI's process-level deadline termination.
        unsafe {
            zirk_rt_thread_run(
                never_returns as ManagedCallThunk,
                std::ptr::addr_of_mut!(TARGET).cast(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                (&mut exception as *mut usize).cast(),
            );
        }
        MANAGED_CALL_OK as usize
    })
    .expect("background job should be admitted");
}

#[test]
fn cli_exits_124_when_native_callback_exceeds_shutdown_grace() {
    if std::env::var_os("ZIRK_DEADLINE_CHILD").is_some() {
        // SAFETY: this process intentionally exits from the CLI policy once
        // the stuck managed callback exceeds its zero-duration grace.
        unsafe { zirk_rt_run_main_cli(stuck_background_root) };
        panic!("CLI deadline policy returned despite a stuck native callback");
    }

    let output = Command::new(std::env::current_exe().expect("test executable path"))
        .args([
            "--exact",
            "cli_exits_124_when_native_callback_exceeds_shutdown_grace",
            "--nocapture",
        ])
        .env("ZIRK_DEADLINE_CHILD", "1")
        .output()
        .expect("spawn deadline-policy child");
    assert_eq!(
        output.status.code(),
        Some(124),
        "stderr: {:?}",
        output.stderr
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("background shutdown grace expired"),
        "deadline diagnostics missing: {:?}",
        output.stderr
    );
}
