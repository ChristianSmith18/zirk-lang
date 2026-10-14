use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use zirk_runtime::{
    SHUTDOWN_DEADLINE_EXIT_124, zirk_rt_init, zirk_rt_request_host_shutdown, zirk_rt_run_main_cli,
    zirk_rt_run_main_with_deadline_policy, zirk_rt_sleep,
};

extern "C-unwind" fn sleeping_root() {
    unsafe { zirk_rt_sleep(60_000_000_000) };
    println!("root cleanup reached");
}

#[test]
fn host_notification_wakes_cooperative_root_and_runs_cleanup() {
    const CHILD: &str = "ZIRK_HOST_SHUTDOWN_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        unsafe { zirk_rt_init() };
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            assert_eq!(zirk_rt_request_host_shutdown(15), 0);
        });
        unsafe { zirk_rt_run_main_cli(sleeping_root) };
        panic!("CLI entrypoint should exit with the queued host signal status");
    }

    let mut child = Command::new(std::env::current_exe().expect("test executable path"))
        .args([
            "--exact",
            "host_notification_wakes_cooperative_root_and_runs_cleanup",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn host-notification child");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().expect("poll child status").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("host notification did not wake cooperative shutdown within 5 seconds");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().expect("collect child output");
    assert_eq!(
        output.status.code(),
        Some(143),
        "stderr: {:?}",
        output.stderr
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("root cleanup reached"),
        "cooperative root did not resume for cleanup; stdout={:?}, stderr={:?}",
        output.stdout,
        output.stderr
    );
}

#[test]
fn explicit_terminating_deadline_policy_preserves_queued_signal_status() {
    const CHILD: &str = "ZIRK_HOST_SHUTDOWN_POLICY_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        unsafe { zirk_rt_init() };
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            assert_eq!(zirk_rt_request_host_shutdown(15), 0);
        });
        unsafe { zirk_rt_run_main_with_deadline_policy(sleeping_root, SHUTDOWN_DEADLINE_EXIT_124) };
        panic!("terminating deadline policy should preserve the host signal status");
    }

    let mut child = Command::new(std::env::current_exe().expect("test executable path"))
        .args([
            "--exact",
            "explicit_terminating_deadline_policy_preserves_queued_signal_status",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn host-shutdown policy child");

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().expect("poll child status").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("host shutdown policy did not finish within 5 seconds");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().expect("collect child output");
    assert_eq!(output.status.code(), Some(143));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("root cleanup reached"),
        "cooperative root cleanup was skipped: stdout={:?}, stderr={:?}",
        output.stdout,
        output.stderr
    );
}
