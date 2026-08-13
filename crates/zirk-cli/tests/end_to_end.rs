//! End-to-end tests: from `.zrk` source to a running process.
//!
//! This is the milestone of Phase 1. Everything else — lexer, parser, checker,
//! IR, backend — has its own tests; what is verified here is that the whole
//! spine works together, which is what `ZIRK_ROADMAP.md` states as the output
//! of the phase.
//!
//! The corpus lives in `tests/corpus/`. Each valid program has a `.out` file
//! with its expected output; the invalid ones have their diagnostics captured
//! as snapshots so a change in wording is visible in review.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the `zirk` executable produced by the workspace.
fn compiler() -> PathBuf {
    // The test binary lives in `<target>/<profile>/deps/`; the CLI is one level
    // up, next to the runtime static library it needs to link.
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

/// A private working directory per test, so parallel runs do not collide over
/// the `build/` directory.
fn workspace(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("working directory");
    dir
}

struct Output {
    status: i32,
    stdout: String,
    stderr: String,
}

/// Runs the compiler over a source file, from its own working directory.
fn zirk(subcommand: &str, source: &Path, name: &str, extra: &[&str]) -> Output {
    let dir = workspace(name);
    let copied = dir.join(source.file_name().expect("file name"));
    std::fs::copy(source, &copied).expect("copy the source");

    let output = Command::new(compiler())
        .arg(subcommand)
        .arg(copied.file_name().expect("file name"))
        .args(extra)
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

// --- Valid programs ---------------------------------------------------------

/// Every valid program compiles, runs, and prints exactly its `.out` file.
#[test]
fn the_valid_corpus_compiles_and_produces_the_expected_output() {
    let dir = corpus("valid");
    let mut checked = 0;

    for entry in std::fs::read_dir(&dir).expect("read the corpus") {
        let path = entry.expect("corpus entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("zrk") {
            continue;
        }

        let name = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .into_owned();
        let expected = std::fs::read_to_string(path.with_extension("out")).unwrap_or_else(|_| {
            panic!("`{name}.out` is missing: every valid program declares its expected output")
        });

        let output = zirk("run", &path, &format!("valid_{name}"), &[]);

        assert_eq!(
            output.status, 0,
            "`{name}` did not run successfully.\nstderr:\n{}",
            output.stderr
        );
        assert_eq!(
            output.stdout, expected,
            "`{name}` printed something different than expected"
        );

        checked += 1;
    }

    assert!(
        checked >= 6,
        "the corpus shrank unexpectedly: {checked} programs"
    );
}

#[test]
fn the_reference_program_of_the_roadmap_runs() {
    let source = corpus("valid").join("hello.zrk");
    let output = zirk("run", &source, "reference", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, "Hola desde Zirk\n");
}

#[test]
fn build_produces_an_executable_that_runs_on_its_own() {
    let source = corpus("valid").join("hello.zrk");
    let dir = workspace("build_only");
    let copied = dir.join("hello.zrk");
    std::fs::copy(&source, &copied).expect("copy the source");

    let build = Command::new(compiler())
        .arg("build")
        .arg("hello.zrk")
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    assert!(
        build.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );

    // `build` prints the path of what it produced, so it can be piped.
    let executable = String::from_utf8_lossy(&build.stdout).trim().to_string();
    let executable = dir.join(&executable);
    assert!(
        executable.is_file(),
        "the announced executable does not exist: {}",
        executable.display()
    );

    let run = Command::new(&executable).output().expect("run the binary");
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Hola desde Zirk\n");
}

#[test]
fn the_executable_stays_on_disk_after_run() {
    // Resolves an open question of the design: someone who ran their program
    // will most likely want to distribute it.
    let source = corpus("valid").join("hello.zrk");
    let dir = workspace("persists");
    let copied = dir.join("hello.zrk");
    std::fs::copy(&source, &copied).expect("copy the source");

    Command::new(compiler())
        .arg("run")
        .arg("hello.zrk")
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    let executable = dir
        .join("build")
        .join(if cfg!(windows) { "hello.exe" } else { "hello" });
    assert!(executable.is_file(), "the executable must remain available");
}

// --- Invalid programs -------------------------------------------------------

/// Every invalid program is rejected, with a diagnostic that carries a cause.
#[test]
fn the_invalid_corpus_is_rejected_with_diagnostics() {
    let dir = corpus("invalid");
    let mut checked = 0;

    for entry in std::fs::read_dir(&dir).expect("read the corpus") {
        let path = entry.expect("corpus entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("zrk") {
            continue;
        }

        let name = path
            .file_stem()
            .expect("stem")
            .to_string_lossy()
            .into_owned();
        let output = zirk("run", &path, &format!("invalid_{name}"), &[]);

        assert_ne!(
            output.status, 0,
            "`{name}` should not have compiled.\nstdout:\n{}",
            output.stdout
        );
        assert!(
            output.stderr.contains("error["),
            "`{name}` failed without a diagnostic:\n{}",
            output.stderr
        );
        assert!(
            output.stderr.contains("= cause:"),
            "the diagnostic of `{name}` has no cause:\n{}",
            output.stderr
        );
        assert!(
            output.stdout.is_empty(),
            "`{name}` wrote to standard output despite failing"
        );

        checked += 1;
    }

    assert!(
        checked >= 10,
        "the corpus shrank unexpectedly: {checked} programs"
    );
}

#[test]
fn diagnostics_go_to_standard_error() {
    // They must never mix with the output of the compiled program.
    let source = corpus("invalid").join("type_mismatch.zrk");
    let output = zirk("run", &source, "stderr_only", &[]);

    assert!(output.stdout.is_empty());
    assert!(output.stderr.contains("error["));
}

#[test]
fn structured_output_is_available() {
    let source = corpus("invalid").join("type_mismatch.zrk");
    let output = zirk("run", &source, "json", &["--json"]);

    assert!(output.stderr.starts_with('['), "{}", output.stderr);
    assert!(output.stderr.contains("\"severity\":\"error\""));
    assert!(output.stderr.contains("\"code\":"));
    assert!(output.stderr.contains("\"line\":"));
}

// --- CLI behaviour ----------------------------------------------------------

#[test]
fn a_missing_file_produces_a_diagnostic() {
    let dir = workspace("missing_file");
    let output = Command::new(compiler())
        .arg("run")
        .arg("does_not_exist.zrk")
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error["));
}

#[test]
fn several_files_state_the_phase_they_arrive_in() {
    let dir = workspace("several_files");
    let output = Command::new(compiler())
        .arg("build")
        .arg("a.zrk")
        .arg("b.zrk")
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("Phase 6"), "{stderr}");
}

#[test]
fn later_subcommands_name_their_phase() {
    for (subcommand, phase) in [
        ("check", "Phase 2"),
        ("format", "Phase 9"),
        ("new", "Phase 6"),
    ] {
        let output = Command::new(compiler())
            .arg(subcommand)
            .output()
            .expect("run the compiler");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(
            stderr.contains(phase),
            "`zirk {subcommand}` must say it arrives in {phase}:\n{stderr}"
        );
    }
}

#[test]
fn help_and_version_succeed() {
    for flag in ["--help", "--version", "--list-targets"] {
        let output = Command::new(compiler())
            .arg(flag)
            .output()
            .expect("run the compiler");
        assert!(output.status.success(), "`zirk {flag}` failed");
        assert!(!output.stdout.is_empty());
    }
}
