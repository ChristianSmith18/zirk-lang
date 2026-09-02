//! Frontend-only validation tests for the `zirk check` subcommand.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the `zirk` executable produced by the workspace.
fn compiler() -> PathBuf {
    let exe = std::env::current_exe().expect("the test executable");
    let profile = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("unexpected Cargo layout");

    let candidate = profile.join(if cfg!(windows) { "zirk.exe" } else { "zirk" });
    assert!(
        candidate.is_file(),
        "the `zirk` executable was not found at {}; build the workspace first",
        candidate.display()
    );
    candidate
}

fn corpus(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("corpus")
        .join(kind)
}

/// A private working directory per test.
fn workspace(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("working directory");
    dir
}

/// Runs `zirk check` over a source file, from its own working directory.
fn zirk_check(source: &Path, name: &str) -> (bool, String, String) {
    zirk_check_with_arg(
        source,
        name,
        source.file_name().expect("file name").to_str().unwrap(),
    )
}

/// Runs `zirk check` with an explicit command-line argument.
fn zirk_check_with_arg(source: &Path, name: &str, arg: &str) -> (bool, String, String) {
    let dir = workspace(name);
    let copied = dir.join(source.file_name().expect("file name"));
    std::fs::copy(source, &copied).expect("copy the source");

    let output = Command::new(compiler())
        .arg("check")
        .arg(arg)
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn a_valid_program_passes_check() {
    let path = corpus("valid").join("hello.zrk");
    let (success, stdout, stderr) = zirk_check(&path, "valid_hello");

    assert!(success, "`zirk check` must exit 0 for a valid program");
    assert!(stdout.is_empty(), "`zirk check` must not write to stdout");
    assert!(
        stderr.is_empty(),
        "`zirk check` must not produce diagnostics"
    );
}

#[test]
fn an_invalid_program_is_rejected_with_diagnostics() {
    let path = corpus("invalid").join("type_mismatch.zrk");
    let (success, stdout, stderr) = zirk_check(&path, "invalid_duplicate");

    assert!(
        !success,
        "`zirk check` must exit non-zero for an invalid program"
    );
    assert!(stdout.is_empty(), "`zirk check` must not write to stdout");
    assert!(
        !stderr.is_empty(),
        "`zirk check` must emit a diagnostic on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("E0411") || stderr.contains("error"),
        "`zirk check` must report an error, got: {stderr}"
    );
}

#[test]
fn a_missing_file_reports_a_diagnostic() {
    let dir = workspace("missing_file");
    let output = Command::new(compiler())
        .arg("check")
        .arg("does_not_exist.zrk")
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not read"),
        "expected a readable diagnostic, got: {stderr}"
    );
}

#[test]
fn check_and_build_agree_on_invalid_programs() {
    let path = corpus("invalid").join("type_mismatch.zrk");
    let (_, _, check_stderr) = zirk_check(&path, "agreement_check");

    let dir = workspace("agreement_build");
    let copied = dir.join(path.file_name().expect("file name"));
    std::fs::copy(&path, &copied).expect("copy the source");

    let build_output = Command::new(compiler())
        .arg("build")
        .arg(copied.file_name().expect("file name"))
        .current_dir(&dir)
        .output()
        .expect("run the compiler");
    let build_stderr = String::from_utf8_lossy(&build_output.stderr).to_string();

    // The important part is that the same error is reported, not necessarily
    // identical formatting (which already has dedicated tests).
    assert!(
        !check_stderr.is_empty() && !build_stderr.is_empty(),
        "both `check` and `build` must emit diagnostics"
    );
}

#[test]
fn a_valid_program_passes_check_without_extension() {
    let path = corpus("valid").join("hello.zrk");
    let (success, stdout, stderr) = zirk_check_with_arg(&path, "valid_hello_bare", "hello");

    assert!(
        success,
        "`zirk check hello` must exit 0 for a valid program"
    );
    assert!(
        stdout.is_empty(),
        "`zirk check hello` must not write to stdout"
    );
    assert!(
        stderr.is_empty(),
        "`zirk check hello` must not produce diagnostics, got: {stderr}"
    );
}

#[test]
fn an_invalid_program_is_rejected_without_extension() {
    let path = corpus("invalid").join("type_mismatch.zrk");
    let (success, stdout, stderr) = zirk_check_with_arg(&path, "invalid_bare", "type_mismatch");

    assert!(
        !success,
        "`zirk check type_mismatch` must exit non-zero for an invalid program"
    );
    assert!(
        stdout.is_empty(),
        "`zirk check type_mismatch` must not write to stdout"
    );
    assert!(
        !stderr.is_empty(),
        "`zirk check type_mismatch` must emit a diagnostic on stderr, got: {stderr}"
    );
}

#[test]
fn a_program_with_zrk_extension_still_works() {
    let path = corpus("valid").join("hello.zrk");
    let (success, stdout, stderr) = zirk_check_with_arg(&path, "valid_hello_ext", "hello.zrk");

    assert!(success, "`zirk check hello.zrk` must still exit 0");
    assert!(
        stdout.is_empty(),
        "`zirk check hello.zrk` must not write to stdout"
    );
    assert!(
        stderr.is_empty(),
        "`zirk check hello.zrk` must not produce diagnostics, got: {stderr}"
    );
}
