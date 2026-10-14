#![cfg(unix)]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use zirk_runtime::{
    zirk_rt_app_set_shutdown_grace, zirk_rt_init, zirk_rt_run_main_cli, zirk_rt_sleep,
};

const CHILD_MODE: &str = "ZIRK_CLI_OS_SIGNAL_CHILD";
const READY_FILE: &str = "ZIRK_CLI_OS_SIGNAL_READY_FILE";
const CLEANUP_FILE: &str = "ZIRK_CLI_OS_SIGNAL_CLEANUP_FILE";

fn signal_file(name: &str) {
    fs::write(
        std::env::var_os(name).expect("signal-test file path"),
        "ready",
    )
    .expect("write signal-test marker");
}

extern "C-unwind" fn graceful_root() {
    signal_file(READY_FILE);
    unsafe { zirk_rt_sleep(60_000_000_000) };
    signal_file(CLEANUP_FILE);
}

extern "C-unwind" fn stuck_root() {
    assert_eq!(unsafe { zirk_rt_app_set_shutdown_grace(300_000_000) }, 0);
    signal_file(READY_FILE);
    loop {
        std::hint::spin_loop();
    }
}

extern "C-unwind" fn ignored_root() {
    signal_file(READY_FILE);
    thread::sleep(Duration::from_millis(400));
    signal_file(CLEANUP_FILE);
}

fn test_paths(label: &str) -> (PathBuf, PathBuf) {
    let base =
        std::env::temp_dir().join(format!("zirk-cli-signals-{}-{label}", std::process::id()));
    (base.with_extension("ready"), base.with_extension("done"))
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(Instant::now() < deadline, "child never reached CLI root");
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_child(child: &mut Child) -> (i32, String) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().expect("poll signal child") {
            let mut stderr = String::new();
            child
                .stderr
                .take()
                .expect("child stderr")
                .read_to_string(&mut stderr)
                .expect("read child stderr");
            return (status.code().unwrap_or(128), stderr);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("signal child did not exit before deadline");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn spawn_child(test_name: &str, mode: &str, ready: &PathBuf, done: &PathBuf) -> Child {
    Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", test_name, "--nocapture"])
        .env(CHILD_MODE, mode)
        .env(READY_FILE, ready)
        .env(CLEANUP_FILE, done)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn signal-test child")
}

#[test]
fn real_sigint_runs_cooperative_cleanup_and_exits_130() {
    if matches!(
        std::env::var(CHILD_MODE).as_deref(),
        Ok("graceful" | "repeat")
    ) {
        unsafe { zirk_rt_init() };
        unsafe { zirk_rt_run_main_cli(graceful_root) };
        panic!("CLI runner returned after handling SIGINT");
    }

    let (ready, done) = test_paths("graceful");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&done);
    let mut child = spawn_child(
        "real_sigint_runs_cooperative_cleanup_and_exits_130",
        "graceful",
        &ready,
        &done,
    );
    wait_for_file(&ready);
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let (status, stderr) = wait_for_child(&mut child);
    assert_eq!(status, 130, "stderr: {stderr}");
    assert!(
        done.exists(),
        "cooperative cleanup did not run; stderr: {stderr}"
    );
    let _ = fs::remove_file(ready);
    let _ = fs::remove_file(done);
}

#[test]
fn repeated_sigint_runs_cleanup_and_releases_each_cli_process() {
    const CYCLES: usize = 8;
    for cycle in 0..CYCLES {
        let label = format!("repeat-{cycle}");
        let (ready, done) = test_paths(&label);
        let _ = fs::remove_file(&ready);
        let _ = fs::remove_file(&done);
        let mut child = spawn_child(
            "real_sigint_runs_cooperative_cleanup_and_exits_130",
            "repeat",
            &ready,
            &done,
        );
        wait_for_file(&ready);
        assert_eq!(
            unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
            0,
            "SIGINT cycle {cycle}"
        );
        let (status, stderr) = wait_for_child(&mut child);
        assert_eq!(status, 130, "cycle {cycle} stderr: {stderr}");
        assert!(done.exists(), "cycle {cycle} did not run cleanup");
        let _ = fs::remove_file(ready);
        let _ = fs::remove_file(done);
    }
}

#[test]
fn real_sigterm_runs_cooperative_cleanup_and_exits_143() {
    if std::env::var_os(CHILD_MODE).as_deref() == Some(std::ffi::OsStr::new("term")) {
        unsafe { zirk_rt_init() };
        unsafe { zirk_rt_run_main_cli(graceful_root) };
        panic!("CLI runner returned after handling SIGTERM");
    }

    let (ready, done) = test_paths("term");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&done);
    let mut child = spawn_child(
        "real_sigterm_runs_cooperative_cleanup_and_exits_143",
        "term",
        &ready,
        &done,
    );
    wait_for_file(&ready);
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) },
        0
    );
    let (status, stderr) = wait_for_child(&mut child);
    assert_eq!(status, 143, "stderr: {stderr}");
    assert!(
        done.exists(),
        "cooperative cleanup did not run; stderr: {stderr}"
    );
    let _ = fs::remove_file(ready);
    let _ = fs::remove_file(done);
}

#[test]
fn real_sigint_hard_exits_124_when_native_code_does_not_yield() {
    if std::env::var_os(CHILD_MODE).as_deref() == Some(std::ffi::OsStr::new("stuck")) {
        unsafe { zirk_rt_init() };
        unsafe { zirk_rt_run_main_cli(stuck_root) };
        panic!("CLI runner returned despite a stuck native root");
    }

    let (ready, done) = test_paths("stuck");
    let _ = fs::remove_file(&ready);
    let mut child = spawn_child(
        "real_sigint_hard_exits_124_when_native_code_does_not_yield",
        "stuck",
        &ready,
        &done,
    );
    wait_for_file(&ready);
    let started = Instant::now();
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let (status, stderr) = wait_for_child(&mut child);
    assert_eq!(status, 124, "stderr: {stderr}");
    assert!(started.elapsed() >= Duration::from_millis(250));
    assert!(started.elapsed() < Duration::from_secs(3));
    let _ = fs::remove_file(ready);
}

#[test]
fn cli_preserves_inherited_ignored_sigint() {
    if std::env::var_os(CHILD_MODE).as_deref() == Some(std::ffi::OsStr::new("ignored")) {
        // SAFETY: this child intentionally establishes an ignored disposition
        // before entering the generated-CLI wrapper.
        assert_ne!(
            unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) },
            libc::SIG_ERR
        );
        unsafe { zirk_rt_init() };
        unsafe { zirk_rt_run_main_cli(ignored_root) };
        return;
    }

    let (ready, done) = test_paths("ignored");
    let _ = fs::remove_file(&ready);
    let _ = fs::remove_file(&done);
    let mut child = spawn_child(
        "cli_preserves_inherited_ignored_sigint",
        "ignored",
        &ready,
        &done,
    );
    wait_for_file(&ready);
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let (status, stderr) = wait_for_child(&mut child);
    assert_eq!(status, 0, "stderr: {stderr}");
    assert!(
        done.exists(),
        "ignored signal interrupted the CLI; stderr: {stderr}"
    );
    let _ = fs::remove_file(ready);
    let _ = fs::remove_file(done);
}
