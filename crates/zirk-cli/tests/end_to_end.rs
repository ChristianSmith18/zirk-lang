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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// Normalizes line breaks for comparison.
///
/// `println` emits the platform line break, as `ZIRK_STDLIB_SPEC.md` section 3
/// requires, so on Windows the output carries CRLF. A `.out` fixture cannot
/// encode both, and the interesting property is the content, not which byte
/// ends each line — that one is already pinned by the runtime tests.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn approval_security_directory(home: &Path) -> PathBuf {
    #[cfg(windows)]
    let root = home.join("AppData/Local/Zirk");
    #[cfg(not(windows))]
    let root = home.join(".local/share/zirk");
    root.join("security")
}

/// Restores only operating-system and native-linker variables after a test
/// deliberately clears the environment. On Windows, `clang` needs these to
/// locate the MSVC linker/SDK and to start child tools; the test still supplies
/// its own PATH and controlled application variables.
fn preserve_native_toolchain_environment(command: &mut Command) {
    #[cfg(windows)]
    for name in [
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "PATHEXT",
        "COMSPEC",
        "VSINSTALLDIR",
        "VCINSTALLDIR",
        "VCIDEInstallDir",
        "VCToolsInstallDir",
        "VCToolsVersion",
        "WindowsSdkDir",
        "UniversalCRTSdkDir",
        "UCRTVersion",
        "VisualStudioVersion",
        "VSCMD_VER",
        "VSCMD_ARG_HOST_ARCH",
        "VSCMD_ARG_TGT_ARCH",
        "VSCMD_ARG_app_plat",
        "DevEnvDir",
        "INCLUDE",
        "LIB",
        "LIBPATH",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// Runs the compiler over a source file, from its own working directory.
fn zirk(subcommand: &str, source: &Path, name: &str, extra: &[&str]) -> Output {
    let file_name = source.file_name().expect("file name").to_string_lossy();
    zirk_with_arg(subcommand, source, name, &file_name, extra)
}

/// Runs the compiler over a source file using an explicit command-line argument.
fn zirk_with_arg(subcommand: &str, source: &Path, name: &str, arg: &str, extra: &[&str]) -> Output {
    zirk_with_runtime_env(subcommand, source, name, arg, extra, &[])
}

fn zirk_with_runtime_env(
    subcommand: &str,
    source: &Path,
    name: &str,
    arg: &str,
    extra: &[&str],
    env: &[(&str, &str)],
) -> Output {
    let dir = workspace(name);
    let copied = dir.join(source.file_name().expect("file name"));
    std::fs::copy(source, &copied).expect("copy the source");

    let output = Command::new(compiler())
        .arg(subcommand)
        .arg(arg)
        .args(extra)
        .envs(env.iter().copied())
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn managed_background_error_handler_roots_throwable_and_captures_during_gc() {
    let source = corpus("valid").join("background_error_handler_receives_throwable.zrk");
    let output = zirk_with_runtime_env(
        "run",
        &source,
        "managed_background_error_roots",
        "background_error_handler_receives_throwable.zrk",
        &[],
        &[("ZIRK_GC_THRESHOLD", "1")],
    );
    assert_eq!(
        output.status, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), "retained:detached failure\n");
}

#[test]
fn managed_background_error_handler_failure_is_reported_without_recursion() {
    let source = corpus("valid").join("background_error_handler_failure_contained.zrk");
    let output = zirk(
        "run",
        &source,
        "managed_background_error_handler_failure",
        &[],
    );
    assert_eq!(
        output.status, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), "handler-called\n");
    assert!(
        output
            .stderr
            .contains("background error handler failed for job BackgroundJobId(1)"),
        "handler failure must be reported: {}",
        output.stderr
    );
}

#[test]
fn bundled_http_server_event_encoder_preserves_bounded_sse_framing() {
    let source = corpus("valid").join("http_server_event_encoding.zrk");
    let output = zirk("run", &source, "http_server_event_encoding", &[]);
    assert_eq!(
        output.status, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(
        normalize(&output.stdout),
        "encoded\nmessage reset\nline endings normalized\nevent injection rejected\nid injection rejected\n"
    );
}

// --- Valid programs ---------------------------------------------------------

/// Every valid program compiles, runs with its declared status, and prints its `.out` file.
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
        let expected_status = std::fs::read_to_string(path.with_extension("exit"))
            .map(|value| value.trim().parse::<i32>().expect("valid exit status"))
            .unwrap_or(0);

        let output = zirk("run", &path, &format!("valid_{name}"), &[]);

        assert_eq!(
            output.status, expected_status,
            "`{name}` exited with an unexpected status.\nstdout:\n{}\nstderr:\n{}",
            output.stdout, output.stderr
        );
        assert_eq!(
            normalize(&output.stdout),
            normalize(&expected),
            "`{name}` printed something different than expected"
        );

        checked += 1;
    }

    assert!(
        checked >= 10,
        "the corpus shrank unexpectedly: {checked} programs"
    );
}

#[test]
fn concurrent_first_uses_completion_order_and_aggregates_failures() {
    for name in [
        "concurrent_first_completion_order",
        "concurrent_first_tie_input_order",
        "concurrent_first_failure_then_success",
        "concurrent_first_all_fail_suppressed",
        "concurrent_first_cleanup_failure",
    ] {
        let source = corpus("valid").join(format!("{name}.zrk"));
        let expected = std::fs::read_to_string(source.with_extension("out"))
            .expect("focused first fixture has expected output");
        let output = zirk("run", &source, &format!("focused_{name}"), &[]);
        assert_eq!(
            output.status, 0,
            "`{name}` failed.\nstdout:\n{}\nstderr:\n{}",
            output.stdout, output.stderr
        );
        assert_eq!(
            normalize(&output.stdout),
            normalize(&expected),
            "`{name}` output"
        );
    }
}

#[test]
fn concurrent_within_first_and_settled_enforce_deadlines() {
    for name in [
        "concurrent_within_first_winner",
        "concurrent_within_first_timeout",
        "concurrent_within_first_timeout_joins_cleanup",
        "concurrent_within_first_caller_cancellation",
        "concurrent_within_first_roots_managed_winner",
        "concurrent_within_settled_timeout",
        "concurrent_within_settled_timeout_joins_cleanup",
        "concurrent_within_settled_success",
        "concurrent_within_negative_duration",
    ] {
        let source = corpus("valid").join(format!("{name}.zrk"));
        let expected = std::fs::read_to_string(source.with_extension("out"))
            .expect("focused within fixture has expected output");
        let output = zirk("run", &source, &format!("focused_{name}"), &[]);
        assert_eq!(
            output.status, 0,
            "`{name}` failed.\nstdout:\n{}\nstderr:\n{}",
            output.stdout, output.stderr
        );
        assert_eq!(
            normalize(&output.stdout),
            normalize(&expected),
            "`{name}` output"
        );
    }
}

#[test]
fn concurrent_task_capacity_is_caught_and_reusable_from_zirk() {
    let name = "concurrent_task_capacity_ceiling";
    let source = corpus("valid").join(format!("{name}.zrk"));
    let expected = std::fs::read_to_string(source.with_extension("out"))
        .expect("task-capacity fixture has expected output");
    let output = zirk("run", &source, "concurrent_task_capacity_ceiling", &[]);

    assert_eq!(
        output.status, 0,
        "`{name}` did not run successfully.\nstdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), normalize(&expected));
}

#[test]
fn background_capacity_error_is_constructible_and_catchable_from_zirk() {
    let name = "background_capacity_error_catch";
    let source = corpus("valid").join(format!("{name}.zrk"));
    let expected = std::fs::read_to_string(source.with_extension("out"))
        .expect("background-capacity fixture has expected output");
    let output = zirk("run", &source, name, &[]);

    assert_eq!(
        output.status, 0,
        "`{name}` did not run successfully.\nstdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), normalize(&expected));
}

#[test]
fn app_shutdown_grace_is_configurable_from_native_zirk_and_negative_literals_are_rejected() {
    let name = "app_shutdown_grace_configured";
    let source = corpus("valid").join(format!("{name}.zrk"));
    let output = zirk("run", &source, name, &[]);
    assert_eq!(
        output.status, 124,
        "configured shutdown grace should use the approved expiry status.\nstdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(
        normalize(&output.stdout),
        "negative rejected\nroot completed\n"
    );
    assert!(output.stderr.contains("background shutdown grace expired"));
    assert!(output.stderr.contains("unfinished jobs: ["));

    let negative = corpus("invalid").join("app_shutdown_grace_negative.zrk");
    let output = zirk("check", &negative, "app_shutdown_grace_negative", &[]);
    assert_ne!(output.status, 0, "negative literal grace must fail in sema");
    assert!(output.stderr.contains("requires a non-negative Duration"));

    let wrong_type = corpus("invalid").join("app_shutdown_grace_wrong_type.zrk");
    let output = zirk("check", &wrong_type, "app_shutdown_grace_wrong_type", &[]);
    assert_ne!(output.status, 0, "non-Duration grace must fail in sema");
    assert!(
        output
            .stderr
            .contains("no implicit conversion from Int32 to Duration"),
        "{}",
        output.stderr
    );
}

#[test]
fn the_reference_program_of_the_roadmap_runs() {
    let source = corpus("valid").join("hello.zrk");
    let output = zirk("run", &source, "reference", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "Hola desde Zirk\n");
}

#[test]
fn the_reference_program_runs_without_the_zrk_extension() {
    let source = corpus("valid").join("hello.zrk");
    let output = zirk_with_arg("run", &source, "reference_bare", "hello", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "Hola desde Zirk\n");
}

#[test]
fn the_concurrent_examples_program_runs() {
    // `examples/concurrent_examples.zrk` — the normative showcase for
    // `concurrent-blocks-and-timers`.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("concurrent_examples.zrk");
    let output = zirk("run", &source, "concurrent_examples", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "10\n1\n2\n3\n42\ncaught: boom\n");
}

#[test]
fn concurrent_join_round_trips_nested_managed_record_results() {
    let source = std::env::temp_dir().join(format!(
        "zirk-managed-task-result-{}-{}.zrk",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::write(
        &source,
        "import { stdout } from std.io;\n\
         record Inner { text: String; }\n\
         record Outer { prefix: Int64; inner: Inner; }\n\
         enum MaybeText { Some(value: String), None }\n\
         fn make_outer(): Outer { return Outer(prefix: 5, inner: Inner(text: \"task result survived\")); }\n\
         fn make_enum(): MaybeText { return MaybeText.Some(value: \"enum result survived\"); }\n\
         fn make_wide(): Int128 { return 123456789123456789012345678901234567; }\n\
         fn churn(): Int32 { mut values: List<Int32> = [0 .. 4096]; return 7; }\n\
         fn main(): Void {\n\
           concurrent {\n\
             inmut value: Outer = make_outer();\n\
             inmut enum_value: MaybeText = make_enum();\n\
             inmut wide: Int128 = make_wide();\n\
             inmut churned: Int32 = churn();\n\
           }\n\
           stdout.println(value.inner.text);\n\
           match enum_value {\n\
             MaybeText.Some(text) => stdout.println(text);\n\
             MaybeText.None => stdout.println(\"empty\");\n\
           }\n\
           stdout.println(wide);\n\
         }\n",
    )
    .expect("write managed-result test source");
    let dir = workspace("concurrent_managed_result_gc");
    let input_name = "managed_result.zrk";
    std::fs::copy(&source, dir.join(input_name)).expect("copy managed-result source");
    let output = Command::new(compiler())
        .arg("run")
        .arg(input_name)
        .env("ZIRK_GC_THRESHOLD", "8192")
        .env("ZIRK_GC_TRACE", "1")
        .current_dir(&dir)
        .output()
        .expect("run managed-result program under forced GC");
    let _ = std::fs::remove_file(source);

    assert_eq!(
        output.status.code().unwrap_or(-1),
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[zirk-gc] collection"),
        "the managed-result regression must observe a real collection; stderr:\n{stderr}"
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "task result survived\nenum result survived\n123456789123456789012345678901234567\n"
    );
}

#[test]
fn the_channel_examples_program_runs() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("channel_examples.zrk");
    let output = zirk("run", &source, "channel_examples", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "6\n60\n4\n16\n");
}

#[test]
fn the_parallel_examples_program_runs() {
    // `examples/parallel_examples.zrk` — the normative showcase for
    // `parallel-cpu-regions`.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("parallel_examples.zrk");
    let output = zirk("run", &source, "parallel_examples", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "compressed\ncores option ok\n36\n40\n2\n9\n"
    );
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
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "Hola desde Zirk\n"
    );
}

#[test]
fn the_executable_stays_small() {
    // How much the dead-code-elimination flag (`Self::dead_strip_flag`, or
    // rather `driver::dead_strip_flag`) recovers is genuinely
    // platform-dependent, and the threshold has to respect that rather than
    // pretend otherwise:
    //
    //   - macOS:   ld64's `-dead_strip` eliminates dead code per symbol, even
    //              within a single section — verified locally at ~450 KB.
    //   - Windows: LLVM emits COMDAT sections by default for this target
    //              triple, so `/OPT:REF` gets the same fine granularity.
    //   - Linux:   neither applies. The prebuilt `std` shipped by rustup has
    //              no per-function sections (verified by inspecting its
    //              object files), so `--gc-sections` can only discard whole
    //              object files — the same granularity archive linking
    //              already had before this fix. True section splitting there
    //              needs `-Z build-std` on nightly, out of reach on the
    //              stable toolchain this project pins. In practice this means
    //              Linux's own number tracks the *whole* `zirk-runtime`
    //              object's size, not what `hello.zrk` actually calls, and
    //              grows as the runtime does — it needs a much larger,
    //              separately-tracked threshold than the other two, which
    //              stay near what this one program actually calls regardless
    //              of how big the runtime as a whole gets.
    //
    // Each threshold is generous enough to pass today with real headroom,
    // while still catching the flag being removed entirely (which inflates
    // every platform by several times over, not by a rounding error).
    #[cfg(target_os = "linux")]
    const SIZE_LIMIT: u64 = 24 * 1024 * 1024;
    #[cfg(not(target_os = "linux"))]
    const SIZE_LIMIT: u64 = 5 * 1024 * 1024;
    let source = corpus("valid").join("hello.zrk");
    let dir = workspace("binary_size");
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

    let executable = String::from_utf8_lossy(&build.stdout).trim().to_string();
    let executable = dir.join(&executable);
    let size = std::fs::metadata(&executable)
        .expect("the executable exists")
        .len();

    assert!(
        size < SIZE_LIMIT,
        "the executable grew to {} KB; dead-code elimination at link time may be missing",
        size / 1024
    );
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
fn removed_concurrency_constructs_name_their_replacement() {
    for (name, construct) in [
        ("task_await_value", "task"),
        ("await_non_task", "await"),
        ("select_removed_construct", "select"),
        (
            "cancellation_shield_removed_construct",
            "cancellation shield",
        ),
    ] {
        let source = corpus("invalid").join(format!("{name}.zrk"));
        let output = zirk("run", &source, &format!("removed_{name}"), &[]);

        assert_ne!(output.status, 0, "`{name}` should not compile");
        assert!(
            output.stderr.contains("error[E0318]"),
            "`{name}` must use the removed-construct diagnostic:\n{}",
            output.stderr
        );
        assert!(
            output
                .stderr
                .contains(&format!("`{construct}` was removed")),
            "`{name}` must name the removed construct:\n{}",
            output.stderr
        );
    }
}

#[test]
fn pending_concurrency_types_report_the_phase_gate() {
    for name in [
        "thread_pending_type",
        "rwlock_pending_type",
        "semaphore_pending_type",
        "barrier_pending_type",
        "once_pending_type",
    ] {
        let source = corpus("invalid").join(format!("{name}.zrk"));
        let output = zirk("check", &source, &format!("pending_{name}"), &[]);

        assert_ne!(output.status, 0, "`{name}` should remain unavailable");
        assert!(
            output.stderr.contains("error[E0402]"),
            "`{name}` should use the pending-type diagnostic:\n{}",
            output.stderr
        );
        assert!(
            output
                .stderr
                .contains("exists in the language but arrives in Phase 5"),
            "`{name}` should identify its delivery phase:\n{}",
            output.stderr
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn mutex_with_mutates_the_protected_payload_across_calls() {
    let source = corpus("valid").join("mutex_with_mutates_payload.zrk");
    let output = zirk("run", &source, "mutex_with_mutates_payload", &[]);
    assert_eq!(
        output.status, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), "2\n");
}

#[test]
fn mutex_roots_nested_record_references_while_mutating_inline_fields() {
    let source = corpus("valid").join("mutex_with_nested_record.zrk");
    let output = zirk_with_runtime_env(
        "run",
        &source,
        "mutex_with_nested_record",
        "mutex_with_nested_record.zrk",
        &[],
        &[("ZIRK_GC_THRESHOLD", "1")],
    );
    assert_eq!(
        output.status, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), "8\n");
}

#[test]
fn mutex_with_unlocks_after_source_exception_and_can_be_reacquired() {
    let source = corpus("valid").join("mutex_with_exception_unlock.zrk");
    let output = zirk("run", &source, "mutex_with_exception_unlock", &[]);
    assert_eq!(
        output.status, 0,
        "stdout:\n{}\nstderr:\n{}",
        output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), "E_INVALID_ARGUMENT\n9\n");
}

#[test]
fn atomic_supported_type_is_accepted_and_unsupported_payload_is_rejected() {
    let supported = corpus("valid").join("atomic_supported_type.zrk");
    let output = zirk("check", &supported, "atomic_supported_type", &[]);
    assert_eq!(
        output.status, 0,
        "Atomic<Int32> is a supported type:\n{}",
        output.stderr
    );

    let unsupported = corpus("invalid").join("atomic_unsupported_payload.zrk");
    let output = zirk("check", &unsupported, "atomic_unsupported_payload", &[]);
    assert_ne!(output.status, 0, "Atomic<String> must be rejected");
    assert!(
        output.stderr.contains("error[E0403]") || output.stderr.contains("error[E0402]"),
        "unsupported Atomic payload should fail type checking:\n{}",
        output.stderr
    );
    assert!(output.stdout.is_empty());

    let ordering = corpus("invalid").join("atomic_weak_order_requires_unsafe.zrk");
    let output = zirk("check", &ordering, "atomic_weak_order_requires_unsafe", &[]);
    assert_ne!(output.status, 0, "weaker atomic ordering requires unsafe");
    assert!(
        output
            .stderr
            .contains("non-sequentially-consistent atomic ordering"),
        "ordering violation should be diagnosed at the CLI boundary:\n{}",
        output.stderr
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn concurrent_protect_requires_non_suspending_cleanup() {
    let invalid = corpus("invalid").join("concurrent_protect_suspension_rejected.zrk");
    let output = zirk("check", &invalid, "protect_suspension_rejected", &[]);
    assert_ne!(output.status, 0, "suspending cleanup must be rejected");
    assert!(
        output.stderr.contains("error[E0483]"),
        "suspending cleanup should retain the guard diagnostic:\n{}",
        output.stderr
    );
    assert!(output.stdout.is_empty());

    let valid = corpus("valid").join("concurrent_protect_cleanup.zrk");
    let expected = std::fs::read_to_string(valid.with_extension("out"))
        .expect("valid cleanup fixture has expected output");
    let output = zirk("run", &valid, "protect_cleanup", &[]);
    assert_eq!(
        output.status, 0,
        "synchronous cleanup should run:\n{}",
        output.stderr
    );
    assert_eq!(normalize(&output.stdout), normalize(&expected));
}

#[test]
fn thread_run_executes_managed_callback_and_returns_its_result() {
    for fixture in ["thread_run_managed_result", "thread_run_throwable"] {
        let source = corpus("valid").join(format!("{fixture}.zrk"));
        let expected = std::fs::read_to_string(source.with_extension("out"))
            .expect("managed Thread.run fixture has expected output");
        let output = zirk("run", &source, fixture, &[]);
        assert_eq!(output.status, 0, "{fixture}: {}", output.stderr);
        assert_eq!(normalize(&output.stdout), normalize(&expected), "{fixture}");
    }
}

#[test]
fn thread_run_preserves_ownership_diagnostics() {
    for (fixture, ownership_code) in [
        ("thread_run_mutable_capture", "E0430"),
        ("thread_run_pointer_result", "E0482"),
    ] {
        let source = corpus("invalid").join(format!("{fixture}.zrk"));
        let output = zirk(
            "check",
            &source,
            &format!("thread_run_security_{fixture}"),
            &[],
        );

        assert_ne!(output.status, 0, "{fixture} must remain rejected");
        let ownership_reported = output.stderr.contains(&format!("error[{ownership_code}]"))
            || (fixture == "thread_run_mutable_capture" && output.stderr.contains("mutable alias"));
        assert!(
            ownership_reported,
            "{fixture} must report its ownership failure as well:\n{}",
            output.stderr
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn thread_run_checks_effects_and_accepts_complete_callbacks() {
    for (fixture, forbidden_effect) in [
        ("thread_run_transitive_channel_effect", "E0484"),
        ("thread_run_try_channel", "E0484"),
        ("thread_run_transitive_unsafe_effect", "E0484"),
    ] {
        let source = corpus("invalid").join(format!("{fixture}.zrk"));
        let output = zirk("check", &source, &format!("thread_effect_{fixture}"), &[]);
        assert_ne!(output.status, 0, "{fixture} must remain rejected");
        assert!(
            output
                .stderr
                .contains(&format!("error[{forbidden_effect}]")),
            "{fixture} must preserve the effect diagnostic alongside the gate:\n{}",
            output.stderr
        );
        assert!(output.stdout.is_empty());
    }

    let source = corpus("valid").join("thread_run_managed_result.zrk");
    let output = zirk("check", &source, "thread_pure_callback_check", &[]);
    assert_eq!(output.status, 0, "{}", output.stderr);
}

#[test]
fn concurrent_boundary_fixtures_reject_invalid_result_and_thunk_types() {
    for (fixture, expected_code) in [
        ("concurrent_of_heterogeneous_results", "E0403"),
        ("concurrent_each_void_result", "E0403"),
        ("concurrent_each_settled_void_result", "E0403"),
        ("concurrent_each_non_collection", "E0403"),
        ("concurrent_each_nontransferable_result", "E0482"),
        ("concurrent_each_nested_mutable_element", "E0482"),
    ] {
        let source = corpus("invalid").join(format!("{fixture}.zrk"));
        let output = zirk(
            "check",
            &source,
            &format!("concurrent_boundary_{fixture}"),
            &[],
        );
        assert_ne!(output.status, 0, "{fixture} must be rejected");
        assert!(
            output.stderr.contains(&format!("error[{expected_code}]")),
            "{fixture} must fail at its source type boundary:\n{}",
            output.stderr
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn ownership_boundary_corpus_covers_each_exercisable_api_boundary() {
    for (fixture, expected_code) in [
        ("spawn_mutable_capture", "E0430"),
        ("spawn_nontransferable_result", "E0482"),
        ("channel_nontransferable_send", "E0482"),
        ("channel_nontransferable_receive", "E0482"),
        ("timer_mutable_capture", "E0430"),
        ("timer_nontransferable_result", "E0482"),
        ("parallel_nontransferable_result", "E0482"),
        ("concurrent_of_mutable_capture", "E0430"),
        ("concurrent_of_nontransferable_result", "E0482"),
        ("concurrent_each_mutable_capture", "E0430"),
        ("concurrent_detach_mutable_capture", "E0430"),
        ("parallel_boundary_alias", "E0430"),
    ] {
        let source = corpus("invalid").join(format!("{fixture}.zrk"));
        let output = zirk(
            "check",
            &source,
            &format!("ownership_boundary_{fixture}"),
            &[],
        );
        assert_ne!(output.status, 0, "{fixture} must be rejected");
        assert!(
            output.stderr.contains(&format!("error[{expected_code}]")),
            "{fixture} must report its boundary ownership error {expected_code}:\n{}",
            output.stderr
        );
        assert!(
            output.stdout.is_empty(),
            "{fixture} unexpectedly wrote stdout"
        );
    }
}

#[test]
fn service_source_callbacks_register_ready_and_shutdown_cleanly() {
    for fixture in [
        "application_spawn_service_lambda",
        "application_spawn_service_named",
        "application_spawn_service_capacity_catch",
        "application_spawn_service_ready_lifecycle_error",
    ] {
        let source = corpus("valid").join(format!("{fixture}.zrk"));
        let output = zirk("run", &source, fixture, &[]);
        assert_eq!(output.status, 0, "{fixture}: {}", output.stderr);
        let expected = match fixture {
            "application_spawn_service_capacity_catch" => "E_SERVICE_CAPACITY\n",
            "application_spawn_service_ready_lifecycle_error" => "E_SERVICE_LIFECYCLE\n",
            _ => "ready\n",
        };
        assert_eq!(normalize(&output.stdout), expected, "{fixture}");
    }
}

#[test]
fn service_callback_shape_ownership_capability_and_namespace_are_checked() {
    for (fixture, code) in [
        ("application_spawn_service_wrong_signature", "E0403"),
        ("application_spawn_service_mutable_capture", "E0430"),
        ("service_context_ready_outside_callback", "E0403"),
    ] {
        let source = corpus("invalid").join(format!("{fixture}.zrk"));
        let output = zirk("check", &source, fixture, &[]);
        assert_ne!(output.status, 0, "{fixture} must be rejected");
        assert!(
            output.stderr.contains(&format!("error[{code}]")),
            "{fixture}: {}",
            output.stderr
        );
        assert!(output.stdout.is_empty());
    }

    let source = corpus("invalid").join("application_namespace_shadowed.zrk");
    let output = zirk("check", &source, "application_namespace_shadowed", &[]);
    assert_ne!(
        output.status, 0,
        "a shadowed application name is an ordinary value"
    );
    assert!(
        !output.stderr.contains("type-checked but not lowered"),
        "shadowing must disable the builtin: {}",
        output.stderr
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn concurrent_each_collection_requires_strict_deep_share_ownership() {
    for fixture in [
        "concurrent_each_mutable_list",
        "concurrent_each_inline_list",
    ] {
        let source = corpus("invalid").join(format!("{fixture}.zrk"));
        let output = zirk("check", &source, &format!("each_ownership_{fixture}"), &[]);

        assert_ne!(
            output.status, 0,
            "{fixture} must fail the ownership boundary"
        );
        assert!(
            output.stderr.contains("error[E0482]"),
            "{fixture} must fail with the ownership diagnostic:\n{}",
            output.stderr,
        );
        assert!(output.stdout.is_empty());
    }

    let strict = corpus("valid").join("concurrent_each_ordered.zrk");
    let output = zirk("check", &strict, "each_ownership_strict_list", &[]);
    assert_eq!(
        output.status, 0,
        "a deeply shareable strict binding is supported:\n{}",
        output.stderr
    );
    assert!(!output.stderr.contains("error[E0424]"), "{}", output.stderr);
}

#[test]
fn concurrent_first_wrong_arity_has_a_specific_diagnostic() {
    let source = corpus("invalid").join("concurrent_first_wrong_arity.zrk");
    let output = zirk("check", &source, "first_wrong_arity", &[]);
    assert_ne!(output.status, 0);
    assert!(output.stderr.contains("error[E0406]"), "{}", output.stderr);
    assert!(
        output
            .stderr
            .contains("`Concurrent.first()` takes no arguments"),
        "{}",
        output.stderr
    );
    assert!(!output.stderr.contains("E0424"), "{}", output.stderr);
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

// --- Colour -----------------------------------------------------------------

#[test]
fn piped_output_carries_no_colour() {
    // `ZIRK_COMPILER_SPEC.md` section 9 requires deterministic output. A test,
    // a pipe or a tool reading the diagnostics must get exactly the text.
    let source = corpus("invalid").join("type_mismatch.zrk");
    let output = zirk("run", &source, "no_colour_piped", &[]);

    assert!(
        !output.stderr.contains('\x1b'),
        "escape sequences leaked into a redirected output:\n{:?}",
        output.stderr
    );
}

#[test]
fn colour_can_be_requested_explicitly() {
    let source = corpus("invalid").join("type_mismatch.zrk");
    let output = zirk("run", &source, "colour_always", &["--color=always"]);

    assert!(output.stderr.contains('\x1b'), "{:?}", output.stderr);
}

#[test]
fn colour_can_be_disabled_explicitly() {
    let source = corpus("invalid").join("type_mismatch.zrk");
    let output = zirk("run", &source, "colour_never", &["--color=never"]);

    assert!(!output.stderr.contains('\x1b'));
}

#[test]
fn colour_does_not_change_what_the_diagnostic_says() {
    let source = corpus("invalid").join("type_mismatch.zrk");
    let plain = zirk("run", &source, "colour_cmp_plain", &["--color=never"]);
    // Use the same workspace path for both runs: source locations are part of
    // the diagnostic text, and the colour flag must be the only difference.
    let colored = zirk("run", &source, "colour_cmp_plain", &["--color=always"]);

    assert_eq!(
        strip_ansi(&colored.stderr),
        plain.stderr,
        "colour adds emphasis, it must not change the text"
    );
}

#[test]
fn the_structured_form_stays_parseable() {
    // An escape sequence inside a JSON string would break whoever reads it.
    let source = corpus("invalid").join("type_mismatch.zrk");
    let output = zirk(
        "run",
        &source,
        "json_no_colour",
        &["--json", "--color=always"],
    );

    assert!(!output.stderr.contains('\x1b'), "{:?}", output.stderr);
    assert!(output.stderr.starts_with('['));
}

/// Removes ANSI escape sequences from a text.
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();

    while let Some(c) = chars.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        for c in chars.by_ref() {
            if c.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
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
        // The CLI surface belongs to Phase 6, except for `check` which is now
        // implemented and no longer reports a future phase.
        ("test", "Phase 6"),
        ("new", "Phase 6"),
        ("doc", "Phase 7"),
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

// --- Crates of several files ------------------------------------------------

/// Writes a crate of several files and compiles its entry point.
fn crate_of(name: &str, files: &[(&str, &str)]) -> Output {
    let dir = workspace(name);

    for (path, contents) in files {
        let target = dir.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("create the directory");
        }
        std::fs::write(&target, contents).expect("write the source");
    }

    let output = Command::new(compiler())
        .arg("run")
        .arg(files[0].0)
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Runs a one-file crate while supplying deterministic process stdin.
fn crate_of_with_input(name: &str, source: &str, input: &[u8]) -> Output {
    let dir = workspace(name);
    std::fs::write(dir.join("main.zrk"), source).expect("write the source");
    let mut child = Command::new(compiler())
        .args(["run", "main.zrk"])
        .current_dir(&dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the compiler");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(input)
        .expect("write process stdin");
    let output = child.wait_with_output().expect("wait for the compiler");
    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn generic_free_function_is_monomorphized_and_executes() {
    let output = crate_of(
        "generic_free_function_monomorphization",
        &[(
            "main.zrk",
            r#"import { stdout } from std.io;
fn identity<T>(value: T): T { return value; }
fn main(): Void {
    mut value: Int32 = identity<Int32>(42);
    stdout.println(value);
    mut text: String = identity<String>("zirk");
    stdout.println(text);
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "42\nzirk\n");
}

#[test]
fn generic_free_function_specializes_nested_collection_types() {
    let output = crate_of(
        "generic_nested_collection_specialization",
        &[(
            "main.zrk",
            r#"import { stdout } from std.io;
fn preserve<T>(values: List<T>): List<T> { return values; }
fn main(): Void {
    mut original: List<Int32> = [42];
    mut copied: List<Int32> = preserve<Int32>(original);
    stdout.println(copied[0]);
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "42\n");
}

#[test]
fn generic_function_dispatches_static_contract_method_after_specialization() {
    let output = crate_of(
        "generic_static_contract_dispatch",
        &[(
            "main.zrk",
            r#"import { stdout } from std.io;
interface EnvironmentParsable<T> { static parse(text: String): T; }
class Port implements EnvironmentParsable<Port> {
    value: String;
    construct(value: String) { this.value = value; }
    static parse(text: String): Port { return Port(text); }
}

fn parse_value<T from EnvironmentParsable<T>>(text: String): T {
    return T.parse(text);
}
fn main(): Void {
    mut port: Port = parse_value<Port>("zirk");
    stdout.println(port.value);
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "zirk\n");
}

#[test]
fn generic_static_contract_can_return_and_destructure_result() {
    let output = crate_of(
        "generic_static_contract_result_dispatch",
        &[(
            "main.zrk",
            r#"import { stdout } from std.io;
interface Parsable<T> { static parse(text: String): Result<T, String>; }
class Port implements Parsable<Port> {
    value: String;
    construct(value: String) { this.value = value; }
    static parse(text: String): Result<Port, String> { return Ok(Port(text)); }
}
fn parse_value<T from Parsable<T>>(text: String): Result<T, String> {
    return match T.parse(text) {
        Ok(value) => Ok<T, String>(value);
        Error(error) => Error<T, String>(error);
    };
}
fn main(): Void {
    match parse_value<Port>("zirk") {
        Ok(port) => stdout.println(port.value);
        Error(error) => stdout.println(error);
    }
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "zirk\n");
}

#[test]
fn generic_static_method_is_monomorphized_and_executes() {
    let output = crate_of(
        "generic_static_method_monomorphization",
        &[(
            "main.zrk",
            r#"import { stdout } from std.io;
class Parser {
    static identity<T>(value: T): T { return value; }
}
fn main(): Void {
    mut value: Int32 = Parser.identity<Int32>(42);
    stdout.println(value);
    mut text: String = Parser.identity<String>("zirk");
    stdout.println(text);
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "42\nzirk\n");
}

#[test]
fn generic_static_method_substitutes_nested_result_types() {
    let output = crate_of(
        "generic_static_nested_result_specialization",
        &[(
            "main.zrk",
            r#"import { stdout } from std.io;
class Parser {
    static wrap<T>(value: T): Result<T, String> { return Ok(value); }
}
fn main(): Void {
    mut result: Result<Int32, String> = Parser.wrap<Int32>(42);
    stdout.println(result.unwrap());
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "42\n");
}

#[test]
fn a_crate_of_several_files_compiles_and_runs() {
    let output = crate_of(
        "modules_shared",
        &[
            (
                "main.zrk",
                "import { stdout } from std.io;\n\
                 import { Role, describe } from \"./domain/user\";\n\
                 fn main(): Void {\n\
                 stdout.println(describe(Role.Admin));\n\
                 stdout.println(describe(Role.Guest));\n\
                 }\n",
            ),
            (
                "domain/user.zrk",
                "share enum Role { Admin, Guest }\n\
                 share fn describe(r: Role): String {\n\
                 return match r {\n\
                 Role.Admin => \"administrador\",\n\
                 Role.Guest => \"invitado\"\n\
                 };\n\
                 }\n",
            ),
        ],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "administrador\ninvitado\n");
}

#[test]
fn a_standard_package_module_compiles_and_runs_through_the_native_backend() {
    let output = crate_of(
        "standard_package_native",
        &[(
            "main.zrk",
            "import { stdout } from std.io;\nfn main(): Void { println(\"package native\"); }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "package native\n");
}

#[test]
fn standard_http_methods_and_extension_statuses_run_natively() {
    let output = crate_of(
        "standard_http_values_native",
        &[(
            "main.zrk",
            "import { HTTP, HTTPError, HTTPHeaders, HTTPMethod, HTTPStatus, URL } from std.http;\n\
             import { stdout } from std.io;\n\
             fn main(): Void {\n\
                 mut method: HTTPMethod = HTTPMethod.Query;\n\
                 mut query_is_safe: Boolean = method.is_safe;\n\
                 mut query_is_idempotent: Boolean = method.is_idempotent;\n\
                 mut post: HTTPMethod = HTTPMethod.Post;\n\
                 mut status: HTTPStatus = HTTPStatus.Ok;\n\
                 mut status_code: Int32 = status.code;\n\
                 mut extension: HTTPStatus = HTTPStatus.Unknown(499);\n\
                 mut extension_code: Int32 = extension.code;\n\
                 mut payload_code: Int32 = match extension {\n\
                     HTTPStatus.Unknown(value) => value;\n\
                     _ => 0;\n\
                 };\n\
                 stdout.println(method);\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Query));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Patch));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Get));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Head));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Post));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Put));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Delete));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Options));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Connect));\n\
                 stdout.println(HTTP.method_name(HTTPMethod.Trace));\n\
                 match URL.parse(\"http://127.0.0.1:1/tunnel\") { Ok(url) => { mut empty: Array<Byte> = []; match HTTP.request(HTTPMethod.Connect, url, empty, HTTPHeaders.create()) { Error(error) => match error { HTTPError.UnsupportedMethod => stdout.println(\"CONNECT requires a tunnel API\"); _ => stdout.println(\"CONNECT returned the wrong error\"); }; Ok(_) => stdout.println(\"CONNECT unexpectedly opened\"); } } Error(_) => stdout.println(\"CONNECT test URL invalid\"); }\n\
                 stdout.println(HTTPMethod.Query.is_safe);\n\
                 stdout.println(query_is_safe);\n\
                 stdout.println(query_is_idempotent);\n\
                 stdout.println(post.is_safe);\n\
                 stdout.println(post.is_idempotent);\n\
                 stdout.println(status_code);\n\
                 stdout.println(status.value);\n\
                 stdout.println(status.is_success);\n\
                 stdout.println(extension_code);\n\
                 stdout.println(extension.is_client_error);\n\
                 stdout.println(extension.is_server_error);\n\
                 stdout.println(payload_code);\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "Query\nQUERY\nPATCH\nGET\nHEAD\nPOST\nPUT\nDELETE\nOPTIONS\nCONNECT\nTRACE\nCONNECT requires a tunnel API\ntrue\ntrue\ntrue\nfalse\nfalse\n200\n200\ntrue\n499\ntrue\nfalse\n499\n"
    );
}

#[test]
fn invalid_named_http_methods_fail_before_permission_lookup() {
    let project = workspace("invalid_named_http_method");
    let home = workspace("invalid_named_http_method_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"invalid_named_http_method\"\nkind application\nentry \"src/main.zrk\"\npermissions {\nenvironment {\nread: { names: [\"APP_MODE\"]; during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        r#"import { HTTP, HTTPBody, HTTPClient, HTTPError, HTTPHeaders, HTTPResponse, URL } from std.http;
import { stdout } from std.io;
fn report(result: Result<HTTPResponse, HTTPError>): Void {
    match result {
        Ok(_) => stdout.println("invalid method accepted");
        Error(error) => match error {
            HTTPError.InvalidArgument => stdout.println("invalid argument");
            _ => stdout.println(error.to_string());
        }
    }
}
fn main(): Void {
    match URL.parse("http://127.0.0.1:1/") {
        Error(error) => stdout.println(error.to_string());
        Ok(url) => {
            mut headers: HTTPHeaders = HTTPHeaders.create();
            mut body: Array<Byte> = Array(0);
            mut empty_chunk: Array<Byte> = Array(0);
            report(HTTP.request_named("BAD METHOD", url, body, headers));
            report(HTTP.request_stream_named(
                "BAD METHOD", url, headers,
                (_index: UInt64): Array<Byte> => empty_chunk,
            ));
            match HTTPClient.create() {
                Error(error) => stdout.println(error.to_string());
                Ok(client) => {
                    report(client.request_named("BAD METHOD", url, body, headers));
                    mut captured_byte: Int32 = 65;
                    mut streaming_body = HTTPBody.Stream(
                        headers,
                        (_index: UInt64): Array<Byte> => {
                            mut stream_chunk: Array<Byte> = Array(1);
                            stream_chunk[0] = captured_byte as Byte;
                            return stream_chunk;
                        },
                    );
                    report(client.request_named_body("BAD METHOD", url, streaming_body));
                    match streaming_body {
                        HTTPBody.Stream(_, producer) => {
                            mut captured_result = producer(0 as UInt64);
                            stdout.println(captured_result[0] as Int32);
                        }
                        _ => stdout.println("wrong body variant");
                    }
                    _ = client.close();
                }
            }
        }
    }
}"#,
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit invalid-method fixture approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run invalid-method fixture");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "invalid argument\ninvalid argument\ninvalid argument\ninvalid argument\n65\n"
    );
}

#[test]
fn checked_http_response_constructor_enforces_status_and_body_bounds() {
    let output = crate_of(
        "checked_http_response_constructor",
        &[(
            "main.zrk",
            "import { HTTPError, HTTPResponse } from std.http;\n\
             import { stdout } from std.io;\n\
             fn main(): Void {\n\
                 mut empty: Array<Byte> = Array(0);\n\
                 match HTTPResponse.try_from_bytes(200, empty) { Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }\n\
                 match HTTPResponse.try_from_bytes(99, empty) { Ok(_) => stdout.println(\"invalid low status accepted\"); Error(error) => stdout.println(error.to_string()); }\n\
                 match HTTPResponse.try_from_bytes(600, empty) { Ok(_) => stdout.println(\"invalid high status accepted\"); Error(error) => stdout.println(error.to_string()); }\n\
                 mut oversized: Array<Byte> = Array(16777217);\n\
                 match HTTPResponse.try_from_bytes(200, oversized) { Ok(_) => stdout.println(\"oversized response accepted\"); Error(error) => stdout.println(error.to_string()); }\n\
                 mut expected: HTTPError = HTTPError.InvalidStatus;\n\
                 _ = expected;\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "200\nInvalidStatus\nInvalidStatus\nBodyTooLarge\n"
    );
}

#[test]
fn standard_http_response_text_and_json_constructors_set_content_type() {
    let output = crate_of(
        "http_response_text_json_constructors",
        &[(
            "main.zrk",
            r#"import { HTTPError, HTTPResponse } from std.http;
import { stdout } from std.io;
fn main(): Void {
    mut text = HTTPResponse.from_text(201, "hello");
    stdout.println(text.status_code());
    match text.bytes() { Ok(body) => stdout.println(body.length); Error(error) => stdout.println(error.to_string()); }
    match text.text() { Ok(_) => stdout.println("duplicate body read accepted"); Error(error) => match error { HTTPError.BodyConsumed => stdout.println("body consumed"); _ => stdout.println("unexpected body error"); } }
    mut text_only = HTTPResponse.from_text(203, "world");
    match text_only.text() { Ok(body) => stdout.println(body); Error(error) => stdout.println(error.to_string()); }
    match text_only.bytes() { Ok(_) => stdout.println("duplicate text read accepted"); Error(error) => match error { HTTPError.BodyConsumed => stdout.println("body consumed"); _ => stdout.println("unexpected body error"); } }
    match text.header_values("CONTENT-TYPE") { Ok(values) => stdout.println(values[0]); Error(_) => stdout.println("missing text content type"); }
    mut json = HTTPResponse.from_json(202, "true");
    stdout.println(json.status_code());
    match json.header_values("content-type") { Ok(values) => stdout.println(values[0]); Error(_) => stdout.println("missing json content type"); }
    match HTTPResponse.try_from_text(99, "bad") { Ok(_) => stdout.println("invalid text status accepted"); Error(error) => stdout.println(error.to_string()); }
    match HTTPResponse.try_from_json(200, "true") { Ok(response) => match response.header_values("content-type") { Ok(values) => stdout.println(values[0]); Error(_) => stdout.println("missing checked json type"); }; Error(error) => stdout.println(error.to_string()); }
}"#,
        )],
    );

    assert_eq!(
        output.status, 0,
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status, output.stdout, output.stderr
    );
    assert_eq!(
        normalize(&output.stdout),
        "201\n5\nbody consumed\nworld\nbody consumed\ntext/plain; charset=utf-8\n202\napplication/json\nInvalidStatus\napplication/json\n"
    );
}

#[test]
fn http_response_json_decoding_uses_a_static_type_contract() {
    let output = crate_of(
        "http_response_json_static_decoder",
        &[(
            "main.zrk",
            r#"import { HTTPError, HTTPJsonParsable, HTTPResponse, decode_json } from std.http;
import { stdout } from std.io;
class ResponseId implements HTTPJsonParsable<ResponseId> {
    value: Int32;
    construct(value: Int32) { this.value = value; }
    static parse_json(document: String): Result<ResponseId, String> {
        return match Int32.parse(document) {
            Ok(value) => Ok<ResponseId, String>(ResponseId(value));
            Error(_) => Error<ResponseId, String>("expected a JSON integer");
        };
    }
}
fn main(): Void {
    mut valid = HTTPResponse.from_json(200, "42");
    match decode_json<ResponseId>(valid) { Ok(value) => stdout.println(value.value); Error(error) => stdout.println(error.to_string()); }
    mut malformed = HTTPResponse.from_json(200, "not-json");
    match decode_json<ResponseId>(malformed) { Ok(_) => stdout.println("invalid JSON accepted"); Error(error) => stdout.println(error.to_string()); }
    mut invalid_text = HTTPResponse.from_bytes(200, [255 as Byte]);
    match decode_json<ResponseId>(invalid_text) { Ok(_) => stdout.println("invalid UTF-8 accepted"); Error(error) => stdout.println(error.to_string()); }
}"#,
        )],
    );

    assert_eq!(
        output.status, 0,
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status, output.stdout, output.stderr
    );
    assert_eq!(normalize(&output.stdout), "42\nInvalidJson\nInvalidText\n");
}

#[test]
fn standard_http_headers_validate_and_preserve_repeated_values() {
    let output = crate_of(
        "http_headers_validated_collection",
        &[(
            "main.zrk",
            r#"import { HTTPHeaders } from std.http;
import { HTTPResponse } from std.http;
import { stdout } from std.io;
fn main(): Void {
    mut headers = HTTPHeaders.create();
    match headers.add("X-Tag", "one") { Ok(_) => stdout.println("added"); Error(error) => stdout.println(error.to_string()); }
    match headers.add("x-tag", "two") { Ok(_) => stdout.println("added"); Error(error) => stdout.println(error.to_string()); }
    stdout.println(headers.get("X-TAG") ?? "missing");
    mut values = headers.get_all("x-tag");
    stdout.println(values.length);
    stdout.println(values[0]);
    stdout.println(values[1]);
    match headers.set("X-Tag", "replacement") { Ok(_) => stdout.println("set"); Error(error) => stdout.println(error.to_string()); }
    values = headers.get_all("x-tag");
    stdout.println(values.length);
    stdout.println(values[0]);
    match headers.add("bad name", "value") { Ok(_) => stdout.println("invalid name accepted"); Error(error) => stdout.println(error.to_string()); }
    match headers.add("Content-Length", "0") { Ok(_) => stdout.println("framing header accepted"); Error(error) => stdout.println(error.to_string()); }
    match headers.add("safe", "line\nbreak") { Ok(_) => stdout.println("invalid value accepted"); Error(error) => stdout.println(error.to_string()); }
    mut body: Array<Byte> = [];
    match HTTPResponse.try_with_headers(201, body, headers) {
        Ok(response) => match response.header_values("x-tag") {
            Ok(response_values) => { stdout.println(response_values.length); stdout.println(response_values[0]); }
            Error(error) => stdout.println(error.to_string());
        };
        Error(error) => stdout.println(error.to_string());
    }
    match HTTPResponse.try_with_headers(99, body, headers) { Ok(_) => stdout.println("invalid status accepted"); Error(error) => stdout.println(error.to_string()); }
}"#,
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "added\nadded\none\n2\none\ntwo\nset\n1\nreplacement\nInvalidName\nForbiddenName\nInvalidValue\n1\nreplacement\nInvalidStatus\n"
    );
}

#[test]
fn standard_http_headers_enforce_the_field_count_bound() {
    let names = (0..256)
        .map(|index| format!("\"x-{index}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let mut source = format!(
        "import {{ HTTPHeaders }} from std.http;\nimport {{ stdout }} from std.io;\nfn main(): Void {{\nmut headers = HTTPHeaders.create();\nmut names: Array<String> = [{names}];\nmut index: UInt64 = 0;\nwhile index < names.length {{ _ = headers.add(names[index], \"v\"); index += 1; }}\n"
    );
    source.push_str(
        "match headers.add(\"x-overflow\", \"v\") { Ok(_) => stdout.println(\"overflow accepted\"); Error(error) => stdout.println(error.to_string()); }\nmatch headers.set(\"x-0\", \"replaced\") { Ok(_) => stdout.println(\"replacement accepted\"); Error(error) => stdout.println(error.to_string()); }\nmatch headers.set(\"x-new\", \"v\") { Ok(_) => stdout.println(\"new field accepted\"); Error(error) => stdout.println(error.to_string()); }\n}\n",
    );
    let output = crate_of("http_headers_field_count_limit", &[("main.zrk", &source)]);

    assert_eq!(
        output.status, 0,
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status, output.stdout, output.stderr
    );
    assert_eq!(
        normalize(&output.stdout),
        "TooManyFields\nreplacement accepted\nTooManyFields\n"
    );
}

#[test]
fn standard_http_headers_enforce_the_byte_limit_and_recalculate_set_size() {
    let value = "v".repeat(65_535);
    let source = format!(
        r#"import {{ HTTPHeaders }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut headers = HTTPHeaders.create();
    match headers.add("x", "{value}") {{ Ok(_) => stdout.println("boundary accepted"); Error(error) => stdout.println(error.to_string()); }}
    match headers.add("z", "v") {{ Ok(_) => stdout.println("overflow accepted"); Error(error) => stdout.println(error.to_string()); }}
    match headers.set("x", "small") {{ Ok(_) => stdout.println("space restored"); Error(error) => stdout.println(error.to_string()); }}
    match headers.add("z", "v") {{ Ok(_) => stdout.println("added after set"); Error(error) => stdout.println(error.to_string()); }}
}}"#
    );
    let output = crate_of("http_headers_byte_limit", &[("main.zrk", &source)]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "boundary accepted\nHeadersTooLarge\nspace restored\nadded after set\n"
    );
}

#[test]
fn standard_http_url_resolves_rfc_references_without_network_access() {
    let output = crate_of(
        "standard_http_url_resolve_native",
        &[(
            "main.zrk",
            "import { URL } from std.http;\n\
             import { stdout } from std.io;\n\
             fn main(): Void {\n\
                 match URL.parse(\"https://example.com/a/b\") {\n\
                     Ok(base) => match URL.resolve(base, \"../items?q=two\") {\n\
                         Ok(url) => stdout.println(url.as_string());\n\
                         Error(_) => stdout.println(\"resolution failed\");\n\
                     };\n\
                     Error(_) => stdout.println(\"base failed\");\n\
                 };\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "https://example.com/items?q=two\n"
    );
}

#[test]
fn environment_api_is_bundled_and_fails_closed_without_runtime_approval() {
    let output = crate_of(
        "standard_environment_fail_closed",
        &[(
            "main.zrk",
            "import { Env, SecretString } from std.environment;\n\
             import { stdout } from std.io;\n\
             fn main(): Void {\n\
                 match Env.get(\"ZIRK_TEST_VALUE\") {\n\
                     Ok(_) => stdout.println(\"unexpected\");\n\
                     Error(_) => stdout.println(\"denied\");\n\
                 }\n\
                 match Env.get_or(\"ZIRK_TEST_VALUE\", \"fallback\") {\n\
                     Ok(_) => stdout.println(\"unexpected fallback\");\n\
                     Error(_) => stdout.println(\"fallback denied\");\n\
                 }\n\
                 match Env.get_or_null(\"ZIRK_TEST_VALUE\") {\n\
                     Ok(_) => stdout.println(\"unexpected null fallback\");\n\
                     Error(_) => stdout.println(\"null fallback denied\");\n\
                 }\n\
                 match Env.require(\"ZIRK_TEST_VALUE\") {\n\
                     Ok(_) => stdout.println(\"unexpected required value\");\n\
                     Error(_) => stdout.println(\"require denied\");\n\
                 }\n\
                 match Env.contains(\"ZIRK_TEST_VALUE\") {\n\
                     Ok(_) => stdout.println(\"unexpected containment\");\n\
                     Error(_) => stdout.println(\"containment denied\");\n\
                 }\n\
                 match Env.list_names() {\n\
                     Ok(_) => stdout.println(\"unexpected listing\");\n\
                     Error(_) => stdout.println(\"listing denied\");\n\
                 }\n\
                 match Env.get_secret(\"ZIRK_TEST_SECRET\") {\n\
                     Ok(_) => stdout.println(\"unexpected secret\");\n\
                     Error(_) => stdout.println(\"secret denied\");\n\
                 }\n\
                 stdout.println(SecretString(\"do-not-print\").to_string());\n                 mut secret_list: List<SecretString> = List(); secret_list.add(SecretString(\"do-not-print\")); stdout.println(secret_list);\n                 mut secret_map: Map<Int32, SecretString> = Map(); secret_map.set(1, SecretString(\"do-not-print\")); stdout.println(secret_map);\n\
                 stdout.println(SecretString(\"do-not-print\"));\n\
                 stdout.println(\"secret={SecretString(\"do-not-print\")}\");\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "denied\nfallback denied\nnull fallback denied\nrequire denied\ncontainment denied\nlisting denied\nsecret denied\n<redacted>\n[<redacted>]\n{1: <redacted>}\n<redacted>\nsecret=<redacted>\n"
    );
}

#[test]
fn signed_project_approval_reaches_runtime_and_stale_manifest_denies() {
    let project = workspace("environment_approved_project");
    let home = workspace("environment_approved_home");
    let toolchain_path = std::env::var_os("PATH").expect("test toolchain PATH");
    let mut toolchain_paths = std::env::split_paths(&toolchain_path).collect::<Vec<_>>();
    if let Some(llvm_prefix) = std::env::var_os("LLVM_SYS_201_PREFIX") {
        let llvm_bin = PathBuf::from(llvm_prefix).join("bin");
        if !toolchain_paths.contains(&llvm_bin) {
            toolchain_paths.insert(0, llvm_bin);
        }
    }
    let toolchain_path =
        std::env::join_paths(toolchain_paths).expect("test toolchain PATH is valid");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let direct_package = project.join("vendor/direct");
    let nested_package = direct_package.join("vendor/nested");
    std::fs::create_dir_all(direct_package.join("src")).unwrap();
    std::fs::create_dir_all(nested_package.join("src")).unwrap();
    std::fs::write(direct_package.join("src/package.zrk"), "").unwrap();
    std::fs::write(nested_package.join("src/package.zrk"), "").unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"approval.fixture\"\nkind application\nentry \"src/main.zrk\"\ndependency acme.direct ^1 local \"vendor/direct\"\npermissions {\nenvironment {\nread: { names: [\"APPROVAL_FIXTURE_VALUE\", \"APPROVAL_FIXTURE_MISSING\", \"APPROVAL_FIXTURE_INT\", \"APPROVAL_FIXTURE_BAD_INT\", \"APPROVAL_FIXTURE_FLOAT\", \"APPROVAL_FIXTURE_BAD_FLOAT\", \"APPROVAL_FIXTURE_PORT\", \"APPROVAL_FIXTURE_BAD_PORT\", \"APPROVAL_FIXTURE_UNCAUGHT\", \"APPROVAL_FIXTURE_INVALID_UTF8\"]; during: runtime; }\nsecrets: { names: [\"APPROVAL_FIXTURE_SECRET\"]; during: runtime; }\nlist_names: { during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        direct_package.join(".zkinit"),
        "project \"acme.direct\"\nkind package\ndependency acme.nested ^1 local \"vendor/nested\"\npermissions {\nenvironment {\nread: { names: [\"APPROVAL_DIRECT_PACKAGE_VALUE\"]; during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        nested_package.join(".zkinit"),
        "project \"acme.nested\"\nkind package\npermissions {\nenvironment {\nread: { names: [\"APPROVAL_NESTED_PACKAGE_VALUE\"]; during: runtime; }\n}\n}\n",
    )
    .unwrap();
    for (root, name, dependency_root) in [
        (project.as_path(), "acme.direct", direct_package.as_path()),
        (
            direct_package.as_path(),
            "acme.nested",
            nested_package.as_path(),
        ),
    ] {
        let package_manifest =
            zirk_cli::project::read_manifest(&dependency_root.join(".zkinit")).unwrap();
        let hash =
            zirk_cli::project::package_integrity_hash(dependency_root, &package_manifest).unwrap();
        std::fs::write(
            root.join("zirk.lock"),
            format!("{name} 1.0 local sha256:{hash} src/package.zrk\n"),
        )
        .unwrap();
    }
    std::fs::write(
        project.join("src/main.zrk"),
        r#"import { Env, EnvironmentParsable, SecretString } from std.environment;
import { stderr, stdout } from std.io;

class SecretFailure implements Throwable {
    inmut value: SecretString;
    construct(value: SecretString) { this.value = value; }
    message(): String { return "failure: " + this.value.to_string(); }
    code(): String { return "SECRET_FAILURE"; }
    cause(): Error? { return null; }
    stack_trace(): StackTrace { return StackTrace(); }
}

class Port implements EnvironmentParsable<Port> {
    value: String;
    construct(value: String) { this.value = value; }
    static parse(text: String): Result<Port, String> {
        if (text == "invalid") { return Error("invalid port"); }
        return Ok(Port(text));
    }
}

fn raise_secret(value: SecretString): Void throws SecretFailure {
    throw SecretFailure(value);
}

fn main(): Void throws SecretFailure {
    match Env.get("APPROVAL_FIXTURE_VALUE") { Ok(value) => stdout.println(value); Error(_) => stdout.println("denied"); }
    match Env.get("APPROVAL_NESTED_PACKAGE_VALUE") { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get_or("APPROVAL_FIXTURE_VALUE", "wrong fallback") { Ok(value) => stdout.println(value); Error(_) => stdout.println("present fallback denied"); }
    match Env.require("APPROVAL_FIXTURE_VALUE") { Ok(value) => stdout.println(value); Error(_) => stdout.println("require denied"); }
    match Env.get_or("APPROVAL_FIXTURE_MISSING", "fallback") { Ok(value) => stdout.println(value); Error(_) => stdout.println("fallback denied"); }
    match Env.get_or<Int32>("APPROVAL_FIXTURE_MISSING", 7) { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get_or<Int32>("APPROVAL_FIXTURE_INT", 99) { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get_or<Int32>("APPROVAL_FIXTURE_BAD_INT", 99) { Ok(_) => stdout.println("unexpected typed fallback parse"); Error(error) => stdout.println(error.to_string()); }
    match Env.get_or<Port>("APPROVAL_FIXTURE_MISSING", Port("custom-fallback")) { Ok(port) => stdout.println(port.value); Error(error) => stdout.println(error.to_string()); }
    match Env.get_or_null("APPROVAL_FIXTURE_MISSING") { Ok(value) => stdout.println(value ?? "null"); Error(_) => stdout.println("null denied"); }
    match Env.contains("APPROVAL_FIXTURE_VALUE") { Ok(value) => stdout.println(value); Error(_) => stdout.println("contains denied"); }
    match Env.contains("APPROVAL_FIXTURE_MISSING") { Ok(value) => stdout.println(value); Error(_) => stdout.println("contains denied"); }
    match Env.list_names() {
        Ok(names) => {
            stdout.println(names.length > 0);
            stdout.println(names[0]);
            stdout.println(names[1]);
        }
        Error(_) => stdout.println("listing denied");
    }
    match Env.get_int32("APPROVAL_FIXTURE_MISSING") { Error(error) => stdout.println(error.to_string()); Ok(_) => stdout.println("unexpected typed value"); }
    match Env.get<Int32>("APPROVAL_FIXTURE_MISSING") { Error(error) => stdout.println(error.to_string()); Ok(_) => stdout.println("unexpected generic typed value"); }
    match Env.get_int32("APPROVAL_FIXTURE_INT") { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get_int32("APPROVAL_FIXTURE_BAD_INT") { Ok(_) => stdout.println("unexpected parsed integer"); Error(error) => stdout.println(error.to_string()); }
    match Env.get("APPROVAL_FIXTURE_INVALID_UTF8") { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.contains("APPROVAL_FIXTURE_INVALID_UTF8") { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get<Int32>("APPROVAL_FIXTURE_INT") { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get<Int32>("APPROVAL_FIXTURE_BAD_INT") { Ok(_) => stdout.println("unexpected generic parsed integer"); Error(error) => stdout.println(error.to_string()); }
    match Env.get<Port>("APPROVAL_FIXTURE_PORT") { Ok(port) => stdout.println(port.value); Error(error) => stdout.println(error.to_string()); }
    match Env.get<Port>("APPROVAL_FIXTURE_BAD_PORT") { Ok(_) => stdout.println("unexpected generic parsed port"); Error(error) => stdout.println(error.to_string()); }
    match Env.get_float64("APPROVAL_FIXTURE_FLOAT") { Ok(value) => stdout.println(value); Error(error) => stdout.println(error.to_string()); }
    match Env.get_float64("APPROVAL_FIXTURE_BAD_FLOAT") { Ok(_) => stdout.println("unexpected parsed float"); Error(error) => stdout.println(error.to_string()); }
    match Env.get_secret("APPROVAL_FIXTURE_SECRET") {
        Ok(value) => {
            stdout.println(value.to_string());
            stdout.println(value);
            stdout.println("secret={value}");
            try { raise_secret(value); } catch SecretFailure(error) { stderr.println(error.message()); }
            match Env.get_or("APPROVAL_FIXTURE_UNCAUGHT", "") {
                Ok(flag) => { if flag == "yes" { raise_secret(value); } }
                Error(_) => { }
            }
        }
        Error(_) => stdout.println("secret denied");
    }
}"#,
    )
    .unwrap();

    let mut approval_command = Command::new(compiler());
    approval_command
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("PATH", &toolchain_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    preserve_native_toolchain_environment(&mut approval_command);
    let mut approval = approval_command
        .spawn()
        .expect("start explicit permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    assert!(String::from_utf8_lossy(&approval.stdout).contains("Approval saved"));
    let approval_output = String::from_utf8_lossy(&approval.stdout);
    assert!(approval_output.contains("Required by package `acme.direct`"));
    assert!(approval_output.contains("Required by package `acme.nested`"));
    assert!(approval_output.contains("environment.read: APPROVAL_DIRECT_PACKAGE_VALUE"));
    assert!(approval_output.contains("environment.read: APPROVAL_NESTED_PACKAGE_VALUE"));
    assert!(approval_output.contains("Project and dependency fingerprint:"));
    let security_directory = approval_security_directory(&home);
    let approved_token = zirk_cli::approvals::verified_token_in(&project, &security_directory)
        .unwrap()
        .expect("the launcher should accept the persisted signed approval");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&security_directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(security_directory.join("permissions.key"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    let mut run_command = Command::new(compiler());
    run_command
        .args(["run", "."])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("PATH", &toolchain_path)
        .env("APPROVAL_FIXTURE_VALUE", "approved")
        .env("APPROVAL_NESTED_PACKAGE_VALUE", "nested-package-approved")
        .env("APPROVAL_FIXTURE_INT", "42")
        .env("APPROVAL_FIXTURE_BAD_INT", "not-an-integer")
        .env("APPROVAL_FIXTURE_FLOAT", "3.5")
        .env("APPROVAL_FIXTURE_BAD_FLOAT", "not-a-float")
        .env("APPROVAL_FIXTURE_PORT", "custom-port")
        .env("APPROVAL_FIXTURE_BAD_PORT", "invalid")
        .env("APPROVAL_FIXTURE_SECRET", "secret-value")
        .env("APPROVAL_FIXTURE_INVALID_UTF8", {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                std::ffi::OsString::from_vec(vec![0xff])
            }
            #[cfg(not(unix))]
            {
                std::ffi::OsString::from("valid-on-this-platform")
            }
        });
    preserve_native_toolchain_environment(&mut run_command);
    let run = run_command.output().expect("run with signed approval");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        if cfg!(unix) {
            "approved\nnested-package-approved\napproved\napproved\nfallback\n7\n42\nParse\ncustom-fallback\nnull\ntrue\nfalse\ntrue\nAPPROVAL_FIXTURE_BAD_FLOAT\nAPPROVAL_FIXTURE_BAD_INT\nMissing\nMissing\n42\nParse\nInvalidEncoding\nInvalidEncoding\n42\nParse\ncustom-port\nParse\n3.5\nParse\n<redacted>\n<redacted>\nsecret=<redacted>\n"
        } else {
            "approved\nnested-package-approved\napproved\napproved\nfallback\n7\n42\nParse\ncustom-fallback\nnull\ntrue\nfalse\ntrue\nAPPROVAL_FIXTURE_BAD_FLOAT\nAPPROVAL_FIXTURE_BAD_INT\nMissing\nMissing\n42\nParse\nvalid-on-this-platform\ntrue\n42\nParse\ncustom-port\nParse\n3.5\nParse\n<redacted>\n<redacted>\nsecret=<redacted>\n"
        },
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert_eq!(normalize(&stderr), "failure: <redacted>\n");
    assert!(
        !stderr.contains("secret-value"),
        "secret leaked through Throwable.message(): {stderr}"
    );

    let mut uncaught_command = Command::new(compiler());
    uncaught_command
        .args(["run", "."])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("PATH", &toolchain_path)
        .env("APPROVAL_FIXTURE_VALUE", "approved")
        .env("APPROVAL_FIXTURE_INT", "42")
        .env("APPROVAL_FIXTURE_BAD_INT", "not-an-integer")
        .env("APPROVAL_FIXTURE_FLOAT", "3.5")
        .env("APPROVAL_FIXTURE_BAD_FLOAT", "not-a-float")
        .env("APPROVAL_FIXTURE_PORT", "custom-port")
        .env("APPROVAL_FIXTURE_BAD_PORT", "invalid")
        .env("APPROVAL_FIXTURE_SECRET", "secret-value")
        .env("APPROVAL_FIXTURE_UNCAUGHT", "yes");
    preserve_native_toolchain_environment(&mut uncaught_command);
    let uncaught = uncaught_command
        .output()
        .expect("run with an uncaught secret-bearing throwable");
    assert!(
        !uncaught.status.success(),
        "the fixture must propagate the throwable"
    );
    let uncaught_stderr = String::from_utf8_lossy(&uncaught.stderr);
    assert!(
        uncaught_stderr.contains("failure: <redacted>"),
        "{uncaught_stderr}"
    );
    assert!(
        !uncaught_stderr.contains("secret-value"),
        "secret leaked through unhandled exception reporting: {uncaught_stderr}"
    );

    let mut uncaught_json_command = Command::new(compiler());
    uncaught_json_command
        .args(["run", "--json", "."])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("PATH", &toolchain_path)
        .env("APPROVAL_FIXTURE_VALUE", "approved")
        .env("APPROVAL_FIXTURE_INT", "42")
        .env("APPROVAL_FIXTURE_BAD_INT", "not-an-integer")
        .env("APPROVAL_FIXTURE_FLOAT", "3.5")
        .env("APPROVAL_FIXTURE_BAD_FLOAT", "not-a-float")
        .env("APPROVAL_FIXTURE_PORT", "custom-port")
        .env("APPROVAL_FIXTURE_BAD_PORT", "invalid")
        .env("APPROVAL_FIXTURE_SECRET", "secret-value")
        .env("APPROVAL_FIXTURE_UNCAUGHT", "yes");
    preserve_native_toolchain_environment(&mut uncaught_json_command);
    let uncaught_json = uncaught_json_command
        .output()
        .expect("run with JSON diagnostics and an uncaught secret-bearing throwable");
    assert!(
        !uncaught_json.status.success(),
        "the JSON fixture must propagate the throwable"
    );
    let json_stderr = String::from_utf8_lossy(&uncaught_json.stderr);
    let mut stderr_lines = json_stderr.lines();
    assert_eq!(
        stderr_lines.next(),
        Some("failure: <redacted>"),
        "the program's own stderr output must remain unchanged: {json_stderr}"
    );
    let diagnostic_line = stderr_lines
        .next()
        .expect("the runtime must emit a structured diagnostic");
    assert_eq!(stderr_lines.next(), None, "{json_stderr}");
    let structured: serde_json::Value = serde_json::from_str(diagnostic_line)
        .unwrap_or_else(|error| panic!("runtime diagnostic was not JSON: {error}: {json_stderr}"));
    assert_eq!(structured.as_array().map(Vec::len), Some(1));
    assert_eq!(structured[0]["code"], "E0601");
    assert!(
        !json_stderr.contains("secret-value"),
        "secret leaked through structured runtime diagnostics: {}",
        json_stderr
    );

    let manifest = std::fs::read_to_string(project.join(".zkinit")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        manifest.replace("APPROVAL_FIXTURE_VALUE", "OTHER_APPROVAL_VALUE"),
    )
    .unwrap();
    let mut stale_command = Command::new(compiler());
    stale_command
        .args(["run", "."])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("PATH", &toolchain_path)
        .env("APPROVAL_FIXTURE_VALUE", "must-not-leak")
        .env("APPROVAL_FIXTURE_INT", "900")
        .env("APPROVAL_FIXTURE_BAD_INT", "not-an-integer")
        .env("APPROVAL_FIXTURE_FLOAT", "9.5")
        .env("APPROVAL_FIXTURE_BAD_FLOAT", "not-a-float")
        .env("APPROVAL_FIXTURE_PORT", "stale-port")
        .env("APPROVAL_FIXTURE_BAD_PORT", "invalid")
        .env("APPROVAL_FIXTURE_SECRET", "must-not-leak-secret")
        .env(zirk_cli::approvals::RUNTIME_GRANT_ENV, approved_token);
    preserve_native_toolchain_environment(&mut stale_command);
    let stale = stale_command
        .output()
        .expect("run after requested scopes changed");
    assert!(
        stale.status.success(),
        "{}",
        String::from_utf8_lossy(&stale.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&stale.stdout)),
        "denied\nPermissionDenied\npresent fallback denied\nrequire denied\nfallback denied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nnull denied\ncontains denied\ncontains denied\nlisting denied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\nsecret denied\n"
    );
    let mut revoke_command = Command::new(compiler());
    revoke_command
        .args(["permissions", "revoke", "."])
        .current_dir(&project)
        .env_clear()
        .env("HOME", &home)
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("PATH", &toolchain_path);
    preserve_native_toolchain_environment(&mut revoke_command);
    let revoke = revoke_command
        .output()
        .expect("revoke the project approval");
    assert!(
        revoke.status.success(),
        "{}",
        String::from_utf8_lossy(&revoke.stderr)
    );
    assert!(
        zirk_cli::approvals::verified_token_in(&project, &security_directory)
            .unwrap()
            .is_none()
    );
}

#[test]
fn compiled_secret_http_sink_rejects_plaintext_before_connecting() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = format!("http://127.0.0.1:{port}");
    let project = workspace("secret_http_sink_plaintext_rejected");
    let home = workspace("secret_http_sink_plaintext_rejected_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"secret.http.sink.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nenvironment {{\nsecrets: {{ names: [\"SINK_TEST_SECRET\"]; during: runtime; }}\n}}\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    let source = r#"import { Env } from std.environment;
import { HTTP, HTTPHeaders, HTTPMethod, URL } from std.http;
import { stdout } from std.io;
fn main(): Void {
    match Env.get_secret("SINK_TEST_SECRET") {
        Error(_) => stdout.println("secret denied");
        Ok(secret) => match URL.parse("ORIGIN/") {
            Error(_) => stdout.println("URL rejected");
            Ok(url) => {
                mut body: Array<Byte> = [];
                mut headers: HTTPHeaders = HTTPHeaders.create();
                match HTTP.request_with_secret_header(HTTPMethod.Get, url, body, headers, "Authorization", secret) {
                    Error(_) => stdout.println("sink rejected");
                    Ok(_) => stdout.println("unexpected plaintext sink success");
                }
            }
        }
    }
}"#
        .replace("ORIGIN", &origin);
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit secret and network permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .env("SINK_TEST_SECRET", "never-send-this-in-plaintext")
        .output()
        .expect("run compiled secret sink fixture");
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "sink rejected\n"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-send-this-in-plaintext"));
    assert!(
        matches!(listener.accept(), Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

#[test]
fn bundled_std_http_get_and_head_return_managed_responses_under_an_exact_grant() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let large_http_body = vec![0; 128 * 1024];
    let server = std::thread::spawn(move || {
        let requests = [
            ("GET", &b"pong"[..]),
            ("GET", &[0xff][..]),
            ("HEAD", &b""[..]),
            ("GET", &b"hello"[..]),
            ("GET", &b"world"[..]),
            ("GET", &b"good"[..]),
            ("GET", &large_http_body[..]),
            ("GET", &large_http_body[..]),
        ];
        for (request_index, (method, response_body)) in requests.into_iter().enumerate() {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert_eq!(request_line, format!("{method} / HTTP/1.1\r\n"));
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
            }
            let mut stream = reader.into_inner();
            let headers = format!(
                "HTTP/1.1 200 OK\r\nSet-Cookie: first=1\r\nContent-Type: text/plain\r\nSet-Cookie: second=2\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            );
            let response_write = stream
                .write_all(headers.as_bytes())
                .and_then(|()| stream.write_all(response_body));
            // The last two requests intentionally close streams early, so a
            // peer reset while writing their remaining bodies is expected.
            if request_index < 6 {
                response_write.unwrap();
            }
            if request_index >= 4 {
                let mut probe = [0; 1];
                assert!(matches!(stream.read(&mut probe), Ok(0) | Err(_)));
            }
        }
    });

    let project = workspace("approved_std_http_get");
    let home = workspace("approved_std_http_get_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.get.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTP, HTTPError, HTTPStreamChunk, HTTPStreamResponse, URL }} from std.http;\n\
             import {{ stdout }} from std.io;\n\
             fn consume_stream(stream: HTTPStreamResponse): Void {{\n\
                 stdout.println(stream.status_code());\n\
                 match stream.header_values(\"set-cookie\") {{ Ok(values) => {{ stdout.println(values.length); stdout.println(values[0]); stdout.println(values[1]); }} Error(_) => stdout.println(\"stream header lookup failed\"); }}\n\
                 match stream.read_chunk(0) {{ HTTPStreamChunk.Error(error) => match error {{ HTTPError.InvalidArgument => stdout.println(\"invalid chunk size\"); _ => stdout.println(\"wrong chunk error\"); }} _ => stdout.println(\"invalid chunk size accepted\"); }}\n\
                 match stream.read_chunk(65537) {{ HTTPStreamChunk.Error(error) => match error {{ HTTPError.InvalidArgument => stdout.println(\"invalid chunk size\"); _ => stdout.println(\"wrong chunk error\"); }} _ => stdout.println(\"oversized chunk size accepted\"); }}\n\
                 match stream.read_chunk(2) {{ HTTPStreamChunk.Bytes(part) => {{ stdout.println(part[0]); stdout.println(part[1]); }} HTTPStreamChunk.End => stdout.println(\"early eof\"); HTTPStreamChunk.Error(_) => stdout.println(\"stream read failed\"); }}\n\
                 match stream.read_chunk(2) {{ HTTPStreamChunk.Bytes(part) => {{ stdout.println(part[0]); stdout.println(part[1]); }} HTTPStreamChunk.End => stdout.println(\"early eof\"); HTTPStreamChunk.Error(_) => stdout.println(\"stream read failed\"); }}\n\
                 match stream.read_chunk(2) {{ HTTPStreamChunk.Bytes(part) => stdout.println(part[0]); HTTPStreamChunk.End => stdout.println(\"early eof\"); HTTPStreamChunk.Error(_) => stdout.println(\"stream read failed\"); }}\n\
                 match stream.read_chunk(2) {{ HTTPStreamChunk.Bytes(_) => stdout.println(\"extra stream bytes\"); HTTPStreamChunk.End => stdout.println(\"stream eof\"); HTTPStreamChunk.Error(_) => stdout.println(\"stream read failed\"); }}\n\
                 stream.close();\n\
             }}\n\
             fn close_stream_early(url: URL): Void {{\n\
                 match HTTP.get_stream(url) {{\n\
                     Ok(stream) => stdout.println(stream.close());\n\
                     Error(_) => stdout.println(\"early close request failed\");\n\
                 }}\n\
             }}\n\
             fn abandon_stream(url: URL): Void {{\n\
                 match HTTP.get_stream(url) {{\n\
                     Ok(stream) => stdout.println(stream.status_code());\n\
                     Error(_) => stdout.println(\"abandoned stream open failed\");\n\
                 }}\n\
             }}\n\
             fn consume_stream_bytes(url: URL): Void {{\n\
                 match HTTP.get_stream(url) {{\n\
                     Ok(response) => {{\n\
                         match response.bytes() {{ Ok(body) => {{ stdout.println(body.length); stdout.println(body[0]); }} Error(_) => stdout.println(\"stream bytes failed\"); }}\n\
                         match response.bytes() {{ Error(error) => match error {{ HTTPError.BodyConsumed => stdout.println(\"body consumed\"); _ => stdout.println(\"other stream body error\"); }} Ok(_) => stdout.println(\"duplicate stream body read\"); }}\n\
                     }}\n\
                     Error(_) => stdout.println(\"stream bytes request failed\");\n\
                 }}\n\
             }}\n\
             fn consume_stream_text(url: URL): Void {{\n\
                 match HTTP.get_stream(url) {{\n\
                     Ok(response) => match response.text() {{ Ok(value) => stdout.println(value); Error(_) => stdout.println(\"stream text failed\"); }}\n\
                     Error(_) => stdout.println(\"stream text request failed\");\n\
                 }}\n\
             }}\n\
             fn main(): Void {{\n\
                 match URL.parse(\"HTTP://127.0.0.1:{}\") {{\n\
                     Ok(url) => {{\n\
                         stdout.println(url.as_string());\n\
                         match HTTP.get(url) {{\n\
                         Ok(response) => {{\n\
                             stdout.println(response.status_code());\n\
                             match response.bytes() {{ Ok(body) => {{ stdout.println(body.length); stdout.println(body[0]); }} Error(_) => stdout.println(\"body read failed\"); }}\n\
                             match response.bytes() {{ Ok(_) => stdout.println(\"duplicate body read accepted\"); Error(error) => match error {{ HTTPError.BodyConsumed => stdout.println(\"body consumed\"); _ => stdout.println(\"other body error\"); }} }}\n\
                             match response.header_values(\"set-cookie\") {{\n\
                                 Ok(values) => {{\n\
                                     stdout.println(values.length);\n\
                                     stdout.println(values[0]);\n\
                                     stdout.println(values[1]);\n\
                                 }}\n\
                                 Error(_) => stdout.println(\"header lookup failed\");\n\
                             }}\n\
                             match response.text() {{\n\
                                 Ok(_) => stdout.println(\"duplicate body text accepted\");\n\
                                 Error(error) => match error {{ HTTPError.BodyConsumed => stdout.println(\"body consumed\"); _ => stdout.println(\"other text error\"); }}\n\
                             }}\n\
                             match HTTP.get(url) {{\n\
                                 Ok(invalid_response) => match invalid_response.text() {{\n\
                                     Ok(_) => stdout.println(\"malformed UTF-8 accepted\");\n\
                                     Error(error) => match error {{\n\
                                         HTTPError.InvalidText => stdout.println(\"invalid text\");\n\
                                         _ => stdout.println(\"other text error\");\n\
                                     }}\n\
                                 }}\n\
                                 Error(_) => stdout.println(\"second request failed\");\n\
                             }}\n\
                             match HTTP.head(url) {{\n\
                                 Ok(head_response) => {{\n\
                                     stdout.println(head_response.status_code());\n\
                                     match head_response.bytes() {{ Ok(body) => stdout.println(body.length); Error(_) => stdout.println(\"HEAD body read failed\"); }}\n\
                                 }}\n\
                                 Error(_) => stdout.println(\"HEAD request failed\");\n\
                             }}\n\
                             match HTTP.get_stream(url) {{\n\
                                 Ok(stream) => consume_stream(stream);\n\
                                 Error(_) => stdout.println(\"stream request failed\");\n\
                                 }}\n\
                             consume_stream_bytes(url);\n\
                             consume_stream_text(url);\n\
                             close_stream_early(url);\n\
                             abandon_stream(url);\n\
                         }}\n\
                         Error(_) => stdout.println(\"request failed\");\n\
                         }}\n\
                     }}\n\
                     Error(_) => stdout.println(\"invalid URL\");\n\
                 }}\n\
             }}",
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved std.http GET program");
    eprintln!(
        "run status={} stdout={} stderr={}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.status.success());
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        format!(
            "http://127.0.0.1:{}/\n200\n4\n112\nbody consumed\n2\nfirst=1\nsecond=2\nbody consumed\ninvalid text\n200\n0\n200\n2\nfirst=1\nsecond=2\ninvalid chunk size\ninvalid chunk size\n104\n101\n108\n108\n111\nstream eof\n5\n119\nbody consumed\ngood\ntrue\n200\n",
            address.port()
        )
    );
    server.join().unwrap();
}

#[test]
fn bundled_std_http_websocket_server_dispatches_approved_owner_task_session() {
    use futures_util::{SinkExt, StreamExt};
    use std::io::Write;
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let peer = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
            let endpoint = format!("ws://{address}/socket");
            let mut malformed_stream = None;
            for _ in 0..200 {
                match tokio::net::TcpStream::connect(address).await {
                    Ok(connected) => { malformed_stream = Some(connected); break; }
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(25)).await,
                }
            }
            let mut malformed_stream = malformed_stream.expect("Zirk WebSocket server should start within five seconds");
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            malformed_stream.write_all(b"GET /socket HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").await.unwrap();
            let mut malformed_response = Vec::new();
            malformed_stream.read_to_end(&mut malformed_response).await.unwrap();
            assert!(malformed_response.starts_with(b"HTTP/1.1 400"), "{malformed_response:?}");

            let denied_stream = tokio::net::TcpStream::connect(address).await.unwrap();
            let mut denied = endpoint.as_str().into_client_request().unwrap();
            denied.headers_mut().insert("origin", "https://evil.example".parse().unwrap());
            assert!(matches!(tokio_tungstenite::client_async(denied, denied_stream).await,
                Err(tokio_tungstenite::tungstenite::Error::Http(response)) if response.status() == http::StatusCode::FORBIDDEN));

            let stream = tokio::net::TcpStream::connect(address).await.unwrap();
            let mut request = endpoint.as_str().into_client_request().unwrap();
            request.headers_mut().insert("origin", "https://app.example".parse().unwrap());
            request.headers_mut().insert("sec-websocket-protocol", "zirk.v1, zirk.v2".parse().unwrap());
            let (mut websocket, response) = tokio_tungstenite::client_async(request, stream).await.unwrap();
            assert_eq!(response.headers().get("sec-websocket-protocol").unwrap(), "zirk.v2");
            websocket.send(Message::Text("hello".into())).await.unwrap();
            assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Text(text) if text == "echo:hello"));
            websocket.close(None).await.unwrap();
            assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Close(_)));

            let stream = tokio::net::TcpStream::connect(address).await.unwrap();
            let mut request = endpoint.as_str().into_client_request().unwrap();
            request.headers_mut().insert("origin", "https://app.example".parse().unwrap());
            let (mut websocket, _) = tokio_tungstenite::client_async(request, stream).await.unwrap();
            let _ = websocket.send(Message::Binary(vec![0; 16 * 1024 * 1024 + 1].into())).await;
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), websocket.next()).await;
        });
    });

    let project = workspace("approved_std_http_websocket_server");
    let home = workspace("approved_std_http_websocket_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(project.join(".zkinit"), format!(
        "project \"std.http.websocket.server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n", address.port()
    )).unwrap();
    std::fs::write(project.join("src/main.zrk"), format!(r#"
import {{ HTTPError, HTTPServer, HTTPWebSocket, HTTPWebSocketMessage }} from std.http;
import {{ TCPListener }} from std.net::tcp;
import {{ stdout }} from std.io;
fn echo(socket: HTTPWebSocket, value: String): Void {{
    _ = socket.send(HTTPWebSocketMessage.Text("echo:" + value));
}}
fn ignore(): Void {{ return; }}
fn report(error: HTTPError): Void {{ stdout.println(error.to_string()); }}
fn process(socket: HTTPWebSocket, message: HTTPWebSocketMessage): Void {{
    match message {{
        HTTPWebSocketMessage.Text(value) => echo(socket, value);
        _ => ignore();
    }};
}}
fn handle(socket: HTTPWebSocket): Void {{
    match socket.receive() {{
        Ok(message) => process(socket, message);
        Error(error) => report(error);
    }}
}}
fn main(): Void {{
    match TCPListener.bind("tcp://127.0.0.1:{0}") {{
        Ok(listener) => {{
            mut origins: List<String> = List(); origins.add("https://app.example");
            mut protocols: List<String> = List(); protocols.add("zirk.v2"); protocols.add("zirk.v1");
            mut count: Int32 = 0;
            while count < 4 {{
                match listener.accept() {{
                    Ok(stream) => match HTTPServer.serve_websocket_once(stream, "/socket", origins, protocols, 8, handle) {{
                        Ok(_) => stdout.println("served"); Error(error) => stdout.println("server-error:" + error.to_string());
                    }};
                    Error(_) => stdout.println("accept-error");
                }}
                count += 1;
            }}
            _ = listener.close();
        }}
        Error(_) => stdout.println("listen-error");
    }}
}}
"#, address.port())).unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let output = normalize(&String::from_utf8_lossy(&run.stdout));
    assert!(output.starts_with("served\nserved\nserved\n"), "{output}");
    assert!(output.contains("BodyTooLarge"), "{output}");
    peer.join().unwrap();
}

#[test]
fn bundled_std_http_tls_websocket_server_dispatches_approved_owner_task_session() {
    use std::io::Write;
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"
import {{ HTTPError, HTTPServer, HTTPWebSocket, HTTPWebSocketMessage }} from std.http;
import {{ TLSListener }} from std.net::tls;
import {{ stdout }} from std.io;
fn echo(socket: HTTPWebSocket, value: String): Void {{
    _ = socket.send(HTTPWebSocketMessage.Text("tls:" + value));
}}
fn ignore(): Void {{ return; }}
fn process(socket: HTTPWebSocket, message: HTTPWebSocketMessage): Void {{
    match message {{ HTTPWebSocketMessage.Text(value) => echo(socket, value); _ => ignore(); }};
}}
fn handle(socket: HTTPWebSocket): Void {{
    match socket.receive() {{ Ok(message) => process(socket, message); Error(_) => ignore(); }}
    match socket.receive() {{ Ok(message) => process(socket, message); Error(_) => ignore(); }}
}}
fn main(): Void {{
    mut cert: Array<Byte> = [{certificate}];
    mut key: Array<Byte> = [{private_key}];
    mut chain: Array<Array<Byte>> = [cert];
    mut origins: List<String> = List(); origins.add("https://app.example");
    mut protocols: List<String> = List();
    match HTTPServer.bind_tls("tcp://127.0.0.1:{port}", chain, key) {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => match listener.accept() {{
            Error(_) => stdout.println("accept failed");
            Ok(stream) => match HTTPServer.serve_tls_websocket_once(stream, "/socket", origins, protocols, 1, handle) {{
                Ok(_) => stdout.println("served");
                Error(error) => stdout.println("server-error:" + error.to_string());
            }};
        }};
    }}
}}
"#,
        certificate = byte_literal(certificate),
        private_key = byte_literal(private_key)
    );

    let project = workspace("approved_std_http_tls_websocket_server");
    let home = workspace("approved_std_http_tls_websocket_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(project.join(".zkinit"), format!(
        "project \"std.http.tls.websocket.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
    )).unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let client = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("TLS WebSocket connect failed: {error}")),
            }
        };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                root.to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let mut config = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let config = std::sync::Arc::new(config);
        let name = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
            .map_err(|error| error.to_string())?;
        let connection = tokio_rustls::rustls::ClientConnection::new(config, name)
            .map_err(|error| error.to_string())?;
        let tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
        let request = format!("wss://localhost:{port}/socket")
            .into_client_request()
            .map_err(|error| error.to_string())?;
        let mut request = request;
        request
            .headers_mut()
            .insert("origin", "https://app.example".parse().unwrap());
        let (mut websocket, _) = tokio_tungstenite::tungstenite::client(request, tls)
            .map_err(|error| error.to_string())?;
        websocket
            .send(Message::Text("hello".into()))
            .map_err(|error| error.to_string())?;
        if !matches!(websocket.read().map_err(|error| error.to_string())?, Message::Text(text) if text == "tls:hello")
        {
            return Err("TLS WebSocket callback returned an unexpected echo".into());
        }
        websocket
            .send(Message::Close(None))
            .map_err(|error| error.to_string())?;
        match websocket.read().map_err(|error| error.to_string())? {
            Message::Close(Some(frame))
                if u16::from(frame.code) == 1001 && frame.reason == "message limit reached" => {}
            message => return Err(format!("unexpected bounded-session close: {message:?}")),
        }
        websocket.flush().map_err(|error| error.to_string())?;
        Ok(())
    });
    let result = client.join().expect("TLS WebSocket peer should not panic");
    if let Err(error) = result {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "{error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "served\n",
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[allow(clippy::result_large_err)]
fn bundled_std_http_websocket_client_exchanges_typed_messages_with_approved_origin() {
    use futures_util::{SinkExt, StreamExt};
    use std::io::Write;
    use tokio_tungstenite::tungstenite::Message;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let (socket, _) = listener.accept().await.unwrap();
                let mut websocket = tokio_tungstenite::accept_hdr_async(
                    socket,
                    |request: &http::Request<()>, mut response: http::Response<()>| {
                        assert_eq!(request.headers().get("origin").unwrap(), "https://app.example");
                        assert_eq!(request.headers().get("sec-websocket-protocol").unwrap(), "zirk.echo.v1");
                        response.headers_mut().insert("sec-websocket-protocol", "zirk.echo.v1".parse().unwrap());
                        Ok(response)
                    },
                ).await.unwrap();
                assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Text(text) if text == "client-text"));
                websocket.send(Message::Text("server-text".into())).await.unwrap();
                websocket.send(Message::Binary(vec![7, 8].into())).await.unwrap();
                websocket.send(Message::Ping(vec![1, 2, 3, 4, 5].into())).await.unwrap();
                assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Pong(bytes) if bytes.as_ref() == [1, 2, 3, 4, 5]));
                assert!(matches!(websocket.next().await.unwrap().unwrap(), Message::Binary(bytes) if bytes.as_ref() == [9, 10]));
                assert!(matches!(
                    websocket.next().await.unwrap().unwrap(),
                    Message::Close(Some(frame))
                        if frame.code == tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal
                            && frame.reason == "done"
                ));
                websocket.flush().await.unwrap();
            });
    });

    let project = workspace("approved_std_http_websocket_client");
    let home = workspace("approved_std_http_websocket_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.websocket.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
        r#"import {{ HTTPError, HTTPWebSocket, HTTPWebSocketMessage, URL }} from std.http;
import {{ stdout }} from std.io;
fn show_message(message: HTTPWebSocketMessage): Void {{
    match message {{
        HTTPWebSocketMessage.Text(value) => stdout.println("text:" + value);
        HTTPWebSocketMessage.Binary(bytes) => stdout.println("binary:" + bytes.length.to_string());
        HTTPWebSocketMessage.Ping(bytes) => stdout.println("ping:" + bytes.length.to_string());
        HTTPWebSocketMessage.Pong(bytes) => stdout.println("pong:" + bytes.length.to_string());
        _ => stdout.println("unexpected message");
    }}
}}
fn show_result(result: Result<HTTPWebSocketMessage, HTTPError>): Void {{
    match result {{ Ok(message) => show_message(message); Error(_) => stdout.println("receive failed"); }}
}}
fn send_text(socket: HTTPWebSocket): Void {{
    match socket.send(HTTPWebSocketMessage.Text("client-text")) {{ Ok(_) => stdout.println("sent text"); Error(_) => stdout.println("send text failed"); }}
}}
fn reject_oversized_message(socket: HTTPWebSocket): Void {{
    mut bytes: Array<Byte> = Array(16777217);
    match socket.send(HTTPWebSocketMessage.Binary(bytes)) {{
        Error(error) => stdout.println(error.to_string());
        _ => stdout.println("oversized accepted");
    }}
}}
fn send_binary(socket: HTTPWebSocket): Void {{
    mut bytes: Array<Byte> = Array(2);
    bytes[0] = 9 as Byte; bytes[1] = 10 as Byte;
    match socket.send(HTTPWebSocketMessage.Binary(bytes)) {{ Ok(_) => stdout.println("sent binary"); Error(_) => stdout.println("send binary failed"); }}
}}
fn exercise(url: URL): Void {{
    mut protocols: List<String> = List();
    protocols.add("zirk.echo.v1");
    match HTTPWebSocket.connect_with_options(url, "HTTPS://App.Example:443", protocols) {{
        Ok(socket) => {{
            reject_oversized_message(socket);
            send_text(socket);
            show_result(socket.receive());
            show_result(socket.receive());
            show_result(socket.receive());
            send_binary(socket);
            match socket.close_with(1000 as UInt16, "done") {{ Ok(_) => stdout.println("closed"); Error(_) => stdout.println("close failed"); }};
        }}
        Error(error) => match error {{
            HTTPError.InvalidUrl => stdout.println("connect invalid url");
            HTTPError.PermissionDenied => stdout.println("connect permission denied");
            HTTPError.Transport => stdout.println("connect transport");
            HTTPError.Protocol => stdout.println("connect protocol");
            HTTPError.Timeout => stdout.println("connect timeout");
            _ => stdout.println("connect other");
        }}
    }};
}}
fn main(): Void {{
    match URL.parse("http://127.0.0.1:{}/socket") {{
        Ok(url) => exercise(url);
        Error(_) => stdout.println("invalid URL");
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("approve WebSocket origin access");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved WebSocket client");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "BodyTooLarge\nsent text\ntext:server-text\nbinary:2\nping:5\nsent binary\nclosed\n"
    );
    server.join().unwrap();
}

#[test]
fn bundled_std_http2_websocket_client_uses_custom_trust_roots() {
    use futures_util::{SinkExt, StreamExt};
    use hyper::body::Incoming;
    use hyper::service::service_fn;
    use hyper::{Request, Response};
    use hyper_util::rt::TokioIo;
    use tokio_tungstenite::tungstenite::{Message, protocol::Role};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let port = address.port();
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let peer = std::thread::spawn(move || {
        use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let (socket, _) = listener.accept().await.unwrap();
            let mut config = tokio_rustls::rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![CertificateDer::from(certificate.to_vec())],
                    PrivateKeyDer::try_from(private_key.to_vec()).unwrap(),
                )
                .unwrap();
            config.alpn_protocols = vec![b"h2".to_vec()];
            let tls = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(config))
                .accept(socket)
                .await
                .unwrap();
            let service = service_fn(|mut request: Request<Incoming>| async move {
                assert_eq!(request.method(), hyper::Method::CONNECT);
                assert_eq!(request.uri().path(), "/socket");
                assert_eq!(
                    request
                        .extensions()
                        .get::<hyper::ext::Protocol>()
                        .unwrap()
                        .as_str(),
                    "websocket"
                );
                assert_eq!(
                    request.headers().get("origin").unwrap(),
                    "https://app.example"
                );
                assert_eq!(
                    request.headers().get("sec-websocket-protocol").unwrap(),
                    "zirk.h2.v1"
                );
                let on_upgrade = hyper::upgrade::on(&mut request);
                tokio::spawn(async move {
                    let upgraded = on_upgrade.await.unwrap();
                    let io = TokioIo::new(upgraded);
                    let mut websocket =
                        tokio_tungstenite::WebSocketStream::from_raw_socket(io, Role::Server, None)
                            .await;
                    while let Some(message) = websocket.next().await {
                        match message.unwrap() {
                            Message::Text(text) => {
                                websocket
                                    .send(Message::Text(format!("echo:{text}").into()))
                                    .await
                                    .unwrap();
                            }
                            Message::Close(_) => {
                                websocket.flush().await.unwrap();
                                break;
                            }
                            _ => panic!("unexpected WebSocket frame"),
                        }
                    }
                });
                Ok::<_, std::convert::Infallible>(
                    Response::builder()
                        .status(hyper::StatusCode::OK)
                        .header("sec-websocket-protocol", "zirk.h2.v1")
                        .body(http_body_util::Full::new(bytes::Bytes::new()))
                        .unwrap(),
                )
            });
            let _ = hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                .enable_connect_protocol()
                .serve_connection(TokioIo::new(tls), service)
                .await;
        });
    });

    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let bytes = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let project = workspace("approved_std_http2_websocket_client");
    let home = workspace("approved_std_http2_websocket_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!("project \"std.http2.websocket.client.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"https://127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"),
    ).unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(r#"import {{ HTTPWebSocket, HTTPWebSocketMessage, URL }} from std.http;
import {{ stdout }} from std.io;
fn ignore(): Void {{ return; }}
fn main(): Void {{
    mut root: Array<Byte> = [{root}];
    mut roots: Array<Array<Byte>> = [root];
    mut protocols: List<String> = List();
    protocols.add("zirk.h2.v1");
    match URL.parse("https://127.0.0.1:{port}/socket") {{
        Error(_) => stdout.println("bad url");
        Ok(url) => match HTTPWebSocket.connect_with_options_and_trust_roots(url, "https://app.example", protocols, roots, false) {{
            Error(error) => stdout.println("connect:" + error.to_string());
            Ok(socket) => {{
                match socket.send(HTTPWebSocketMessage.Text("hello")) {{
                    Error(error) => stdout.println("send:" + error.to_string());
                    Ok(_) => match socket.receive() {{
                        Error(error) => stdout.println("receive:" + error.to_string());
                        Ok(message) => match message {{
                            HTTPWebSocketMessage.Text(text) => stdout.println(text);
                            _ => stdout.println("unexpected frame");
                        }};
                    }};
                }}
                match socket.close() {{ Ok(_) => ignore(); Error(_) => ignore(); }};
            }}
        }};
    }}
}}"#, root = bytes(root)),
    ).unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "echo:hello\n"
    );
    peer.join().unwrap();
}

#[test]
fn bundled_std_http3_server_api_uses_the_official_host_binding() {
    let project = workspace("bundled_std_http3_server_api");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.http.http3.server.api.fixture\"\nkind application\nentry \"src/main.zrk\"\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        r#"import { HTTPServer, HTTPRequest, HTTPResponse } from std.http;
import { stdout } from std.io;
fn main(): Void {
    mut certificates: Array<Array<Byte>> = [];
    mut private_key: Array<Byte> = [];
    match HTTPServer.serve_http3(
        "udp://127.0.0.1:8443",
        certificates,
        private_key,
        1,
        1,
        (request: HTTPRequest): HTTPResponse => HTTPResponse.from_text(200, "ok"),
    ) {
        Error(error) => stdout.println(error.to_string());
        Ok(count) => stdout.println(count);
    }
}"#,
    )
    .unwrap();

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "Protocol\n"
    );
}

#[test]
fn bundled_std_http3_websocket_server_fails_closed_without_runtime_feature() {
    let project = workspace("bundled_std_http3_websocket_server");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.http.http3.websocket.server.fixture\"\nkind application\nentry \"src/main.zrk\"\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        r#"import { HTTPError, HTTPServer, HTTPWebSocket } from std.http;
import { stdout } from std.io;
fn handle(socket: HTTPWebSocket): Void { _ = socket.close(); }
fn main(): Void {
    mut certificates: Array<Array<Byte>> = [];
    mut private_key: Array<Byte> = [];
    mut origins: List<String> = List();
    mut protocols: List<String> = List();
    match HTTPServer.serve_http3_websocket_once(
        "udp://127.0.0.1:8444", certificates, private_key, "/socket",
        origins, protocols, 1, handle,
    ) {
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("served");
    }
}"#,
    )
    .unwrap();

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "Protocol\n"
    );
}

#[test]
fn bundled_std_http3_websocket_fails_closed_without_http3_runtime_feature() {
    let project = workspace("bundled_std_http3_websocket_client");
    let home = workspace("bundled_std_http3_websocket_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.http.http3.websocket.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {\nnetwork {\nconnect: { origins: [\"https://127.0.0.1:443\"]; during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        r#"import { HTTPWebSocket, URL } from std.http;
import { stdout } from std.io;
fn main(): Void {
    mut protocols: List<String> = List();
    mut root: Array<Byte> = [1 as Byte];
    mut roots: Array<Array<Byte>> = [root];
    match URL.parse("https://127.0.0.1/socket") {
        Error(_) => stdout.println("InvalidUrl");
        Ok(url) => {
            match HTTPWebSocket.connect_http3(url, "https://app.example", protocols) {
                Error(error) => stdout.println(error.to_string());
                Ok(_) => stdout.println("connected");
            }
            match HTTPWebSocket.connect_http3_with_trust_roots(
                url, "https://app.example", protocols, roots,
            ) {
                Error(error) => stdout.println(error.to_string());
                Ok(_) => stdout.println("connected with roots");
            }
        }
    }
}"#,
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "Protocol\nProtocol\n"
    );
}

#[test]
fn bundled_std_http1_client_interoperates_with_independent_hyper_peer() {
    use hyper::service::service_fn;
    use std::convert::Infallible;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let (socket, _) = listener.accept().unwrap();
            socket.set_nonblocking(true).unwrap();
            let socket = tokio::net::TcpStream::from_std(socket).unwrap();
            let service = service_fn(
                |request: hyper::Request<hyper::body::Incoming>| async move {
                    assert_eq!(request.method(), hyper::Method::GET);
                    assert_eq!(request.uri().path(), "/interop");
                    Ok::<_, Infallible>(hyper::Response::new(http_body_util::Full::new(
                        bytes::Bytes::from_static(b"hyper-http1-peer"),
                    )))
                },
            );
            hyper::server::conn::http1::Builder::new()
                .serve_connection(hyper_util::rt::TokioIo::new(socket), service)
                .await
                .unwrap();
        });
    });

    let project = workspace("approved_std_http1_hyper_peer");
    let home = workspace("approved_std_http1_hyper_peer_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!("project \"std.http1.hyper.peer.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTP, URL }} from std.http;\nimport {{ stdout }} from std.io;\nfn main(): Void {{\nmatch URL.parse(\"{origin}/interop\") {{\nError(_) => stdout.println(\"URL parse failed\");\nOk(url) => match HTTP.get(url) {{\nError(error) => stdout.println(error.to_string());\nOk(response) => {{ stdout.println(response.status_code()); stdout.println(response.protocol().to_string()); match response.text() {{ Ok(body) => stdout.println(body); Error(error) => stdout.println(error.to_string()); }} }}\n}}\n}}\n}}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP/1.1 peer approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run compiled-Zirk HTTP/1.1 client against Hyper peer");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nHTTP1\nhyper-http1-peer\n"
    );
    peer.join().unwrap();
}

#[test]
fn bundled_std_http_client_reuses_connection_and_scopes_cookie_jar() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        for request_index in 0..25 {
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let expected_method = match request_index {
                22 => "POST",
                23 => "QUERY",
                _ => "GET",
            };
            let expected_path = if request_index == 24 {
                "/duplicate-location"
            } else {
                "/client"
            };
            assert_eq!(
                request_line,
                format!("{expected_method} {expected_path} HTTP/1.1\r\n")
            );
            let mut cookie_seen = false;
            let mut json_content_type = false;
            let mut content_length = 0usize;
            let mut chunked = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                if header.to_ascii_lowercase().starts_with("cookie:") {
                    cookie_seen = header.contains("session=secret");
                }
                if header.to_ascii_lowercase().starts_with("content-length:") {
                    content_length = header[15..].trim().parse().unwrap();
                }
                if header
                    .to_ascii_lowercase()
                    .starts_with("transfer-encoding: chunked")
                {
                    chunked = true;
                }
                if request_index == 22
                    && header
                        .to_ascii_lowercase()
                        .starts_with("content-type: application/json")
                {
                    json_content_type = true;
                }
            }
            assert_eq!(cookie_seen, request_index > 0);
            if request_index == 22 {
                assert!(
                    json_content_type,
                    "JSON client body must set its media type"
                );
            }
            if request_index == 22 {
                assert_eq!(content_length, 4);
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).unwrap();
                assert_eq!(body, b"true");
            } else if request_index == 23 {
                assert!(chunked, "stream upload must use HTTP/1.1 chunked framing");
                let mut body = Vec::new();
                loop {
                    let mut size = String::new();
                    reader.read_line(&mut size).unwrap();
                    let size = usize::from_str_radix(size.trim(), 16).unwrap();
                    if size == 0 {
                        let mut ending = String::new();
                        reader.read_line(&mut ending).unwrap();
                        break;
                    }
                    let start = body.len();
                    body.resize(start + size, 0);
                    reader.read_exact(&mut body[start..]).unwrap();
                    let mut ending = [0; 2];
                    reader.read_exact(&mut ending).unwrap();
                    assert_eq!(&ending, b"\r\n");
                }
                assert_eq!(body, b"streamed-client-body");
            } else if content_length > 0 {
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).unwrap();
            }
            let stream = reader.get_mut();
            if request_index == 0 {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nSet-Cookie: session=secret; Path=/; HttpOnly\r\nContent-Length: 1\r\n\r\na")
                    .unwrap();
            } else if (1..=20).contains(&request_index) {
                stream
                    .write_all(
                        b"HTTP/1.1 302 Redirect\r\nLocation: /client\r\nContent-Length: 0\r\n\r\n",
                    )
                    .unwrap();
            } else if request_index == 23 {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nb")
                    .unwrap();
            } else if request_index == 24 {
                stream
                    .write_all(
                        b"HTTP/1.1 302 Redirect\r\nLocation: /first\r\nLocation: /second\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    )
                    .unwrap();
            } else {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nb")
                    .unwrap();
            }
            stream.flush().unwrap();
        }
        let mut reader = reader;
        let mut tail = Vec::new();
        reader.read_to_end(&mut tail).unwrap();
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "GET /external HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        reader.into_inner().write_all(
            b"HTTP/1.1 302 Redirect\r\nLocation: http://127.0.0.1:1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ).unwrap();
    });

    let project = workspace("approved_std_http_client");
    let home = workspace("approved_std_http_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.client.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTPClient, HTTPClientOptions, HTTPError, HTTPMethod, HTTPBody, HTTPHeaders, URL }} from std.http;\nimport {{ stdout }} from std.io;\nfn client_with_options(): Result<HTTPClient, HTTPError> {{ return match HTTPClientOptions.create(connect_timeout_ms: 5000, response_headers_timeout_ms: 30000, response_idle_timeout_ms: 30000, max_origins: 2) {{ Ok(options) => {{ stdout.println(options.connect_timeout_ms()); stdout.println(options.response_headers_timeout_ms()); stdout.println(options.response_idle_timeout_ms()); stdout.println(options.max_origins()); HTTPClient.create_with_options(options); }} Error(error) => Error<HTTPClient, HTTPError>(error); }}; }}\nfn main(): Void {{\nmatch URL.parse(\"http://127.0.0.1:{}/client\") {{\nOk(url) => {{ match HTTPClient.create(0) {{ Ok(_) => stdout.println(\"invalid client bounds accepted\"); Error(_) => stdout.println(\"invalid client bounds rejected\"); }} match HTTPClientOptions.create(connect_timeout_ms: 0) {{ Ok(_) => stdout.println(\"invalid connect timeout accepted\"); Error(_) => stdout.println(\"invalid connect timeout rejected\"); }} match HTTPClientOptions.create(response_headers_timeout_ms: 0) {{ Ok(_) => stdout.println(\"invalid headers timeout accepted\"); Error(_) => stdout.println(\"invalid headers timeout rejected\"); }} match HTTPClientOptions.create(response_idle_timeout_ms: 0) {{ Ok(_) => stdout.println(\"invalid idle timeout accepted\"); Error(_) => stdout.println(\"invalid idle timeout rejected\"); }} match HTTPClientOptions.create(connect_timeout_ms: 3600001) {{ Ok(_) => stdout.println(\"oversized timeout accepted\"); Error(_) => stdout.println(\"oversized timeout rejected\"); }} match HTTPClientOptions.create(max_origins: 33) {{ Ok(_) => stdout.println(\"oversized pool accepted\"); Error(_) => stdout.println(\"oversized pool rejected\"); }} match client_with_options() {{\nOk(client) => {{\nmatch client.enable_cookies() {{ Ok(_) => stdout.println(\"cookies enabled\"); Error(_) => stdout.println(\"cookie setup failed\"); }}\nmatch client.clear_cookies() {{ Ok(_) => stdout.println(\"cookies cleared\"); Error(_) => stdout.println(\"cookie clear failed\"); }}\ninmut empty: Array<Byte> = [];\nmatch client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{ Ok(first) => stdout.println(first.status_code()); Error(_) => stdout.println(\"first request failed\"); }}\nmatch client.get_following(url, 20) {{ Ok(redirected) => stdout.println(redirected.status_code()); Error(_) => stdout.println(0); }}\nmatch client.request_body(HTTPMethod.Post, url, HTTPBody.Json(\"not-json\")) {{ Error(error) => match error {{ HTTPError.InvalidJson => stdout.println(\"invalid client JSON rejected\"); _ => stdout.println(\"invalid client JSON accepted\"); }}; Ok(_) => stdout.println(\"invalid client JSON accepted\"); }} match client.request_body(HTTPMethod.Post, url, HTTPBody.Json(\"true\")) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut stream_headers = HTTPHeaders.create(); _ = stream_headers.add(\"content-type\", \"application/query+json\"); match client.request_body(HTTPMethod.Query, url, HTTPBody.Stream(stream_headers, (index: UInt64): Array<Byte> => match index as Int32 {{ 0 => [115 as Byte, 116 as Byte, 114 as Byte, 101 as Byte, 97 as Byte, 109 as Byte, 101 as Byte, 100 as Byte, 45 as Byte]; 1 => [99 as Byte, 108 as Byte, 105 as Byte, 101 as Byte, 110 as Byte, 116 as Byte, 45 as Byte, 98 as Byte, 111 as Byte, 100 as Byte, 121 as Byte]; _ => {{ mut stream_done: Array<Byte> = Array(0); stream_done; }}; }})) {{ Ok(extension) => stdout.println(extension.status_code()); Error(_) => stdout.println(\"stream request failed\"); }}\nmatch URL.resolve(url, \"duplicate-location\") {{ Ok(target) => match client.get_following(target, 1) {{ Ok(_) => stdout.println(\"ambiguous reusable redirect followed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"duplicate reusable URL failed\"); }}\nmatch URL.resolve(url, \"external\") {{ Ok(external) => match client.get_following(external, 1) {{ Ok(_) => stdout.println(\"external client redirect allowed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"external client URL failed\"); }}\nmatch client.close() {{ Ok(_) => stdout.println(\"client closed\"); Error(_) => stdout.println(\"client close failed\"); }} match client.close() {{ Ok(_) => stdout.println(\"client close idempotent\"); Error(_) => stdout.println(\"second client close failed\"); }}\n}}\nError(_) => stdout.println(\"client creation failed\");\n}} }}\nError(_) => stdout.println(\"invalid URL\");\n}}\n}}",
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP client permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved reusable HTTP client program");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "invalid client bounds rejected\ninvalid connect timeout rejected\ninvalid headers timeout rejected\ninvalid idle timeout rejected\noversized timeout rejected\noversized pool rejected\n5000\n30000\n30000\n2\ncookies enabled\ncookies cleared\n200\n200\ninvalid client JSON rejected\n200\n200\nProtocol\nPermissionDenied\nclient closed\nclient close idempotent\n"
    );
    server.join().unwrap();
}

#[test]
fn reusable_client_redirect_policy_preserves_method_and_strips_cross_origin_credentials() {
    use std::io::{BufRead, BufReader, Read, Write};

    let first_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let first_address = first_listener.local_addr().unwrap();
    let second_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let second_address = second_listener.local_addr().unwrap();
    let second_port = second_address.port();
    let first_server = std::thread::spawn(move || {
        let (stream, _) = first_listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "POST /start HTTP/1.1\r\n");
        let mut authorization_seen = false;
        let mut session_credential_seen = false;
        let mut client_token_seen = false;
        let mut appended_client_token_seen = false;
        let mut request_header_seen = false;
        let mut chunked = false;
        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if header.to_ascii_lowercase().starts_with("authorization:") {
                authorization_seen = header.contains("Bearer redirect-secret");
            }
            if header
                .to_ascii_lowercase()
                .starts_with("x-session-credential:")
            {
                session_credential_seen = header.contains("session-secret");
            }
            if header.to_ascii_lowercase().starts_with("x-client-token:") {
                client_token_seen |= header.contains("client-secret");
                appended_client_token_seen |= header.contains("later-client-secret");
            }
            if header.to_ascii_lowercase().starts_with("x-request:") {
                request_header_seen = header.contains("keep");
            }
            if header.to_ascii_lowercase().starts_with("content-length:") {
                content_length = header[15..].trim().parse().unwrap();
            }
            if header
                .to_ascii_lowercase()
                .starts_with("transfer-encoding:")
            {
                chunked = header.to_ascii_lowercase().contains("chunked");
            }
        }
        assert!(authorization_seen);
        assert!(session_credential_seen);
        assert!(client_token_seen);
        assert!(appended_client_token_seen);
        assert!(request_header_seen);
        let mut body = Vec::new();
        if chunked {
            loop {
                let mut size_line = String::new();
                reader.read_line(&mut size_line).unwrap();
                let size = usize::from_str_radix(size_line.trim(), 16).unwrap();
                if size == 0 {
                    let mut terminator = String::new();
                    reader.read_line(&mut terminator).unwrap();
                    break;
                }
                let offset = body.len();
                body.resize(offset + size, 0);
                reader.read_exact(&mut body[offset..]).unwrap();
                let mut terminator = [0; 2];
                reader.read_exact(&mut terminator).unwrap();
                assert_eq!(&terminator, b"\r\n");
            }
        } else {
            body.resize(content_length, 0);
            reader.read_exact(&mut body).unwrap();
        }
        assert_eq!(body, b"payload");
        reader.into_inner().write_all(
            format!(
                "HTTP/1.1 303 See Other\r\nLocation: http://127.0.0.1:{}/final?token=redirect-secret#fragment-secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                second_address.port()
            )
            .as_bytes(),
        ).unwrap();
    });
    let second_server = std::thread::spawn(move || {
        let (stream, _) = second_listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(
            request_line,
            "GET /final?token=redirect-secret HTTP/1.1\r\n"
        );
        let mut authorization_seen = false;
        let mut session_credential_seen = false;
        let mut client_token_seen = false;
        let mut request_header_seen = false;
        let mut content_length = None;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if header.to_ascii_lowercase().starts_with("authorization:") {
                authorization_seen = true;
            }
            if header
                .to_ascii_lowercase()
                .starts_with("x-session-credential:")
            {
                session_credential_seen = true;
            }
            if header.to_ascii_lowercase().starts_with("x-client-token:") {
                client_token_seen = true;
            }
            if header.to_ascii_lowercase().starts_with("x-request:") {
                request_header_seen = header.contains("keep");
            }
            if header.to_ascii_lowercase().starts_with("content-length:") {
                content_length = Some(header[15..].trim().parse::<usize>().unwrap());
            }
        }
        assert!(
            !authorization_seen,
            "credentials must be stripped cross-origin"
        );
        assert!(
            !session_credential_seen,
            "explicitly sensitive custom headers must be stripped cross-origin"
        );
        assert!(
            !client_token_seen,
            "headers set as sensitive must be stripped cross-origin"
        );
        assert!(
            request_header_seen,
            "non-sensitive headers should be preserved"
        );
        assert_eq!(content_length, None, "303 GET must not carry the old body");
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });

    let project = workspace("approved_std_http_generic_redirect");
    let home = workspace("approved_std_http_generic_redirect_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.generic-redirect.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\", \"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            first_address.port(), second_address.port()
        ),
    ).unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPClient, HTTPHeaders, HTTPMethod, HTTPBody, HTTPRedirectPolicy, HTTPRedirectResult, HTTPResponse, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("http://127.0.0.1:{}/start") {{
        Error(_) => stdout.println("url error");
        Ok(url) => match HTTPClient.create() {{
            Error(_) => stdout.println("client error");
            Ok(client) => {{
                mut headers = HTTPHeaders.create();
                _ = headers.add("authorization", "Bearer redirect-secret");
                _ = headers.add_sensitive("X-Session-Credential", "old-session-secret");
                _ = headers.set("X-Session-Credential", "session-secret");
                _ = headers.set_sensitive("X-Client-Token", "client-secret");
                _ = headers.add("X-Client-Token", "later-client-secret");
                _ = headers.add("x-request", "keep");
                match client.request_body_following_with_history(HTTPMethod.Post, url, HTTPBody.Stream(headers, (index: UInt64): Array<Byte> => match index as Int32 {{
                    0 => [112 as Byte, 97 as Byte, 121 as Byte, 108 as Byte, 111 as Byte, 97 as Byte, 100 as Byte];
                    _ => {{ mut done: Array<Byte> = Array(0); done; }};
                }}), HTTPRedirectPolicy.Follow, 2) {{
                    Ok(redirected) => {{ mut response = redirected.response(); stdout.println(response.status_code()); stdout.println(HTTPRedirectResult.format_hop(HTTPMethod.Get, 302, "https://user:secret@example.com/path?token=secret#fragment-secret")); for hop in redirected.history() {{ stdout.println(hop); }} stdout.println(redirected.history_json()); mut fabricated = HTTPRedirectResult.from_client(HTTPResponse.from_text(200, ""), "GET 302 https://example.com/private?credential=factory-secret#fragment\nPOST 999 https://invalid.example/"); stdout.println(fabricated.history_json()); match response.text() {{ Ok(text) => stdout.println(text); Error(_) => stdout.println("invalid body"); }}; }}
                    Error(error) => stdout.println(error.to_string());
                }}
                _ = client.close();
                stdout.println("client closed");
            }}
        }};
    }}
}}
"#,
            first_address.port()
        ),
    ).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start redirect permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved generic redirect fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        format!(
            "200\nGET 302 https://example.com/path?redacted\nPOST 303 http://127.0.0.1:{}/final?redacted\n[\"POST 303 http://127.0.0.1:{}/final?redacted\"]\n[\"GET 302 https://example.com/private?redacted\"]\nok\nclient closed\n",
            second_port, second_port
        )
    );
    first_server.join().unwrap();
    second_server.join().unwrap();
}

#[test]
fn reusable_client_preserves_body_variants_for_method_preserving_redirects() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for status in [301, 302, 307, 308] {
            for body_kind in 0..4 {
                for redirect in [true, false] {
                    let (stream, _) = listener.accept().unwrap();
                    let mut reader = BufReader::new(stream);
                    let mut request_line = String::new();
                    reader.read_line(&mut request_line).unwrap();
                    let path = if redirect { "/start" } else { "/final" };
                    assert_eq!(request_line, format!("POST {path} HTTP/1.1\r\n"));
                    let mut content_type = String::new();
                    let mut content_length = None;
                    loop {
                        let mut header = String::new();
                        reader.read_line(&mut header).unwrap();
                        if header == "\r\n" || header.is_empty() {
                            break;
                        }
                        if header.to_ascii_lowercase().starts_with("content-type:") {
                            content_type = header[13..].trim().to_ascii_lowercase();
                        }
                        if header.to_ascii_lowercase().starts_with("content-length:") {
                            content_length = Some(header[15..].trim().parse::<usize>().unwrap());
                        }
                    }
                    let length = content_length.expect("JSON body must have Content-Length");
                    let mut body = vec![0; length];
                    reader.read_exact(&mut body).unwrap();
                    let (expected_type, expected_body) = match body_kind {
                        0 => ("application/json", b"7".to_vec()),
                        1 => ("application/octet-stream", vec![7]),
                        2 => (
                            "application/x-www-form-urlencoded",
                            b"q=hello+world".to_vec(),
                        ),
                        3 => {
                            let mut expected = b"--zirk-boundary\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"sample.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
                            expected.extend_from_slice(&[0, 255, b'\r', b'\n']);
                            expected.extend_from_slice(b"\r\n--zirk-boundary--\r\n");
                            ("multipart/form-data; boundary=\"zirk-boundary\"", expected)
                        }
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        content_type, expected_type,
                        "body media type must survive {status} replay"
                    );
                    assert_eq!(
                        body, expected_body,
                        "body variant {body_kind} must survive {status} replay"
                    );

                    if redirect {
                        reader
                        .get_mut()
                        .write_all(
                            format!("HTTP/1.1 {status} Redirect\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
                        )
                        .unwrap();
                    } else {
                        reader
                        .get_mut()
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                        )
                        .unwrap();
                    }
                }
            }
        }
    });

    let project = workspace("approved_std_http_body_method_redirects");
    let home = workspace("approved_std_http_body_method_redirects_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.body-method-redirects.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPBody, HTTPClient, HTTPFormData, HTTPMethod, HTTPMultipartBuilder, HTTPRedirectPolicy, URL }} from std.http;
import {{ stdout }} from std.io;
fn redirect_body(kind: Int32): HTTPBody {{
    return match kind {{
        0 => HTTPBody.Json("7");
        1 => HTTPBody.Bytes([7 as Byte]);
        2 => {{ mut form = HTTPFormData.create(); _ = form.add("q", "hello world"); HTTPBody.Form(form); }}
        3 => {{ mut multipart = HTTPMultipartBuilder.create(); _ = multipart.add("upload", [0 as Byte, 255 as Byte, 13 as Byte, 10 as Byte], "sample.bin", "application/octet-stream"); HTTPBody.Multipart(multipart, "zirk-boundary"); }}
        _ => {{ mut empty: Array<Byte> = []; HTTPBody.Bytes(empty); }}
    }};
}}
fn main(): Void {{
    match URL.parse("{origin}/start") {{
        Error(_) => stdout.println("url error");
        Ok(url) => match HTTPClient.create() {{
            Error(_) => stdout.println("client error");
            Ok(client) => {{
                mut status_index: Int32 = 0;
                while status_index < 4 {{
                    mut body_kind: Int32 = 0;
                    while body_kind < 4 {{
                        match client.request_body_following(HTTPMethod.Post, url, redirect_body(body_kind), HTTPRedirectPolicy.Follow, 1) {{
                            Error(error) => stdout.println(error.to_string());
                            Ok(response) => stdout.println(response.status_code());
                        }}
                        body_kind += 1;
                    }}
                    status_index += 1;
                }}
                _ = client.close();
                stdout.println("client closed");
            }}
        }};
    }}
}}
"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start JSON redirect permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved JSON redirect fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        format!("{}client closed\n", "200\n".repeat(16))
    );
    server.join().unwrap();
}

#[test]
fn reusable_client_307_restarts_deterministic_stream_from_zero() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for redirect in [true, false] {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let path = if redirect { "/start" } else { "/final" };
            assert_eq!(request_line, format!("POST {path} HTTP/1.1\r\n"));
            let mut chunked = false;
            let mut sensitive_header_seen = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                if header
                    .to_ascii_lowercase()
                    .starts_with("transfer-encoding:")
                {
                    chunked = header.to_ascii_lowercase().contains("chunked");
                }
                if header
                    .to_ascii_lowercase()
                    .starts_with("x-session-credential:")
                {
                    sensitive_header_seen = header.contains("same-origin-secret");
                }
            }
            assert!(chunked, "HTTPBody.Stream must use chunked framing");
            assert!(
                sensitive_header_seen,
                "same-origin redirects must retain explicitly sensitive headers"
            );
            let mut body = Vec::new();
            loop {
                let mut size_line = String::new();
                reader.read_line(&mut size_line).unwrap();
                let size = usize::from_str_radix(size_line.trim(), 16).unwrap();
                if size == 0 {
                    let mut trailer_end = String::new();
                    reader.read_line(&mut trailer_end).unwrap();
                    assert_eq!(trailer_end, "\r\n");
                    break;
                }
                let offset = body.len();
                body.resize(offset + size, 0);
                reader.read_exact(&mut body[offset..]).unwrap();
                let mut chunk_end = [0; 2];
                reader.read_exact(&mut chunk_end).unwrap();
                assert_eq!(&chunk_end, b"\r\n");
            }
            assert_eq!(body, b"replay", "producer must restart at chunk zero");
            if redirect {
                reader
                    .get_mut()
                    .write_all(b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            } else {
                reader
                    .get_mut()
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .unwrap();
            }
        }
    });

    let project = workspace("approved_std_http_stream_307_redirect");
    let home = workspace("approved_std_http_stream_307_redirect_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.stream-307.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPBody, HTTPClient, HTTPHeaders, HTTPMethod, HTTPRedirectPolicy, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("{origin}/start") {{
        Error(_) => stdout.println("url error");
        Ok(url) => match HTTPClient.create() {{
            Error(_) => stdout.println("client error");
            Ok(client) => {{
                mut headers = HTTPHeaders.create();
                _ = headers.add("content-type", "application/octet-stream");
                _ = headers.add_sensitive("X-Session-Credential", "same-origin-secret");
                match client.request_body_following(HTTPMethod.Post, url, HTTPBody.Stream(headers, (index: UInt64): Array<Byte> => match index as Int32 {{
                    0 => [114 as Byte, 101 as Byte];
                    1 => [112 as Byte, 108 as Byte, 97 as Byte, 121 as Byte];
                    _ => {{ mut done: Array<Byte> = Array(0); done; }};
                }}), HTTPRedirectPolicy.Follow, 1) {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(response) => stdout.println(response.status_code());
                }}
                _ = client.close();
                stdout.println("client closed");
            }}
        }};
    }}
}}
"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start stream redirect permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved streamed redirect fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nclient closed\n"
    );
    server.join().unwrap();
}

#[test]
fn reusable_client_303_preserves_head_and_discards_request_body() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "HEAD /start HTTP/1.1\r\n");
        let mut body_length = None;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if header.to_ascii_lowercase().starts_with("content-length:") {
                body_length = Some(header[15..].trim().parse::<usize>().unwrap());
            }
        }
        let mut body = vec![0; body_length.expect("initial HEAD JSON body has a length")];
        reader.read_exact(&mut body).unwrap();
        assert_eq!(body, b"7");
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 303 See Other\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();

        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "HEAD /final HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            assert!(
                !header.to_ascii_lowercase().starts_with("content-length:")
                    && !header
                        .to_ascii_lowercase()
                        .starts_with("transfer-encoding:"),
                "303 HEAD follow-up must not include the original request body"
            );
        }
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let project = workspace("approved_std_http_head_303_redirect");
    let home = workspace("approved_std_http_head_303_redirect_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.head-303.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPBody, HTTPClient, HTTPMethod, HTTPRedirectPolicy, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("{origin}/start") {{
        Error(_) => stdout.println("url error");
        Ok(url) => match HTTPClient.create() {{
            Error(_) => stdout.println("client error");
            Ok(client) => {{
                match client.request_body_following(HTTPMethod.Head, url, HTTPBody.Json("7"), HTTPRedirectPolicy.Follow, 1) {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(response) => stdout.println(response.status_code());
                }}
                _ = client.close();
                stdout.println("client closed");
            }}
        }};
    }}
}}
"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start HEAD redirect permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HEAD redirect fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nclient closed\n"
    );
    server.join().unwrap();
}

#[test]
fn reusable_client_rejects_ambiguous_redirect_location_without_following() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "POST /start HTTP/1.1\r\n");
        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if header.to_ascii_lowercase().starts_with("content-length:") {
                content_length = header[15..].trim().parse().unwrap();
            }
        }
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        assert_eq!(body, b"x");
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: /first\r\nLocation: /second\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
        drop(reader.into_inner());
        std::thread::sleep(std::time::Duration::from_millis(100));
        listener.set_nonblocking(true).unwrap();
        assert!(
            matches!(
                listener.accept(),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
            ),
            "ambiguous Location must not trigger a follow-up request"
        );
    });

    let project = workspace("approved_std_http_duplicate_location");
    let home = workspace("approved_std_http_duplicate_location_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.duplicate-location.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPBody, HTTPClient, HTTPMethod, HTTPRedirectPolicy, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("{origin}/start") {{
        Error(_) => stdout.println("url error");
        Ok(url) => match HTTPClient.create() {{
            Error(_) => stdout.println("client error");
            Ok(client) => {{
                match client.request_body_following(HTTPMethod.Post, url, HTTPBody.Text("x"), HTTPRedirectPolicy.Follow, 1) {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(response) => stdout.println(response.status_code());
                }}
                _ = client.close();
                stdout.println("client closed");
            }}
        }};
    }}
}}
"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start duplicate Location permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run duplicate Location redirect fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "Protocol\nclient closed\n"
    );
    server.join().unwrap();
}

#[test]
fn reusable_client_same_origin_policy_returns_cross_origin_redirect() {
    use std::io::{BufRead, BufReader, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let target_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target_address = target_listener.local_addr().unwrap();
    target_listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "GET /start HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        reader
            .get_mut()
            .write_all(
                format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/blocked\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    target_address.port()
                )
                .as_bytes(),
            )
            .unwrap();
        drop(reader.into_inner());
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            matches!(
                target_listener.accept(),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
            ),
            "SameOrigin must not connect to a different port"
        );
    });

    let project = workspace("approved_std_http_same_origin_redirect");
    let home = workspace("approved_std_http_same_origin_redirect_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.same-origin.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPBody, HTTPClient, HTTPMethod, HTTPRedirectPolicy, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("{origin}/start") {{
        Error(_) => stdout.println("url error");
        Ok(url) => match HTTPClient.create() {{
            Error(_) => stdout.println("client error");
            Ok(client) => {{
                mut empty: Array<Byte> = [];
                match client.request_body_following(HTTPMethod.Get, url, HTTPBody.Bytes(empty), HTTPRedirectPolicy.SameOrigin, 1) {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(response) => stdout.println(response.status_code());
                }}
                _ = client.close();
                stdout.println("client closed");
            }}
        }};
    }}
}}
"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start SameOrigin redirect permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved SameOrigin redirect fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "302\nclient closed\n"
    );
    server.join().unwrap();
}

#[test]
fn http_client_options_enforce_response_header_timeout() {
    use std::io::{BufRead, BufReader, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "GET /slow-headers HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        let _ = reader
            .get_mut()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
    });

    let project = workspace("http_client_header_timeout");
    let home = workspace("http_client_header_timeout_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("http://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.client.header.timeout.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTPClient, HTTPClientOptions, HTTPMethod, HTTPHeaders, URL }} from std.http;\nimport {{ stdout }} from std.io;\nfn main(): Void {{\nmatch URL.parse(\"{origin}/slow-headers\") {{\nError(_) => stdout.println(\"URL error\");\nOk(url) => match HTTPClientOptions.create(connect_timeout_ms: 1000, response_headers_timeout_ms: 25, response_idle_timeout_ms: 1000) {{\nError(_) => stdout.println(\"options error\");\nOk(options) => match HTTPClient.create_with_options(options) {{\nError(_) => stdout.println(\"client error\");\nOk(client) => {{ mut empty: Array<Byte> = []; match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{ Error(error) => stdout.println(error.to_string()); Ok(_) => stdout.println(\"late response accepted\"); }} _ = client.close(); stdout.println(\"closed\"); }}\n}}\n}}\n}}\n}}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP client permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HTTP timeout fixture");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "Timeout\nclosed\n"
    );
    server.join().unwrap();
}

#[test]
fn http_client_options_enforce_tls_connect_timeout() {
    use std::io::Read;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut client_hello = [0; 5];
        stream.read_exact(&mut client_hello).unwrap();
        assert_eq!(client_hello[0], 22, "client must start a TLS handshake");
        std::thread::sleep(std::time::Duration::from_millis(300));
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .unwrap();
        let mut buffer = [0; 64];
        let mut received = 0usize;
        loop {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            received += count;
            assert!(
                received <= 256,
                "timed-out TLS socket sent excessive trailing data"
            );
        }
    });

    let project = workspace("http_client_tls_connect_timeout");
    let home = workspace("http_client_tls_connect_timeout_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let origin = format!("https://127.0.0.1:{}", address.port());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.client.tls.connect.timeout.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTPClient, HTTPClientOptions, HTTPMethod, HTTPHeaders, URL }} from std.http;\nimport {{ stdout }} from std.io;\nfn main(): Void {{\nmatch URL.parse(\"{origin}/stalled-tls\") {{\nError(_) => stdout.println(\"URL error\");\nOk(url) => match HTTPClientOptions.create(connect_timeout_ms: 100, response_headers_timeout_ms: 1000, response_idle_timeout_ms: 1000) {{\nError(_) => stdout.println(\"options error\");\nOk(options) => match HTTPClient.create_with_options(options) {{\nError(_) => stdout.println(\"client error\");\nOk(client) => {{ mut empty: Array<Byte> = []; match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{ Error(error) => stdout.println(error.to_string()); Ok(_) => stdout.println(\"stalled TLS response accepted\"); }} _ = client.close(); stdout.println(\"closed\"); }}\n}}\n}}\n}}\n}}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTPS permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HTTPS timeout fixture");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "Timeout\nclosed\n"
    );
    server.join().unwrap();
}

#[test]
fn shared_http_client_serializes_requests_across_concurrent_tasks() {
    use std::io::{BufRead, BufReader, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        for expected_path in ["/one", "/two"] {
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert_eq!(request_line, format!("GET {expected_path} HTTP/1.1\r\n"));
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
            }
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
            reader.get_mut().flush().unwrap();
        }
    });

    let project = workspace("approved_shared_http_client");
    let home = workspace("approved_shared_http_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.shared.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPClient, HTTPHeaders, HTTPMethod, HTTPResponse, HTTPError, URL }} from std.http;
import {{ stdout }} from std.io;
fn send(client: HTTPClient, url: URL): Result<HTTPResponse, HTTPError> {{
    inmut empty: Array<Byte> = [];
    return client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create());
}}
fn main(): Void {{
    match HTTPClient.create(2) {{
        Error(_) => stdout.println("client creation failed");
        Ok(client) => {{
            match URL.parse("http://127.0.0.1:{}/one") {{
                Error(_) => stdout.println("first URL failed");
                Ok(first_url) => match URL.parse("http://127.0.0.1:{}/two") {{
                    Error(_) => stdout.println("second URL failed");
                    Ok(second_url) => {{
                        concurrent {{
                            inmut first = send(client, first_url);
                            inmut second = send(client, second_url);
                        }}
                        match first {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}
                        match second {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}
                    }}
                }}
            }}
            _ = client.close();
        }}
    }}
}}"#,
            address.port(),
            address.port(),
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("approve shared HTTP client access");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run shared HTTP client program");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\n200\n",
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    server.join().unwrap();
}

#[test]
fn cancelled_shared_http_client_close_can_be_retried() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (first, _) = listener.accept().unwrap();
        let mut first = BufReader::new(first);
        let mut line = String::new();
        first.read_line(&mut line).unwrap();
        assert_eq!(line, "GET /blocked HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            first.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        let mut discarded = Vec::new();
        let _ = first.read_to_end(&mut discarded);

        let (second, _) = listener.accept().unwrap();
        let mut second = BufReader::new(second);
        line.clear();
        second.read_line(&mut line).unwrap();
        assert_eq!(line, "GET /retry HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            second.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        second
            .get_mut()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        second.get_mut().flush().unwrap();
        let mut tail = Vec::new();
        let _ = second.read_to_end(&mut tail);
    });

    let project = workspace("approved_cancelled_http_client_close");
    let home = workspace("approved_cancelled_http_client_close_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.cancel-close.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPClient, HTTPHeaders, HTTPMethod, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match HTTPClient.create(2) {{
        Error(_) => stdout.println("client creation failed");
        Ok(client) => {{ inmut::strict shared = client; match URL.parse("http://127.0.0.1:{}/blocked") {{
            Error(_) => stdout.println("URL parsing failed");
            Ok(blocked_url) => match URL.parse("http://127.0.0.1:{}/retry") {{
                Error(_) => stdout.println("retry URL parsing failed");
                Ok(retry_url) => {{
                    inmut::strict blocked = blocked_url;
                    inmut::strict retry = retry_url;
                    inmut::strict empty: Array<Byte> = [];
                    mut winner: Int32 = Concurrent.of(
                        (): Int32 => {{ _ = shared.request(HTTPMethod.Get, blocked, empty, HTTPHeaders.create()); return 1; }},
                        (): Int32 => {{ Timer.sleep(10ms); _ = shared.close(); return 2; }},
                        (): Int32 => {{ Timer.sleep(50ms); return 3; }},
                    ).first();
                    match shared.request(HTTPMethod.Get, retry, empty, HTTPHeaders.create()) {{
                        Ok(response) => stdout.println(response.status_code());
                        Error(error) => stdout.println(error.to_string());
                    }}
                    match shared.close() {{ Ok(_) => stdout.println("client closed"); Error(_) => stdout.println("close failed"); }}
                }}
            }}
        }} }}
    }}
}}"#,
            address.port(),
            address.port(),
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("approve HTTP client close cancellation fixture");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run HTTP client close cancellation fixture");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nclient closed\n"
    );
    server.join().unwrap();
}

#[test]
fn reusable_http_client_retries_safe_requests_and_rejects_unsafe_retry_policy() {
    use std::io::{BufRead, BufReader, Read, Write};

    fn read_request(reader: &mut BufReader<std::net::TcpStream>) -> (String, Vec<u8>, bool) {
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let mut content_length = 0;
        let mut chunked = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if let Some(value) = header
                .strip_prefix("content-length:")
                .or_else(|| header.strip_prefix("Content-Length:"))
            {
                content_length = value.trim().parse().unwrap();
            }
            if header
                .to_ascii_lowercase()
                .starts_with("transfer-encoding: chunked")
            {
                chunked = true;
            }
        }
        let body = if chunked {
            let mut body = Vec::new();
            loop {
                let mut size = String::new();
                reader.read_line(&mut size).unwrap();
                let size = usize::from_str_radix(size.trim(), 16).unwrap();
                if size == 0 {
                    loop {
                        let mut trailer = String::new();
                        reader.read_line(&mut trailer).unwrap();
                        if trailer == "\r\n" || trailer.is_empty() {
                            break;
                        }
                    }
                    break;
                }
                let start = body.len();
                body.resize(start + size, 0);
                reader.read_exact(&mut body[start..]).unwrap();
                let mut ending = [0; 2];
                reader.read_exact(&mut ending).unwrap();
                assert_eq!(&ending, b"\r\n");
            }
            body
        } else {
            let mut body = vec![0; content_length];
            reader.read_exact(&mut body).unwrap();
            body
        };
        (request_line, body, chunked)
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for attempt in 0..6 {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let (request_line, body, chunked) = read_request(&mut reader);
            match attempt {
                0 | 1 => {
                    assert_eq!(request_line, "GET /retry HTTP/1.1\r\n");
                    assert!(body.is_empty());
                    if attempt == 0 {
                        drop(reader);
                        continue;
                    }
                }
                2..=5 => {
                    assert_eq!(request_line, "QUERY /retry HTTP/1.1\r\n");
                    assert_eq!(body, b"{\"q\":1}");
                    assert_eq!(chunked, attempt >= 4);
                    if attempt == 2 || attempt == 4 {
                        drop(reader);
                        continue;
                    }
                }
                _ => unreachable!(),
            }
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            reader.get_mut().flush().unwrap();
            let mut tail = Vec::new();
            let _ = reader.read_to_end(&mut tail);
        }
    });

    let project = workspace("approved_std_http_retry");
    let home = workspace("approved_std_http_retry_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.retry.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPClient, HTTPHeaders, HTTPMethod, HTTPBody, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match HTTPClient.create(2) {{
        Error(_) => stdout.println("client creation failed");
        Ok(client) => {{
            mut empty: Array<Byte> = [];
            match URL.parse("http://127.0.0.1:{}/retry") {{
                Error(_) => stdout.println("URL parse failed");
                Ok(url) => {{
                    match client.request_with_retries(HTTPMethod.Get, url, empty, HTTPHeaders.create(), 1) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(response) => stdout.println(response.status_code());
                    }}
                    mut query_body: Array<Byte> = [123 as Byte, 34 as Byte, 113 as Byte, 34 as Byte, 58 as Byte, 49 as Byte, 125 as Byte];
                    mut query_headers = HTTPHeaders.create();
                    _ = query_headers.add("content-type", "application/query+json");
                    match client.request_with_retries(HTTPMethod.Query, url, query_body, query_headers, 1) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(response) => stdout.println(response.status_code());
                    }}
                    mut stream_headers = HTTPHeaders.create();
                    _ = stream_headers.add("content-type", "application/query+json");
                    match client.request_body_with_retries(HTTPMethod.Query, url, HTTPBody.Stream(stream_headers, (index: UInt64): Array<Byte> => match index as Int32 {{
                        0 => [123 as Byte, 34 as Byte, 113 as Byte];
                        1 => [34 as Byte, 58 as Byte, 49 as Byte, 125 as Byte];
                        _ => {{ mut done: Array<Byte> = Array(0); done; }};
                    }}), 1) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(response) => stdout.println(response.status_code());
                    }}
                    match client.request_with_retries(HTTPMethod.Post, url, empty, HTTPHeaders.create(), 1) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(_) => stdout.println("unsafe request sent");
                    }}
                }}
            }}
            _ = client.close();
        }}
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit retry permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved retry client program");
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\n200\n200\nProtocol\n"
    );
    server.join().unwrap();
}

#[test]
fn bundled_std_http_client_reuses_authenticated_https_connection() {
    use std::sync::Arc;

    use std::io::{BufRead, BufReader, Write};
    use std::sync::atomic::{AtomicUsize, Ordering};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let server_accepted = Arc::clone(&accepted);
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let server_config = Arc::new(
        tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                    certificate.to_vec(),
                )],
                tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(private_key.to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let server = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        server_accepted.fetch_add(1, Ordering::SeqCst);
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let tls = tokio_rustls::rustls::StreamOwned::new(
            tokio_rustls::rustls::ServerConnection::new(server_config).unwrap(),
            socket,
        );
        let mut reader = BufReader::new(tls);
        for _ in 0..2 {
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert_eq!(request_line, "GET /reuse HTTP/1.1\r\n");
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
            }
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
            reader.get_mut().flush().unwrap();
        }
    });

    let origin = format!("https://127.0.0.1:{port}");
    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let root_literal = root
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"import {{ HTTPClient, HTTPHeaders, HTTPMethod, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [[{root_literal}]];
    match HTTPClient.create_with_trust_roots(roots, 2) {{
        Error(_) => stdout.println("client creation failed");
        Ok(client) => {{
            mut empty: Array<Byte> = [];
            match URL.parse("{origin}/reuse") {{
                Error(_) => stdout.println("URL parse failed");
                Ok(url) => {{
                    match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(first) => stdout.println(first.status_code());
                    }}
                    match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{
                        Error(error) => stdout.println(error.to_string());
                        Ok(second) => stdout.println(second.status_code());
                    }}
                }}
            }}
            _ = client.close();
            stdout.println("client closed");
        }}
    }}
}}"#
    );

    let project = workspace("approved_std_http_https_client");
    let home = workspace("approved_std_http_https_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.https.client.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTPS client approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HTTPS pooled client");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\n200\nclient closed\n"
    );
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
    server.join().unwrap();
}

#[test]
fn bundled_std_http_client_reuses_authenticated_http2_connection() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let requests = Arc::new(AtomicUsize::new(0));
    let overlapped = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let server_accepted = Arc::clone(&accepted);
    let server_requests = Arc::clone(&requests);
    let server_overlapped = Arc::clone(&overlapped);
    let expected_authority = format!("127.0.0.1:{port}");
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let mut server_config = tokio_rustls::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                certificate.to_vec(),
            )],
            tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(private_key.to_vec()).unwrap(),
        )
        .unwrap();
    server_config.alpn_protocols = vec![b"h2".to_vec()];
    let server = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let (socket, _) = listener.accept().await.unwrap();
                server_accepted.fetch_add(1, Ordering::SeqCst);
                let tls = tokio_rustls::TlsAcceptor::from(Arc::new(server_config))
                    .accept(socket)
                    .await
                    .unwrap();
                assert_eq!(tls.get_ref().1.alpn_protocol(), Some(b"h2".as_slice()));
                let arrivals = tokio::sync::watch::channel(0usize).0;
                let first_arrival = Arc::new(std::sync::Mutex::new(None));
                let service = hyper::service::service_fn(
                    move |request: hyper::Request<hyper::body::Incoming>| {
                        let requests = Arc::clone(&server_requests);
                        let arrivals = arrivals.clone();
                        let first_arrival = Arc::clone(&first_arrival);
                        let overlapped = Arc::clone(&server_overlapped);
                        let expected_authority = expected_authority.clone();
                        async move {
                            assert_eq!(request.version(), hyper::Version::HTTP_2);
                            assert_eq!(
                                request
                                    .headers()
                                    .get(hyper::header::HOST)
                                    .and_then(|value| value.to_str().ok()),
                                Some(expected_authority.as_str()),
                                "HTTP/2 :authority must match the request host"
                            );
                            assert_eq!(request.uri().path(), "/reuse");
                            assert_eq!(request.method(), "QUERY");
                            assert_eq!(
                                request
                                    .headers()
                                    .get("user-agent")
                                    .unwrap()
                                    .to_str()
                                    .unwrap(),
                                format!("Zirk-http-client/{}", env!("CARGO_PKG_VERSION")).as_str()
                            );
                            assert_eq!(
                                request.headers().get("content-type").unwrap(),
                                "application/query+json"
                            );
                            requests.fetch_add(1, Ordering::SeqCst);
                            let body = http_body_util::BodyExt::collect(request.into_body())
                                .await
                                .unwrap()
                                .to_bytes();
                            assert_eq!(body.as_ref(), b"h2-stream-body");
                            let arrived_at = std::time::Instant::now();
                            let first = {
                                let mut first = first_arrival.lock().unwrap();
                                *first.get_or_insert(arrived_at)
                            };
                            let mut receiver = arrivals.subscribe();
                            arrivals.send_modify(|count| *count += 1);
                            let both_arrived =
                                tokio::time::timeout(std::time::Duration::from_secs(2), async {
                                    loop {
                                        if *receiver.borrow_and_update() >= 2 {
                                            break;
                                        }
                                        if receiver.changed().await.is_err() {
                                            break;
                                        }
                                    }
                                })
                                .await
                                .is_ok();
                            if both_arrived
                                && arrived_at.duration_since(first)
                                    < std::time::Duration::from_secs(1)
                            {
                                overlapped.store(true, Ordering::SeqCst);
                            }
                            Ok::<_, std::convert::Infallible>(hyper::Response::new(
                                http_body_util::Full::new(bytes::Bytes::from_static(b"ok")),
                            ))
                        }
                    },
                );
                let _ =
                    hyper::server::conn::http2::Builder::new(hyper_util::rt::TokioExecutor::new())
                        .serve_connection(hyper_util::rt::TokioIo::new(tls), service)
                        .await;
            });
    });

    let origin = format!("https://127.0.0.1:{port}");
    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let root_literal = root
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"import {{ HTTPClient, HTTPClientOptions, HTTPProtocolPreference, HTTPHeaders, HTTPMethod, HTTPBody, HTTPResponse, HTTPError, URL }} from std.http;
import {{ stdout }} from std.io;
fn send(client: HTTPClient, url: URL): Result<HTTPResponse, HTTPError> {{
    mut headers = HTTPHeaders.create();
    _ = headers.add("content-type", "application/query+json");
    return client.request_body(HTTPMethod.Query, url, HTTPBody.Stream(headers, (index: UInt64): Array<Byte> => match index as Int32 {{
        0 => [104 as Byte, 50 as Byte, 45 as Byte, 115 as Byte, 116 as Byte, 114 as Byte, 101 as Byte, 97 as Byte, 109 as Byte];
        1 => [45 as Byte, 98 as Byte, 111 as Byte, 100 as Byte, 121 as Byte];
        _ => {{ mut done: Array<Byte> = Array(0); done; }};
    }}));
}}
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [[{root_literal}]];
    match HTTPClientOptions.create_with_protocol(HTTPProtocolPreference.HTTP2, max_origins: 2) {{
        Error(_) => stdout.println("options creation failed");
        Ok(options) => match HTTPClient.create_with_options_and_trust_roots(options, roots) {{
        Error(_) => stdout.println("client creation failed");
        Ok(client) => {{
            match URL.parse("{origin}/reuse") {{
                Error(_) => stdout.println("URL parse failed");
                Ok(url) => {{
                    concurrent {{
                        inmut first = send(client, url);
                        inmut second = send(client, url);
                    }}
                    match first {{ Error(error) => stdout.println(error.to_string()); Ok(response) => {{ stdout.println(response.status_code()); stdout.println(response.protocol().to_string()); }} }}
                    match second {{ Error(error) => stdout.println(error.to_string()); Ok(response) => {{ stdout.println(response.status_code()); stdout.println(response.protocol().to_string()); }} }}
                }}
            }}
            _ = client.close();
            stdout.println("client closed");
        }}
        }}
    }}
}}"#
    );
    let project = workspace("approved_std_http_https_h2_client");
    let home = workspace("approved_std_http_https_h2_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.https.h2.client.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP/2 client permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HTTP/2 pooled client");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nHTTP2\n200\nHTTP2\nclient closed\n"
    );
    server.join().unwrap();
    assert_eq!(accepted.load(Ordering::SeqCst), 1);
    assert_eq!(requests.load(Ordering::SeqCst), 2);
    assert!(
        overlapped.load(Ordering::SeqCst),
        "HTTP/2 upload streams were serialized"
    );
}

#[test]
fn bundled_std_http_explicit_cache_reuses_get_and_revalidates_query() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::time::{Duration, Instant};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for (expected_method, expected_body, revalidate, expected_language) in [
            ("GET", &b""[..], false, None),
            ("QUERY", &b"{}"[..], false, Some("en")),
            ("QUERY", &b"{}"[..], false, Some("fr")),
            ("QUERY", &b"{\"x\":1}"[..], false, None),
            ("QUERY", &b"{}"[..], true, Some("en")),
            ("POST", &b"write"[..], false, None),
            ("GET", &b""[..], false, None),
            ("POST_STREAM", &b"stream"[..], false, None),
            ("GET", &b""[..], false, None),
        ] {
            // Permission approval and compiler startup can take several
            // seconds under emulation; the socket is bound before either runs.
            let deadline = Instant::now() + Duration::from_secs(30);
            let (stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "timed out waiting for {expected_method}"
                        );
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("HTTP fixture accept failed: {error}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let wire_method = if expected_method == "POST_STREAM" {
                "POST"
            } else {
                expected_method
            };
            assert_eq!(line, format!("{wire_method} / HTTP/1.1\r\n"));
            let mut length = 0;
            let mut content_type = false;
            let mut content_language = None;
            let mut if_none_match = false;
            let mut chunked = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                let lower = header.to_ascii_lowercase();
                if let Some(value) = lower.strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                chunked |= lower.starts_with("transfer-encoding: chunked");
                content_type |= lower == "content-type: application/query+json\r\n";
                if let Some(value) = lower.strip_prefix("content-language:") {
                    content_language = Some(value.trim().to_owned());
                }
                if_none_match |= lower == "if-none-match: \"query-v1\"\r\n";
            }
            assert_eq!(content_language.as_deref(), expected_language);
            if expected_method == "POST_STREAM" {
                assert!(chunked);
            } else {
                assert_eq!(length, expected_body.len());
            }
            if expected_method == "QUERY" {
                assert!(content_type);
            }
            assert_eq!(if_none_match, revalidate);
            let body = if expected_method == "POST_STREAM" {
                let mut body = Vec::new();
                loop {
                    let mut size = String::new();
                    reader.read_line(&mut size).unwrap();
                    let size = usize::from_str_radix(size.trim(), 16).unwrap();
                    if size == 0 {
                        let mut trailer = String::new();
                        reader.read_line(&mut trailer).unwrap();
                        assert_eq!(trailer, "\r\n");
                        break;
                    }
                    let start = body.len();
                    body.resize(start + size, 0);
                    reader.read_exact(&mut body[start..]).unwrap();
                    let mut crlf = [0; 2];
                    reader.read_exact(&mut crlf).unwrap();
                    assert_eq!(&crlf, b"\r\n");
                }
                body
            } else {
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                body
            };
            assert_eq!(body, expected_body);
            if revalidate {
                reader.into_inner().write_all(
                    b"HTTP/1.1 304 Not Modified\r\nCache-Control: max-age=60\r\nETag: \"query-v1\"\r\nConnection: close\r\n\r\n",
                ).unwrap();
            } else if expected_method == "GET" {
                reader.get_mut().write_all(
                    b"HTTP/1.1 200 OK\r\nDate: Thu, 01 Jan 2099 00:00:00 GMT\r\nExpires: Thu, 01 Jan 2099 00:01:00 GMT\r\nETag: \"get-v1\"\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                ).unwrap();
            } else {
                reader.into_inner().write_all(
                    b"HTTP/1.1 200 OK\r\nCache-Control: max-age=60\r\nETag: \"query-v1\"\r\nVary: Content-Type\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                ).unwrap();
            }
        }
    });

    let project = workspace("approved_std_http_get_cached");
    let home = workspace("approved_std_http_get_cached_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.cache.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    ).unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTP, HTTPHeaders, HTTPMethod, URL }} from std.http;\nimport {{ stdout }} from std.io;\nfn main(): Void {{\nmatch URL.parse(\"http://127.0.0.1:{}/\") {{\nOk(url) => {{\nmatch HTTP.get_cached(url) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmatch HTTP.get_cached(url) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut headers = HTTPHeaders.create(); _ = headers.add(\"content-type\", \"application/query+json\"); _ = headers.add(\"content-language\", \"en\");\nmut payload: Array<Byte> = [123 as Byte, 125 as Byte];\nmatch HTTP.request_cached(HTTPMethod.Query, url, payload, headers) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut french_headers = HTTPHeaders.create(); _ = french_headers.add(\"content-type\", \"application/query+json\"); _ = french_headers.add(\"content-language\", \"fr\");\nmatch HTTP.request_cached(HTTPMethod.Query, url, payload, french_headers) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut other_query: Array<Byte> = [123 as Byte, 34 as Byte, 120 as Byte, 34 as Byte, 58 as Byte, 49 as Byte, 125 as Byte];\nmut other_query_headers = HTTPHeaders.create(); _ = other_query_headers.add(\"content-type\", \"application/query+json\");\nmatch HTTP.request_cached(HTTPMethod.Query, url, other_query, other_query_headers) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut validate_headers = HTTPHeaders.create(); _ = validate_headers.add(\"content-type\", \"application/query+json\"); _ = validate_headers.add(\"content-language\", \"en\"); _ = validate_headers.add(\"cache-control\", \"no-cache\");\nmatch HTTP.request_cached(HTTPMethod.Query, url, payload, validate_headers) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\n\nmut write_bytes: Array<Byte> = [119 as Byte, 114 as Byte, 105 as Byte, 116 as Byte, 101 as Byte];\nmatch HTTP.request(HTTPMethod.Post, url, write_bytes, HTTPHeaders.create()) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmatch HTTP.get_cached(url) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut upload_headers = HTTPHeaders.create(); _ = upload_headers.add(\"content-type\", \"application/octet-stream\");\nmatch HTTP.post_stream(url, upload_headers, (index: UInt64): Array<Byte> => match index as Int32 {{ 0 => [115 as Byte, 116 as Byte, 114 as Byte, 101 as Byte, 97 as Byte, 109 as Byte]; _ => {{ mut empty: Array<Byte> = Array(0); empty; }}; }}) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmatch HTTP.get_cached(url) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\nmut offline_body: Array<Byte> = [];\nmut offline_headers = HTTPHeaders.create(); _ = offline_headers.add(\"cache-control\", \"only-if-cached\");\nmatch URL.resolve(url, \"offline-only\") {{ Ok(offline_url) => match HTTP.request_cached(HTTPMethod.Get, offline_url, offline_body, offline_headers) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"offline URL failed\"); }}\n}}\nError(_) => stdout.println(\"invalid URL\");\n}}\n}}",
            address.port()
        ),
    ).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run explicitly cached HTTP client");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\n200\n200\n200\n200\n200\n200\n200\n200\n200\n504\n",
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    server.join().unwrap();
}

#[test]
fn bundled_std_http_get_fails_closed_without_a_network_approval() {
    let project = workspace("unapproved_std_http_get");
    let home = workspace("unapproved_std_http_get_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.http.denied.fixture\"\nkind application\nentry \"src/main.zrk\"\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        "import { HTTP, HTTPServer, URL } from std.http;\n\
         import { stdout } from std.io;\n\
         fn main(): Void {\n\
             match URL.parse(\"ftp://127.0.0.1/\") {\n\
                 Ok(_) => stdout.println(\"unexpected URL success\");\n\
                 Error(_) => stdout.println(\"invalid URL\");\n\
             }\n\
             match URL.parse(\"http://127.0.0.1:1/\") {\n\
                 Ok(url) => match HTTP.get(url) {\n\
                     Ok(_) => stdout.println(\"unexpected success\");\n\
                     Error(_) => stdout.println(\"denied\");\n\
                 }\n\
                 Error(_) => stdout.println(\"invalid URL\");\n\
             }\n\
             match HTTPServer.bind(\"tcp://127.0.0.1:1\") {\n\
                 Ok(_) => stdout.println(\"unexpected listen success\");\n\
                 Error(error) => stdout.println(error.to_string());\n\
             }\n\
         }",
    )
    .unwrap();

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run unapproved std.http GET program");
    assert!(
        run.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "invalid URL\ndenied\nPermissionDenied\n"
    );
}

#[test]
fn bundled_std_http_query_and_text_post_use_exact_grant_and_media_contracts() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for (method, target, content_type, expected_body, status) in [
            ("GET", "/redirect", "-", &b""[..], 302),
            ("GET", "/results", "-", &b""[..], 200),
            ("GET", "/external", "-", &b""[..], 302),
            (
                "QUERY",
                "/query-external",
                "application/json",
                &b"true"[..],
                302,
            ),
            ("GET", "/multiple-location", "-", &b""[..], 302),
            (
                "QUERY",
                "/query-multiple-location",
                "application/json",
                &b"true"[..],
                302,
            ),
            ("GET", "/missing-location", "-", &b""[..], 302),
            (
                "QUERY",
                "/query-missing-location",
                "application/json",
                &b"true"[..],
                302,
            ),
            ("GET", "/loop", "-", &b""[..], 302),
            ("GET", "/loop", "-", &b""[..], 302),
            ("QUERY", "/indirect", "application/json", &b"true"[..], 303),
            ("GET", "/results", "-", &b""[..], 200),
            (
                "QUERY",
                "/preserve-301",
                "application/json",
                &b"true"[..],
                301,
            ),
            ("QUERY", "/preserved", "application/json", &b"true"[..], 200),
            (
                "QUERY",
                "/preserve-302",
                "application/json",
                &b"true"[..],
                302,
            ),
            ("QUERY", "/preserved", "application/json", &b"true"[..], 200),
            ("QUERY", "/preserve", "application/json", &b"true"[..], 307),
            ("QUERY", "/preserved", "application/json", &b"true"[..], 200),
            (
                "QUERY",
                "/preserve-308",
                "application/json",
                &b"true"[..],
                308,
            ),
            ("QUERY", "/preserved", "application/json", &b"true"[..], 200),
            ("QUERY", "/", "application/sql", &b"select 1"[..], 200),
            ("POST", "/", "text/plain; charset=utf-8", &b"hello"[..], 200),
            (
                "POST",
                "/",
                "text/plain; charset=utf-8",
                &b"body-api"[..],
                200,
            ),
            (
                "PUT",
                "/",
                "text/plain; charset=utf-8",
                &b"generic-body"[..],
                200,
            ),
            (
                "POST",
                "/",
                "application/octet-stream",
                &[0, 255, 65][..],
                200,
            ),
        ] {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, format!("{method} {target} HTTP/1.1\r\n"));
            let mut length = 0usize;
            let mut has_expected_type = content_type == "-";
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                let normalized = header.to_ascii_lowercase();
                if let Some(value) = normalized.strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if content_type != "-" {
                    has_expected_type |=
                        normalized.starts_with(&format!("content-type: {content_type}"));
                }
            }
            assert!(has_expected_type);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            assert_eq!(body, expected_body);
            let mut stream = reader.into_inner();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                if target == "/multiple-location" || target == "/query-multiple-location" {
                    stream
                        .write_all(
                            b"HTTP/1.1 302 Redirect\r\nLocation: /results\r\nLocation: /private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .unwrap();
                    continue;
                }
                if target == "/missing-location" || target == "/query-missing-location" {
                    stream
                        .write_all(
                            b"HTTP/1.1 302 Redirect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .unwrap();
                    continue;
                }
                let location = if target == "/external" || target == "/query-external" {
                    "http://127.0.0.1:1/private"
                } else if target == "/loop" {
                    "/loop"
                } else if target.starts_with("/preserve") {
                    "/preserved"
                } else {
                    "/results"
                };
                write!(
                    stream,
                    "HTTP/1.1 {status} Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            } else {
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .unwrap();
            }
        }
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "PATCH /custom HTTP/1.1\r\n");
        let mut length = 0usize;
        let mut custom_header_seen = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            let normalized = header.to_ascii_lowercase();
            if let Some(value) = normalized.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            if normalized == "x-request: generic\r\n" {
                custom_header_seen = true;
            }
        }
        assert!(custom_header_seen);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        assert_eq!(body, b"patch-body");
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "M-SEARCH /extension HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
        }
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });

    let project = workspace("approved_std_http_query");
    let home = workspace("approved_std_http_query_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
        "project \"std.http.query.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    ).unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTP, HTTPBody, HTTPHeaders, HTTPMethod, URL }} from std.http;\nimport {{ stdout }} from std.io;\nfn main(): Void {{\nmatch URL.parse(\"http://127.0.0.1:{}\") {{\nOk(url) => {{\nmatch URL.resolve(url, \"redirect\") {{ Ok(start) => match HTTP.get_following(start) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"same-origin redirect failed\"); }} Error(_) => stdout.println(\"redirect URL resolve failed\"); }}\nmatch URL.resolve(url, \"external\") {{ Ok(start) => match HTTP.get_following(start, 1) {{ Ok(_) => stdout.println(\"external redirect unexpectedly allowed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"external URL resolve failed\"); }}\nmatch URL.resolve(url, \"query-external\") {{ Ok(start) => match HTTP.query_following(start, \"true\", 1) {{ Ok(_) => stdout.println(\"external QUERY redirect unexpectedly allowed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"external QUERY URL resolve failed\"); }}\nmatch URL.resolve(url, \"multiple-location\") {{ Ok(start) => match HTTP.get_following(start, 1) {{ Ok(_) => stdout.println(\"ambiguous GET redirect followed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"multiple Location URL resolve failed\"); }}\nmatch URL.resolve(url, \"query-multiple-location\") {{ Ok(start) => match HTTP.query_following(start, \"true\", 1) {{ Ok(_) => stdout.println(\"ambiguous QUERY redirect followed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"multiple QUERY Location URL resolve failed\"); }}\nmatch URL.resolve(url, \"missing-location\") {{ Ok(start) => match HTTP.get_following(start, 1) {{ Ok(_) => stdout.println(\"GET redirect without Location followed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"missing Location URL resolve failed\"); }}\nmatch URL.resolve(url, \"query-missing-location\") {{ Ok(start) => match HTTP.query_following(start, \"true\", 1) {{ Ok(_) => stdout.println(\"QUERY redirect without Location followed\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"missing QUERY Location URL resolve failed\"); }}\nmatch URL.resolve(url, \"loop\") {{ Ok(start) => match HTTP.get_following(start, 1) {{ Ok(_) => stdout.println(\"redirect limit ignored\"); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"loop URL resolve failed\"); }}\nmatch HTTP.get_following(url, 21) {{ Ok(_) => stdout.println(\"redirect bound ignored\"); Error(error) => stdout.println(error.to_string()); }}\nmatch HTTP.query_following(url, \"true\", 21) {{ Ok(_) => stdout.println(\"QUERY redirect bound ignored\"); Error(error) => stdout.println(error.to_string()); }}\nmatch HTTP.query_with_content_type(url, \"x\", \"invalid media type\") {{ Ok(_) => stdout.println(\"invalid media type accepted\"); Error(_) => stdout.println(\"invalid query media rejected\"); }}\nmatch URL.resolve(url, \"indirect\") {{ Ok(query_url) => match HTTP.query_following(query_url, \"true\") {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"indirect QUERY result failed\"); }} Error(_) => stdout.println(\"query URL resolve failed\"); }}\nmatch URL.resolve(url, \"preserve-301\") {{ Ok(query_url) => match HTTP.query_following_with_content_type(query_url, \"true\", \"application/json\", 1) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"301 QUERY redirect did not preserve content\"); }} Error(_) => stdout.println(\"301 QUERY URL resolve failed\"); }}\nmatch URL.resolve(url, \"preserve-302\") {{ Ok(query_url) => match HTTP.query_following_with_content_type(query_url, \"true\", \"application/json\", 1) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"302 QUERY redirect did not preserve content\"); }} Error(_) => stdout.println(\"302 QUERY URL resolve failed\"); }}\nmatch URL.resolve(url, \"preserve\") {{ Ok(query_url) => match HTTP.query_following_with_content_type(query_url, \"true\", \"application/json\", 1) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"QUERY redirect did not preserve content\"); }} Error(_) => stdout.println(\"preserved QUERY URL resolve failed\"); }}\nmatch URL.resolve(url, \"preserve-308\") {{ Ok(query_url) => match HTTP.query_following_with_content_type(query_url, \"true\", \"application/json\", 1) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"308 QUERY redirect did not preserve content\"); }} Error(_) => stdout.println(\"308 QUERY URL resolve failed\"); }}\nmatch HTTP.query_with_content_type(url, \"select 1\", \"application/sql\") {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"typed query failed\"); }}\nmatch HTTP.post(url, \"hello\") {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"post failed\"); }}\nmatch HTTP.post_body(url, HTTPBody.Text(\"body-api\")) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"body variant post failed\"); }}\nmatch HTTP.request_body(HTTPMethod.Put, url, HTTPBody.Text(\"generic-body\")) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }}\ninmut payload: Array<Byte> = [0, 255, 65];\nmatch HTTP.post_body(url, HTTPBody.Bytes(payload)) {{ Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"byte post failed\"); }}\nmatch URL.resolve(url, \"custom\") {{ Ok(target) => {{ mut headers = HTTPHeaders.create(); _ = headers.add(\"x-request\", \"generic\"); mut bytes: Array<Byte> = [112 as Byte, 97 as Byte, 116 as Byte, 99 as Byte, 104 as Byte, 45 as Byte, 98 as Byte, 111 as Byte, 100 as Byte, 121 as Byte]; match HTTP.request(HTTPMethod.Patch, target, bytes, headers) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }} }} Error(_) => stdout.println(\"custom URL resolve failed\"); }}\ninmut empty_body: Array<Byte> = [];\nmatch HTTP.request_named(\"bad method\", url, empty_body, HTTPHeaders.create()) {{ Ok(_) => stdout.println(\"invalid extension method accepted\"); Error(_) => stdout.println(\"invalid extension method rejected\"); }}\nmatch URL.resolve(url, \"extension\") {{ Ok(target) => match HTTP.request_named(\"M-SEARCH\", target, empty_body, HTTPHeaders.create()) {{ Ok(response) => stdout.println(response.status_code()); Error(error) => stdout.println(error.to_string()); }} Error(_) => stdout.println(\"extension URL resolve failed\"); }}\n}}\nError(_) => stdout.println(\"invalid URL\");\n}}\n}}",

            address.port()
        ),
    ).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP QUERY approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nPermissionDenied\nPermissionDenied\nProtocol\nProtocol\nProtocol\nProtocol\nRedirectLimit\nRedirectLimit\nRedirectLimit\ninvalid query media rejected\n200\n200\n200\n200\n200\n200\n200\n200\n200\n200\n200\ninvalid extension method rejected\n200\n",
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    server.join().unwrap();
}
#[test]
fn bundled_std_net_ip_address_parser_canonicalizes_literals_without_dns() {
    let output = crate_of(
        "standard_net_ip_address_parse",
        &[(
            "main.zrk",
            "import { IPAddress } from std.net::address;\n\
             import { stdout } from std.io;\n\
             fn main(): Void {\n\
                 match IPAddress.parse(\"2001:0db8:0:0:0:0:0:1\") {\n\
                     Ok(address) => stdout.println(address.as_string());\n\
                     Error(_) => stdout.println(\"valid address rejected\");\n\
                 };\n\
                 match IPAddress.parse(\"localhost\") {\n\
                     Ok(_) => stdout.println(\"host name accepted\");\n\
                     Error(_) => stdout.println(\"host name rejected\");\n\
                 };\n\
                 match IPAddress.parse(\"fe80::1%en0\") {\n\
                     Ok(_) => stdout.println(\"scoped IPv6 address accepted\");\n\
                     Error(_) => stdout.println(\"scoped IPv6 address rejected\");\n\
                 };\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "2001:db8::1\nhost name rejected\nscoped IPv6 address rejected\n"
    );
}

#[test]
fn bundled_std_net_socket_address_formats_ipv4_and_ipv6_origins() {
    let output = crate_of(
        "standard_net_socket_address",
        &[(
            "main.zrk",
            "import { SocketAddress } from std.net::address;\n\
             import { stdout } from std.io;\n\
             fn main(): Void {\n\
                 match SocketAddress.parse(\"127.0.0.1\", 8080) {\n\
                     Ok(address) => stdout.println(address.as_tcp_origin());\n\
                     Error(_) => stdout.println(\"valid IPv4 socket address rejected\");\n\
                 };\n\
                 match SocketAddress.parse(\"2001:db8::1\", 443) {\n\
                     Ok(address) => stdout.println(address.as_tcp_origin());\n\
                     Error(_) => stdout.println(\"valid IPv6 socket address rejected\");\n\
                 };\n\
                 match SocketAddress.parse(\"localhost\", 80) {\n\
                     Ok(_) => stdout.println(\"host name accepted as an IP literal\");\n\
                     Error(_) => stdout.println(\"host name rejected\");\n\
                 };\n\
                 match SocketAddress.parse(\"127.0.0.1\", 0) {\n\
                     Ok(_) => stdout.println(\"zero port accepted\");\n\
                     Error(_) => stdout.println(\"zero port rejected\");\n\
                 };\n\
                 match SocketAddress.parse(\"fe80::1%en0\", 443) {\n\
                     Ok(_) => stdout.println(\"scoped IPv6 socket address accepted\");\n\
                     Error(_) => stdout.println(\"scoped IPv6 socket address rejected\");\n\
                 };\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "tcp://127.0.0.1:8080\ntcp://[2001:db8::1]:443\nhost name rejected\nzero port rejected\nscoped IPv6 socket address rejected\n"
    );
}

#[test]
fn bundled_std_net_host_port_address_normalizes_idna_without_dns() {
    let output = crate_of(
        "standard_net_host_port_address",
        &[(
            "main.zrk",
            "import { HostPortAddress } from std.net::address;\n\
             import { TCPError, TCPStream } from std.net::tcp;\n\
             import { stdout } from std.io;\n\
             fn connect_host(address: HostPortAddress): Result<TCPStream, TCPError> {\n\
                 return TCPStream.connect_to_host(address);\n\
             }\n\
             fn main(): Void {\n\
                 match HostPortAddress.parse(\"Exämple.com\", 443) {\n\
                     Ok(address) => stdout.println(address.as_tcp_origin());\n\
                     Error(_) => stdout.println(\"valid hostname rejected\");\n\
                 };\n\
                 match HostPortAddress.parse(\"2001:0db8::1\", 443) {\n\
                     Ok(address) => stdout.println(address.as_tcp_origin());\n\
                     Error(_) => stdout.println(\"valid IPv6 host rejected\");\n\
                 };\n\
                 match HostPortAddress.parse(\"bad..example\", 80) {\n\
                     Ok(_) => stdout.println(\"invalid hostname accepted\");\n\
                     Error(_) => stdout.println(\"invalid hostname rejected\");\n\
                 };\n\
                 match HostPortAddress.parse(\"example.com\", 0) {\n\
                     Ok(_) => stdout.println(\"zero port accepted\");\n\
                     Error(_) => stdout.println(\"zero port rejected\");\n\
                 };\n\
                 match HostPortAddress.parse(\"localhost\", 80) {\n\
                     Ok(address) => match TCPStream.connect_to_host(address) {\n\
                         Ok(_) => stdout.println(\"unapproved host connected\");\n\
                         Error(error) => stdout.println(error);\n\
                     };\n\
                     Error(error) => stdout.println(error);\n\
                 };\n\
                 match HostPortAddress.parse(\"fe80::1%en0\", 443) {\n\
                     Ok(_) => stdout.println(\"scoped IPv6 host accepted\");\n\
                     Error(_) => stdout.println(\"scoped IPv6 host rejected\");\n\
                 };\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "tcp://xn--exmple-cua.com:443\ntcp://[2001:db8::1]:443\ninvalid hostname rejected\nzero port rejected\nPermissionDenied\nscoped IPv6 host rejected\n"
    );
}

#[test]
fn bundled_std_net_tls_connect_fails_closed_without_network_approval() {
    let source = r#"import { TLSError, TLSStream } from std.net::tls;
import { stdout } from std.io;
fn connect(): Result<TLSStream, TLSError> {
    return TLSStream.connect_with_trust_roots(
        "https://127.0.0.1:443",
        [[1 as Byte]],
        false,
    );
}
fn main(): Void {
    match connect() {
        Ok(_) => stdout.println("unexpected connection");
        Error(_) => stdout.println("permission denied");
    };
}"#;
    let output = crate_of("standard_net_tls_denied", &[("main.zrk", source)]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "permission denied\n");
}

#[test]
fn bundled_std_net_tls_alpn_api_fails_closed_without_network_approval() {
    let source = r#"import { TLSError, TLSStream } from std.net::tls;
import { stdout } from std.io;
fn main(): Void {
    mut empty: Array<String> = [];
    match TLSStream.connect_with_alpn("https://127.0.0.1:443", empty) {
        Ok(_) => stdout.println("unexpected empty ALPN connection");
        Error(error) => stdout.println(error.to_string());
    };
    mut protocols: Array<String> = ["h2", "http/1.1"];
    match TLSStream.connect_with_alpn("https://127.0.0.1:443", protocols) {
        Ok(_) => stdout.println("unexpected connection");
        Error(error) => stdout.println(error.to_string());
    };
    mut system_roots: Array<Array<Byte>> = [];
    mut http_protocols: Array<String> = ["h2", "http/1.1"];
    match TLSStream.connect_with_options(
        "https://127.0.0.1:443", system_roots, true, http_protocols,
    ) {
        Ok(_) => stdout.println("unexpected configured connection");
        Error(error) => stdout.println(error.to_string());
    };
}"#;
    let output = crate_of("standard_net_tls_alpn_denied", &[("main.zrk", source)]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "InvalidConfiguration\nPermissionDenied\nPermissionDenied\n"
    );
}

#[test]
fn bundled_std_net_tls_client_completes_approved_trusted_public_api_handshake() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("https://127.0.0.1:{}", address.port());
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let mut server_config = std::sync::Arc::new(
        tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                    certificate.to_vec(),
                )],
                tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(private_key.to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    std::sync::Arc::get_mut(&mut server_config)
        .unwrap()
        .alpn_protocols = vec![b"zirktest/1".to_vec()];
    let server = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let (socket, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("TLS client did not connect: {error}")),
            }
        };
        socket
            .set_nonblocking(false)
            .map_err(|error| error.to_string())?;
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let connection = tokio_rustls::rustls::ServerConnection::new(server_config)
            .map_err(|error| error.to_string())?;
        let mut tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
        let mut request = [0; 4];
        tls.read_exact(&mut request)
            .map_err(|error| error.to_string())?;
        if &request != b"ping" {
            return Err(format!("unexpected TLS request: {request:?}"));
        }
        tls.write_all(b"pong").map_err(|error| error.to_string())
    });

    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let root_literal = root
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"import {{ TLSReadResult, TLSError, TLSStream }} from std.net::tls;
import {{ stdout }} from std.io;
fn read(stream: TLSStream): TLSReadResult<Array<Byte>> {{ return stream.read(4); }}
fn tls_write(stream: TLSStream, bytes: Array<Byte>): Result<Void, TLSError> {{ return stream.write_all(bytes); }}
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [[{root_literal}]];
    mut protocols: Array<String> = ["zirktest/1"];
    match TLSStream.connect_with_options("{origin}", roots, false, protocols) {{
        Error(_) => stdout.println("TLS connect failed");
        Ok(stream) => {{
            match stream.negotiated_alpn() {{
                Ok(protocol) => match protocol {{
                    null => stdout.println("missing ALPN");
                    value => stdout.println(value);
                }};
                Error(_) => stdout.println("ALPN query failed");
            }};
            mut request: Array<Byte> = [112 as Byte, 105 as Byte, 110 as Byte, 103 as Byte];
            _ = tls_write(stream, request);
            match read(stream) {{
                TLSReadResult.Value(_) => stdout.println("authenticated TLS exchange");
                _ => stdout.println("TLS read failed");
            }}
        }}
    }}
}}"#,
        origin = origin,
    );

    let project = workspace("approved_std_net_tls_client");
    let home = workspace("approved_std_net_tls_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tls.client.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TLS connect permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved Zirk TLS client");
    let server_result = server.join().expect("TLS fixture should not panic");
    assert!(
        server_result.is_ok(),
        "{server_result:?}; client stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "zirktest/1\nauthenticated TLS exchange\n"
    );
}

#[test]
fn bundled_std_net_tcp_upgrades_the_same_authorized_socket_to_tls() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let tcp_origin = format!("tcp://127.0.0.1:{}", address.port());
    let https_origin = format!("https://127.0.0.1:{}", address.port());
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let mut server_config = std::sync::Arc::new(
        tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                    certificate.to_vec(),
                )],
                tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(private_key.to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    std::sync::Arc::get_mut(&mut server_config)
        .unwrap()
        .alpn_protocols = vec![b"zirk-upgrade/1".to_vec()];
    let server = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let (socket, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("TCP client did not connect: {error}")),
            }
        };
        socket
            .set_nonblocking(false)
            .map_err(|error| error.to_string())?;
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let connection = tokio_rustls::rustls::ServerConnection::new(server_config)
            .map_err(|error| error.to_string())?;
        let mut tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
        let mut request = [0; 4];
        tls.read_exact(&mut request)
            .map_err(|error| error.to_string())?;
        if &request != b"ping" {
            return Err(format!("unexpected upgraded TLS request: {request:?}"));
        }
        tls.write_all(b"pong").map_err(|error| error.to_string())?;
        match listener.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Ok(_) => Err("TLS upgrade opened a second TCP connection".into()),
            Err(error) => Err(format!("could not check accepted connections: {error}")),
        }
    });

    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let root_literal = root
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"import {{ TCPStream }} from std.net::tcp;
import {{ TLSReadResult }} from std.net::tls;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [[{root_literal}]];
    mut protocols: Array<String> = ["zirk-upgrade/1"];
    match TCPStream.connect("{tcp_origin}") {{
        Error(_) => stdout.println("TCP connect failed");
        Ok(tcp) => match tcp.upgrade_tls("{https_origin}", roots, false, protocols) {{
            Error(_) => stdout.println("TLS upgrade failed");
            Ok(tls) => {{
                stdout.println(tcp.is_closed().to_string());
                match tls.negotiated_alpn() {{
                    Ok(protocol) => match protocol {{
                        null => stdout.println("missing ALPN");
                        value => stdout.println(value);
                    }};
                    Error(_) => stdout.println("ALPN query failed");
                }};
                mut request: Array<Byte> = [112 as Byte, 105 as Byte, 110 as Byte, 103 as Byte];
                _ = tls.write_all(request);
                match tls.read(4) {{
                    TLSReadResult.Value(_) => stdout.println("same-socket TLS exchange");
                    _ => stdout.println("TLS read failed");
                }}
            }}
        }};
    }}
}}"#,
        tcp_origin = tcp_origin,
        https_origin = https_origin,
    );

    let project = workspace("approved_std_net_tcp_tls_upgrade");
    let home = workspace("approved_std_net_tcp_tls_upgrade_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tcp.tls-upgrade.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{tcp_origin}\", \"{https_origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TCP and TLS permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved Zirk TCP-to-TLS upgrade");
    let server_result = server.join().expect("upgrade fixture should not panic");
    assert!(
        server_result.is_ok(),
        "{server_result:?}; client stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "true\nzirk-upgrade/1\nsame-socket TLS exchange\n"
    );
}

#[test]
fn bundled_std_net_tls_client_identity_completes_approved_mutual_authentication() {
    use std::io::Write;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use tokio_rustls::rustls::{RootCertStore, server::WebPkiClientVerifier};

    let server_certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let server_private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let client_ca = include_bytes!("../../zirk-runtime/testdata/mtls-client-ca.der");
    let client_certificate = include_bytes!("../../zirk-runtime/testdata/mtls-client.der");
    let mut client_roots = RootCertStore::empty();
    client_roots
        .add(CertificateDer::from(client_ca.to_vec()))
        .unwrap();
    let verifier = WebPkiClientVerifier::builder(std::sync::Arc::new(client_roots))
        .build()
        .unwrap();
    let mut server_config = tokio_rustls::rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![CertificateDer::from(server_certificate.to_vec())],
            PrivateKeyDer::try_from(server_private_key.to_vec()).unwrap(),
        )
        .unwrap();
    server_config.alpn_protocols = vec![b"zirk-mtls-test/1".to_vec()];

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("https://127.0.0.1:{}", address.port());
    let server = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let (socket, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("mTLS client did not connect: {error}")),
            }
        };
        socket
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(async move {
            let socket = tokio::net::TcpStream::from_std(socket).map_err(|e| e.to_string())?;
            let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(server_config));
            let mut stream = acceptor.accept(socket).await.map_err(|e| e.to_string())?;
            if stream.get_ref().1.alpn_protocol() != Some(b"zirk-mtls-test/1".as_slice()) {
                return Err("mTLS ALPN did not negotiate".to_owned());
            }
            let peer_chain = stream
                .get_ref()
                .1
                .peer_certificates()
                .ok_or_else(|| "client certificate was not presented".to_owned())?;
            if peer_chain.first().map(|cert| cert.as_ref()) != Some(client_certificate.as_slice()) {
                return Err("unexpected client certificate".to_owned());
            }
            let mut request = [0; 4];
            stream
                .read_exact(&mut request)
                .await
                .map_err(|error| error.to_string())?;
            if &request != b"mTLS" {
                return Err(format!("unexpected mTLS request: {request:?}"));
            }
            stream
                .write_all(b"ok")
                .await
                .map_err(|error| error.to_string())
        })
    });

    let der_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let root_literal = der_literal(include_bytes!(
        "../../zirk-runtime/testdata/tls-loopback-root.der"
    ));
    let client_certificate_literal = der_literal(client_certificate);
    let client_key_literal = der_literal(include_bytes!(
        "../../zirk-runtime/testdata/mtls-client-key.der"
    ));
    let source = format!(
        r#"import {{ TLSReadResult, TLSError, TLSStream }} from std.net::tls;
import {{ stdout }} from std.io;
fn read(stream: TLSStream): TLSReadResult<Array<Byte>> {{ return stream.read(2); }}
fn tls_write(stream: TLSStream, bytes: Array<Byte>): Result<Void, TLSError> {{ return stream.write_all(bytes); }}
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [[{root_literal}]];
    mut chain: Array<Array<Byte>> = [[{client_certificate_literal}]];
    mut key: Array<Byte> = [{client_key_literal}];
    mut protocols: Array<String> = ["zirk-mtls-test/1"];
    match TLSStream.connect_with_client_identity(
        "{origin}", roots, chain, key, false, protocols,
    ) {{
        Error(error) => stdout.println(error.to_string());
        Ok(stream) => {{
            match stream.negotiated_alpn() {{
                Ok(protocol) => match protocol {{
                    null => stdout.println("missing ALPN");
                    value => stdout.println(value);
                }};
                Error(_) => stdout.println("ALPN query failed");
            }};
            mut request: Array<Byte> = [109 as Byte, 84 as Byte, 76 as Byte, 83 as Byte];
            _ = tls_write(stream, request);
            match read(stream) {{
                TLSReadResult.Value(_) => stdout.println("mutual TLS exchange");
                _ => stdout.println("TLS read failed");
            }}
            _ = stream.close();
        }}
    }}
}}"#,
        origin = origin,
    );

    let project = workspace("approved_std_net_mtls_client");
    let home = workspace("approved_std_net_mtls_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tls.mtls.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit mTLS network permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved Zirk mTLS client");
    let server_result = server.join().expect("mTLS fixture should not panic");
    assert!(
        server_result.is_ok(),
        "{server_result:?}; client stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        run.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "zirk-mtls-test/1\nmutual TLS exchange\n"
    );
}

#[test]
fn bundled_std_net_tls_listener_checks_permission_before_identity_parsing() {
    let source = r#"import { TLSError, TLSResourceError, TLSListener, TLSStream } from std.net::tls;
import { stdout } from std.io;
fn accept(listener: TLSListener): Result<TLSStream, TLSError> { return listener.accept(); }
fn close_listener(listener: TLSListener): Result<Void, TLSResourceError> { return listener.close(); }
fn listener_is_closed(listener: TLSListener): Boolean { return listener.is_closed(); }
fn peer_address(stream: TLSStream): Result<String, TLSError> { return stream.peer_address(); }
fn main(): Void {
    mut certificate_chain: Array<Array<Byte>> = Array(1);
    mut certificate: Array<Byte> = Array(1);
    mut private_key: Array<Byte> = Array(1);
    mut client_roots: Array<Array<Byte>> = Array(1);
    certificate[0] = 1 as Byte;
    private_key[0] = 1 as Byte;
    certificate_chain[0] = certificate;
    client_roots[0] = certificate;
    match TLSListener.bind(
        "tcp://127.0.0.1:0",
        certificate_chain,
        private_key,
        false,
    ) {
        Ok(_) => stdout.println("unexpected listener");
        Error(error) => match error {
            TLSError.PermissionDenied => stdout.println("permission denied");
            _ => stdout.println("wrong error");
        };
    };
    match TLSListener.bind_with_client_auth(
        "tcp://127.0.0.1:0",
        certificate_chain,
        private_key,
        client_roots,
        false,
    ) {
        Ok(_) => stdout.println("unexpected mutual-auth listener");
        Error(error) => match error {
            TLSError.PermissionDenied => stdout.println("mutual auth permission denied");
            _ => stdout.println("wrong mutual-auth error");
        };
    };
}"#;
    let output = crate_of("standard_net_tls_listener_denied", &[("main.zrk", source)]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "permission denied\nmutual auth permission denied\n"
    );
}

#[test]
fn bundled_std_net_tls_listener_reports_invalid_server_identity_after_approval() {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let valid_certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der")
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");
    let valid_private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der")
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");

    let source = format!(
        r#"import {{ TLSError, TLSListener }} from std.net::tls;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut malformed: Array<Byte> = [1 as Byte];
    mut empty_chain: Array<Array<Byte>> = Array(0);
    mut malformed_chain: Array<Array<Byte>> = [malformed];
    mut certificate: Array<Byte> = [{valid_certificate}];
    mut valid_chain: Array<Array<Byte>> = [certificate];
    mut valid_private_key: Array<Byte> = [{valid_private_key}];
    match TLSListener.bind("tcp://127.0.0.1:{port}", empty_chain, malformed, false) {{
        Error(error) => match error {{
            TLSError.InvalidServerIdentity => stdout.println("empty chain rejected");
            _ => stdout.println("wrong empty-chain error");
        }}
        Ok(_) => stdout.println("empty chain unexpectedly accepted");
    }}
    match TLSListener.bind("tcp://127.0.0.1:{port}", malformed_chain, malformed, false) {{
        Error(error) => match error {{
            TLSError.InvalidServerIdentity => stdout.println("malformed DER rejected");
            _ => stdout.println("wrong malformed-DER error");
        }}
        Ok(_) => stdout.println("malformed DER unexpectedly accepted");
    }}
    match TLSListener.bind("tcp://127.0.0.1:{port}", valid_chain, malformed, false) {{
        Error(error) => match error {{
            TLSError.InvalidServerIdentity => stdout.println("malformed private key rejected");
            _ => stdout.println("wrong malformed-private-key error");
        }}
        Ok(_) => stdout.println("malformed private key unexpectedly accepted");
    }}
    match TLSListener.bind("tcp://127.0.0.1:{port}", valid_chain, valid_private_key, false) {{
        Error(_) => stdout.println("invalid identity left the listener bound");
        Ok(listener) => {{
            stdout.println("listener remained bindable");
            _ = listener.close();
        }}
    }}
}}"#,
    );
    let project = workspace("approved_std_net_tls_invalid_identity");
    let home = workspace("approved_std_net_tls_invalid_identity_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tls.invalid_identity.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TLS identity approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved invalid TLS identity cases");
    assert!(
        run.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "empty chain rejected\nmalformed DER rejected\nmalformed private key rejected\nlistener remained bindable\n"
    );
}

#[test]
fn bundled_std_net_tls_listener_completes_approved_public_api_handshake() {
    use std::io::{Read, Write};

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);

    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"import {{ TLSError, TLSResourceError, TLSReadResult, TLSListener, TLSStream }} from std.net::tls;
import {{ stdout }} from std.io;
fn accept(listener: TLSListener): Result<TLSStream, TLSError> {{ return listener.accept(); }}
fn read(stream: TLSStream): TLSReadResult<Array<Byte>> {{ return stream.read(4); }}
fn tls_write(stream: TLSStream, bytes: Array<Byte>): Result<Void, TLSError> {{ return stream.write_all(bytes); }}
fn close(listener: TLSListener): Result<Void, TLSResourceError> {{ return listener.close(); }}
fn main(): Void {{
    mut cert: Array<Byte> = [{certificate}];
    mut key: Array<Byte> = [{private_key}];
    mut chain: Array<Array<Byte>> = [cert];
    match TLSListener.bind("tcp://127.0.0.1:{port}", chain, key, false) {{
        Error(_) => stdout.println("bind failed");
        Ok(listener) => {{
            stdout.println("ready");
            match accept(listener) {{
                Error(_) => stdout.println("accept failed");
                Ok(stream) => {{
                    match read(stream) {{
                        TLSReadResult.Value(_) => {{
                            mut response: Array<Byte> = [112 as Byte, 111 as Byte, 110 as Byte, 103 as Byte];
                            _ = tls_write(stream, response);
                            stdout.println("served");
                        }}
                        _ => stdout.println("read failed");
                    }}
                }}
            }}
            _ = close(listener);
        }}
    }}
}}"#,
        certificate = byte_literal(certificate),
        private_key = byte_literal(private_key),
    );

    let project = workspace("approved_std_net_tls_server");
    let home = workspace("approved_std_net_tls_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tls.server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TLS listen permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled Zirk TLS server");
    let client = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("could not connect to TLS listener: {error}")),
            }
        };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                root.to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let config = std::sync::Arc::new(
            tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        );
        let name = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
            .map_err(|error| error.to_string())?;
        let connection = tokio_rustls::rustls::ClientConnection::new(config, name)
            .map_err(|error| error.to_string())?;
        let mut tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
        tls.write_all(b"ping").map_err(|error| error.to_string())?;
        let mut response = [0; 4];
        tls.read_exact(&mut response)
            .map_err(|error| error.to_string())?;
        if &response != b"pong" {
            return Err(format!("unexpected TLS response: {response:?}"));
        }
        Ok(())
    });

    let client_result = client.join().expect("TLS client thread should not panic");
    if let Err(error) = client_result {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "public TLS client failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    assert!(stdout.contains("ready\n"), "{stdout}");
    assert!(stdout.contains("served\n"), "{stdout}");
}

#[test]
fn bundled_std_net_tls_listener_requires_client_identity_and_exposes_validated_fingerprint() {
    use std::io::{Read, Write};

    use sha2::{Digest, Sha256};

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let server_cert = literal(include_bytes!(
        "../../zirk-runtime/testdata/tls-loopback-cert.der"
    ));
    let server_key = literal(include_bytes!(
        "../../zirk-runtime/testdata/tls-loopback-key.der"
    ));
    let client_ca = literal(include_bytes!(
        "../../zirk-runtime/testdata/mtls-client-ca.der"
    ));
    let source = format!(
        r#"import {{ TLSError, TLSReadResult, TLSListener, TLSStream }} from std.net::tls;
import {{ stdout }} from std.io;
fn accept(listener: TLSListener): Result<TLSStream, TLSError> {{ return listener.accept(); }}
fn read(stream: TLSStream): TLSReadResult<Array<Byte>> {{ return stream.read(4); }}
fn tls_write(stream: TLSStream, bytes: Array<Byte>): Result<Void, TLSError> {{ return stream.write_all(bytes); }}
fn main(): Void {{
    mut cert: Array<Byte> = [{server_cert}];
    mut chain: Array<Array<Byte>> = [cert];
    mut key: Array<Byte> = [{server_key}];
    mut ca: Array<Byte> = [{client_ca}];
    mut roots: Array<Array<Byte>> = [ca];
    match TLSListener.bind_with_client_auth(
        "tcp://127.0.0.1:{port}", chain, key, roots, false,
    ) {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => {{
            match accept(listener) {{
                Error(error) => match error {{
                    TLSError.TlsHandshake => stdout.println("client without identity rejected");
                    _ => stdout.println("unexpected unauthenticated error");
                }};
                Ok(_) => stdout.println("client without identity accepted");
            }}
            match accept(listener) {{
            Error(error) => stdout.println(error.to_string());
            Ok(stream) => {{
                match stream.peer_certificate_sha256() {{
                    Error(_) => stdout.println("fingerprint error");
                    Ok(fingerprint) => match fingerprint {{
                        null => stdout.println("client identity missing");
                        value => stdout.println(value);
                    }};
                }};
                match read(stream) {{
                    TLSReadResult.Value(_) => {{
                        mut response: Array<Byte> = [112 as Byte, 111 as Byte, 110 as Byte, 103 as Byte];
                        _ = tls_write(stream, response);
                        stdout.println("served mTLS");
                    }}
                    _ => stdout.println("TLS read failed");
                }}
            }}
            }}
        }}
    }}
}}"#,
        port = port,
    );
    let client_cert = include_bytes!("../../zirk-runtime/testdata/mtls-client.der");
    let expected_fingerprint = Sha256::digest(client_cert)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let project = workspace("approved_std_net_tls_mtls_server");
    let home = workspace("approved_std_net_tls_mtls_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tls.mtls_server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit mTLS listener permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled Zirk mTLS server");

    let client = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let unauthenticated_socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("could not connect to mTLS listener: {error}")),
            }
        };
        unauthenticated_socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let unauthenticated_config = std::sync::Arc::new(
            tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates({
                    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
                    roots
                        .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                            include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der")
                                .to_vec(),
                        ))
                        .map_err(|error| error.to_string())?;
                    roots
                })
                .with_no_client_auth(),
        );
        let unauthenticated_name =
            tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
                .map_err(|error| error.to_string())?;
        let unauthenticated_connection = tokio_rustls::rustls::ClientConnection::new(
            unauthenticated_config,
            unauthenticated_name,
        )
        .map_err(|error| error.to_string())?;
        let mut unauthenticated = tokio_rustls::rustls::StreamOwned::new(
            unauthenticated_connection,
            unauthenticated_socket,
        );
        let rejected = unauthenticated.write_all(b"ping").is_err()
            || unauthenticated.read(&mut [0; 1]).is_err();
        if !rejected {
            return Err("server accepted a client without a certificate".to_owned());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("could not reconnect to mTLS listener: {error}")),
            }
        };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der").to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let client_chain = vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
            client_cert.to_vec(),
        )];
        let client_key = tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(
            include_bytes!("../../zirk-runtime/testdata/mtls-client-key.der").to_vec(),
        )
        .map_err(|error| error.to_string())?;
        let config = std::sync::Arc::new(
            tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_client_auth_cert(client_chain, client_key)
                .map_err(|error| error.to_string())?,
        );
        let name = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
            .map_err(|error| error.to_string())?;
        let connection = tokio_rustls::rustls::ClientConnection::new(config, name)
            .map_err(|error| error.to_string())?;
        let mut tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
        tls.write_all(b"ping").map_err(|error| error.to_string())?;
        let mut response = [0; 4];
        tls.read_exact(&mut response)
            .map_err(|error| error.to_string())?;
        if &response != b"pong" {
            return Err(format!("unexpected mTLS response: {response:?}"));
        }
        Ok(())
    });
    let client_result = client.join().expect("mTLS client thread should not panic");
    if let Err(error) = client_result {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "mTLS client failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    assert!(stdout.contains(&expected_fingerprint), "stdout: {stdout}");
    assert!(
        !stdout.contains("client identity missing"),
        "stdout: {stdout}"
    );
}

#[test]
fn bundled_http1_server_interoperates_with_independent_hyper_peer() {
    use hyper::client::conn::http1;
    use std::io::Write;

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let project = workspace("approved_std_http1_hyper_server");
    let home = workspace("approved_std_http1_hyper_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http1.hyper.server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"{}\"]; during: runtime; }}\n}}\n}}\n",
            address
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPError, HTTPMethod, HTTPNext, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
fn reply(_request: HTTPRequest, _parameters: HTTPRouteParameters): HTTPResponse {{
    return HTTPResponse.from_text(200, "zirk-http1-server");
}}
fn main(): Void {{
    match HTTPServer.bind("tcp://{address}") {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            _ = router.route(HTTPMethod.Get, "/interop", reply);
            match router.serve_and_close(listener, 1) {{
                Error(error) => stdout.println(error.to_string());
                Ok(_) => stdout.println("served");
            }}
        }}
    }}
}}"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP/1.1 listen approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled-Zirk HTTP/1.1 server");
    let peer = std::thread::spawn(move || -> Result<(u16, String), String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(async move {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            let socket = loop {
                match tokio::net::TcpStream::connect(address).await {
                    Ok(socket) => break socket,
                    Err(error) if std::time::Instant::now() < deadline => {
                        let _ = error;
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    }
                    Err(error) => {
                        return Err(format!("could not connect to Zirk HTTP server: {error}"));
                    }
                }
            };
            let (mut sender, connection) = http1::handshake(hyper_util::rt::TokioIo::new(socket))
                .await
                .map_err(|error| error.to_string())?;
            let driver = tokio::spawn(connection);
            let request = hyper::Request::builder()
                .method(hyper::Method::GET)
                .uri(format!("http://{address}/interop"))
                .body(http_body_util::Empty::<bytes::Bytes>::new())
                .map_err(|error| error.to_string())?;
            let response = sender
                .send_request(request)
                .await
                .map_err(|error| error.to_string())?;
            let status = response.status().as_u16();
            let body = http_body_util::BodyExt::collect(response.into_body())
                .await
                .map_err(|error| error.to_string())?
                .to_bytes();
            let body = String::from_utf8(body.to_vec()).map_err(|error| error.to_string())?;
            let _ = driver.await;
            Ok((status, body))
        })
    });

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        peer.join().expect("Hyper peer must not panic").unwrap(),
        (200, "zirk-http1-server".to_owned())
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "served\n"
    );
}

#[test]
fn bundled_http_server_contains_handler_throwable_and_continues_finite_scope() {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let project = workspace("approved_std_http_handler_failure");
    let home = workspace("approved_std_http_handler_failure_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.handler.failure.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"{}\"]; during: runtime; }}\n}}\n}}\n",
            address
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPError, HTTPMethod, HTTPNext, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;

class HandlerFailure implements Throwable {{
    construct() {{}}
    message(): String {{ return "private handler detail"; }}
    code(): String {{ return "PRIVATE_HANDLER_DETAIL"; }}
    cause(): Error? {{ return null; }}
    stack_trace(): StackTrace {{ return StackTrace(); }}
}}

fn fail(_request: HTTPRequest, _parameters: HTTPRouteParameters): HTTPResponse throws HandlerFailure {{
    throw HandlerFailure();
}}

fn succeed(_request: HTTPRequest, _parameters: HTTPRouteParameters): HTTPResponse {{
    return HTTPResponse.from_text(200, "still serving");
}}

fn fail_structured(_request: HTTPRequest, _parameters: HTTPRouteParameters): Result<HTTPResponse, HTTPError> {{
    return Error(HTTPError.Transport);
}}

fn middleware_result(request: HTTPRequest, next: HTTPNext): Result<HTTPResponse, HTTPError> {{
    if request.path() == "/middleware-error" {{ return Error(HTTPError.Transport); }}
    return Ok(next.proceed(request));
}}

fn main(): Void {{
    match HTTPServer.bind("tcp://{address}") {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            _ = router.route(HTTPMethod.Get, "/fail", fail);
            _ = router.route_result(HTTPMethod.Get, "/structured", fail_structured);
            _ = router.route(HTTPMethod.Get, "/ok", succeed);
            _ = router.add_middleware_result(middleware_result);
            match router.serve_and_close(listener, 4) {{
                Error(error) => stdout.println(error.to_string());
                Ok(_) => stdout.println("served both requests");
            }}
        }}
    }}
}}"#
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP listen approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled Zirk server with throwing handler");
    let peer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut responses = Vec::new();
        for path in ["/fail", "/structured", "/middleware-error", "/ok"] {
            let mut stream = loop {
                match std::net::TcpStream::connect(address) {
                    Ok(stream) => break stream,
                    Err(error) if std::time::Instant::now() < deadline => {
                        let _ = error;
                        std::thread::sleep(std::time::Duration::from_millis(25));
                    }
                    Err(error) => panic!("could not connect to Zirk server: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            std::io::Read::read_to_string(&mut stream, &mut response).unwrap();
            responses.push(response);
        }
        responses
    });
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = peer.join().expect("HTTP peer must not panic");
    assert!(responses[0].starts_with("HTTP/1.1 500"), "{}", responses[0]);
    assert!(
        responses[0].contains("Internal Server Error"),
        "{}",
        responses[0]
    );
    assert!(
        !responses[0].contains("private handler detail"),
        "{}",
        responses[0]
    );
    assert!(responses[1].starts_with("HTTP/1.1 500"), "{}", responses[1]);
    assert!(
        responses[1].contains("Internal Server Error"),
        "{}",
        responses[1]
    );
    assert!(!responses[1].contains("Transport"), "{}", responses[1]);
    assert!(responses[2].starts_with("HTTP/1.1 500"), "{}", responses[2]);
    assert!(
        responses[2].contains("Internal Server Error"),
        "{}",
        responses[2]
    );
    assert!(!responses[2].contains("Transport"), "{}", responses[2]);
    assert!(responses[3].starts_with("HTTP/1.1 200"), "{}", responses[3]);
    assert!(responses[3].contains("still serving"), "{}", responses[3]);
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "served both requests\n"
    );
}

#[test]
fn bundled_http_server_serves_over_an_accepted_tls_stream() {
    use std::io::{Read, Write};

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);

    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"import {{ TLSListener }} from std.net::tls;
import {{ HTTPError, HTTPHeaders, HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut cert: Array<Byte> = [{certificate}];
    mut key: Array<Byte> = [{private_key}];
    mut chain: Array<Array<Byte>> = [cert];
    match HTTPServer.bind_tls("tcp://127.0.0.1:{port}", chain, key) {{
        Error(_) => stdout.println("bind failed");
        Ok(listener) => {{
            stdout.println("ready");
            mut router: HTTPRouter = HTTPRouter();
            match router.route(HTTPMethod.Get, "/ping", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => HTTPResponse.from_text(200, "secure " + request.path())) {{
                Error(_) => stdout.println("route failed");
                Ok(_) => match router.serve_tls_and_close(listener, 1) {{
                    Ok(_) => stdout.println("served");
                    Error(error) => stdout.println(error.to_string());
                }};
            }}
        }}
    }}
}}"#,
        certificate = byte_literal(certificate),
        private_key = byte_literal(private_key),
    );

    let project = workspace("approved_std_http_tls_server");
    let home = workspace("approved_std_http_tls_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.tls.server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTPS listen permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled Zirk HTTPS server");
    let client = std::thread::spawn(move || -> Result<String, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("could not connect to HTTPS listener: {error}")),
            }
        };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                root.to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let config = std::sync::Arc::new(
            tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        );
        let name = tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
            .map_err(|error| error.to_string())?;
        let connection = tokio_rustls::rustls::ClientConnection::new(config, name)
            .map_err(|error| error.to_string())?;
        let mut tls = tokio_rustls::rustls::StreamOwned::new(connection, socket);
        tls.write_all(b"GET /ping HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .map_err(|error| error.to_string())?;
        let mut response = Vec::new();
        loop {
            let mut byte = [0];
            tls.read_exact(&mut byte)
                .map_err(|error| error.to_string())?;
            response.push(byte[0]);
            if response.ends_with(b"\r\n\r\n") {
                break;
            }
            if response.len() > 16 * 1024 {
                return Err("HTTP response headers exceeded the test bound".into());
            }
        }
        let headers = std::str::from_utf8(&response).map_err(|error| error.to_string())?;
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .ok_or_else(|| "HTTP response did not include Content-Length".to_string())?;
        let body_start = response.len();
        response.resize(body_start + content_length, 0);
        tls.read_exact(&mut response[body_start..])
            .map_err(|error| error.to_string())?;
        String::from_utf8(response).map_err(|error| error.to_string())
    });

    let response = client.join().expect("HTTPS client thread should not panic");
    if let Err(error) = response {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "public HTTPS request failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let response = response.unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"), "{response}");
    assert!(response.ends_with("secure /ping"), "{response}");
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    assert!(stdout.contains("served\n"), "{stdout}");
    assert!(
        std::net::TcpStream::connect(("127.0.0.1", port)).is_err(),
        "serve_tls_and_close must release the TLS listener after one connection"
    );
}

#[test]
fn bundled_http_server_long_lived_loop_serves_until_cancelled_and_releases_listener() {
    use std::io::Write;

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let source = format!(
        r#"
import {{ HTTP, HTTPRequest, HTTPResponse, HTTPServer, URL }} from std.http;
import {{ stdout }} from std.io;
fn respond(_request: HTTPRequest): HTTPResponse {{ stdout.println("handled"); return HTTPResponse.from_text(200, "ok"); }}
fn serve(endpoint: String): Void {{
    match HTTPServer.bind(endpoint) {{
        Error(_) => stdout.println("bind failed");
        Ok(listener) => {{ _ = HTTPServer.serve_forever(listener, respond); }}
    }}
}}
fn main(): Void {{
    inmut server_job = spawn serve("tcp://127.0.0.1:{port}");
    Timer.sleep(100ms);
    match URL.parse("http://127.0.0.1:{port}/") {{
        Error(_) => stdout.println("URL invalid");
        Ok(url) => {{
            match HTTP.get(url) {{
                Error(_) => stdout.println("first request failed");
                Ok(response) => {{ stdout.println("first " + response.status_code().to_string()); _ = response.close(); }}
            }}
            match HTTP.get(url) {{
                Error(_) => stdout.println("second request failed");
                Ok(response) => {{ stdout.println("second " + response.status_code().to_string()); _ = response.close(); }}
            }}
        }}
    }}
    server_job.cancel();
    stdout.println("server job cancelled");
}}
"#,
        port = address.port(),
    );
    let project = workspace("http_server_pending_accept_cancel");
    let home = workspace("http_server_pending_accept_cancel_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.server.cancel.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\nlisten: {{ addresses: [\"{address}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "handled\nfirst 200\nhandled\nsecond 200\nserver job cancelled\n"
    );
    let rebound = std::net::TcpListener::bind(address)
        .expect("cancelling the long-lived serving task must close its listener");
    drop(rebound);
}

#[test]
fn bundled_http_server_job_cancellation_stops_an_active_cooperative_handler() {
    use std::io::{Read, Write};

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let source = format!(
        r#"
import {{ HTTPMethod, HTTPNext, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
fn slow(_request: HTTPRequest, _params: HTTPRouteParameters): HTTPResponse {{
    Timer.sleep(10000ms);
    return HTTPResponse.from_text(200, "late response");
}}
fn serve(endpoint: String): Void {{
    match HTTPServer.bind(endpoint) {{
        Error(_) => stdout.println("bind failed");
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            _ = router.add_middleware((_request: HTTPRequest, _next: HTTPNext): HTTPResponse => {{
                Timer.sleep(10000ms);
                return HTTPResponse.from_text(200, "late middleware response");
            }});
            match router.route(HTTPMethod.Get, "/slow", slow) {{
                Error(_) => stdout.println("route failed");
                Ok(_) => router.serve(listener, 1000);
            }}
        }}
    }}
}}
fn main(): Void {{
    inmut server_job = spawn serve("tcp://127.0.0.1:{port}");
    Timer.sleep(1000ms);
    server_job.cancel();
    stdout.println("active server job cancelled");
}}
"#,
        port = address.port(),
    );
    let project = workspace("http_server_active_handler_cancel");
    let home = workspace("http_server_active_handler_cancel_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.server.active.cancel.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"{address}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let client = std::thread::spawn(move || -> Result<Vec<u8>, String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut socket = loop {
            match std::net::TcpStream::connect(address) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(error) => {
                    return Err(format!("could not connect to active HTTP server: {error}"));
                }
            }
        };
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .map_err(|error| error.to_string())?;
        socket
            .write_all(b"GET /slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .map_err(|error| error.to_string())?;
        let mut response = Vec::new();
        socket
            .read_to_end(&mut response)
            .map_err(|error| error.to_string())?;
        Ok(response)
    });
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response = client
        .join()
        .expect("active HTTP client thread should not panic")
        .unwrap();
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "active server job cancelled\n"
    );
    assert!(
        response.is_empty() || !response.starts_with(b"HTTP/1.1 200 OK"),
        "cancelling an active handler must not deliver a successful late response: {}",
        String::from_utf8_lossy(&response)
    );
    let rebound = std::net::TcpListener::bind(address)
        .expect("cancelling an active handler must release the accepted listener");
    drop(rebound);
}

#[test]
fn bundled_http2_tls_peer_disconnect_cancels_the_zirk_handler() {
    use std::io::Write;

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"import {{ HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
fn slow(_request: HTTPRequest, _params: HTTPRouteParameters): HTTPResponse {{
    try {{
        Timer.sleep(10000ms);
    }} finally {{
        stdout.println("handler finalized");
    }}
    stdout.println("handler response produced");
    return HTTPResponse.from_text(200, "late response");
}}
fn serve(endpoint: String, certificate: Array<Byte>, key: Array<Byte>): Void {{
    mut chain: Array<Array<Byte>> = [certificate];
    match HTTPServer.bind_tls(endpoint, chain, key) {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            match router.route(HTTPMethod.Get, "/slow", slow) {{
                Error(_) => stdout.println("route failed");
                Ok(_) => match listener.accept() {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(stream) => match router.serve_tls_many(stream, 1) {{
                        Ok(_) => stdout.println("server finished");
                        Error(error) => stdout.println(error.to_string());
                    }};
                }};
            }}
        }}
    }}
}}
fn main(): Void {{
    inmut::strict certificate: Array<Byte> = [{certificate}];
    inmut::strict key: Array<Byte> = [{private_key}];
    inmut server_job = spawn serve("tcp://127.0.0.1:{port}", certificate, key);
    while !server_job.done {{ Timer.sleep(10ms); }}
    server_job.cancel();
}}"#,
        port = address.port(),
        certificate = byte_literal(certificate),
        private_key = byte_literal(private_key),
    );

    let project = workspace("http2_tls_peer_disconnect_cancels_handler");
    let home = workspace("http2_tls_peer_disconnect_cancels_handler_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http2.tls.disconnect.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"{address}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("approve HTTP/2 TLS listener");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled HTTP/2 TLS server");

    let client = std::thread::spawn(move || -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(address) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("could not connect to TLS listener: {error}")),
            }
        };
        let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                root.to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let mut config = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"h2".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        socket
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        runtime.block_on(async move {
            let socket =
                tokio::net::TcpStream::from_std(socket).map_err(|error| error.to_string())?;
            let name =
                tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
                    .map_err(|error| error.to_string())?;
            let tls = connector
                .connect(name, socket)
                .await
                .map_err(|error| error.to_string())?;
            if tls.get_ref().1.alpn_protocol() != Some(b"h2") {
                return Err(format!(
                    "server did not negotiate h2: {:?}",
                    tls.get_ref().1.alpn_protocol()
                ));
            }
            let (mut sender, connection) = hyper::client::conn::http2::handshake(
                hyper_util::rt::TokioExecutor::new(),
                hyper_util::rt::TokioIo::new(tls),
            )
            .await
            .map_err(|error| error.to_string())?;
            let driver = tokio::spawn(connection);
            let request = hyper::Request::builder()
                .method("GET")
                .uri(format!("https://localhost:{}/slow", address.port()))
                .body(http_body_util::Empty::<bytes::Bytes>::new())
                .map_err(|error| error.to_string())?;
            let response = tokio::spawn(async move { sender.send_request(request).await });
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            response.abort();
            driver
                .await
                .map_err(|error| format!("HTTP/2 connection driver: {error}"))?
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    });

    if let Err(error) = client.join().expect("HTTP/2 client must not panic") {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "HTTP/2 disconnect client failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // The runtime may use its full five-second cooperative shutdown grace
    // after the handler has observed cancellation. Keep the process-level
    // assertion outside that bound, with room for native Windows scheduling.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "HTTP/2 server did not finish after peer disconnect; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    assert!(stdout.contains("handler finalized\n"), "{stdout}");
    assert!(!stdout.contains("handler response produced"), "{stdout}");
}

#[test]
fn bundled_https_server_job_cancellation_releases_pending_listener() {
    use std::io::Write;

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let bytes = |value: &[u8]| {
        value
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"
import {{ HTTPClient, HTTPHeaders, HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer, URL }} from std.http;
import {{ stdout }} from std.io;
fn respond(_request: HTTPRequest, _params: HTTPRouteParameters): HTTPResponse {{ stdout.println("handled"); return HTTPResponse.from_text(200, "ok"); }}
fn fetch(url: URL, roots: Array<Array<Byte>>, label: String): Void {{
    match HTTPClient.create_with_trust_roots(roots) {{
        Error(_) => stdout.println(label + " client failed");
        Ok(client) => {{
            mut empty: Array<Byte> = [];
            match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{
                Error(_) => stdout.println(label + " request failed");
                Ok(response) => {{ stdout.println(label + " " + response.status_code().to_string()); _ = response.close(); }}
            }}
            _ = client.close();
        }}
    }}
}}
fn serve(endpoint: String, certificate: Array<Byte>, private_key: Array<Byte>): Void {{
    match HTTPServer.bind_tls(endpoint, [certificate], private_key) {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            _ = router.route(HTTPMethod.Get, "/", respond);
            _ = router.serve_tls_forever(listener);
        }}
    }}
}}
fn main(): Void {{
    inmut server_job = spawn serve(
        "tcp://127.0.0.1:{port}",
        [{certificate}],
        [{private_key}],
    );
    Timer.sleep(100ms);
    match URL.parse("https://127.0.0.1:{port}/") {{
        Error(_) => stdout.println("TLS URL invalid");
        Ok(url) => {{
            mut roots: Array<Array<Byte>> = [[{root}]];
            fetch(url, roots, "first");
            fetch(url, roots, "second");
        }}
    }}
    server_job.cancel();
    stdout.println("TLS server job cancelled");
}}
"#,
        port = address.port(),
        certificate = bytes(certificate),
        private_key = bytes(private_key),
        root = bytes(root),
    );
    let project = workspace("https_server_pending_accept_cancel");
    let home = workspace("https_server_pending_accept_cancel_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"https.server.cancel.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"https://127.0.0.1:{}\"]; during: runtime; }}\nlisten: {{ addresses: [\"{address}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "handled\nfirst 200\nhandled\nsecond 200\nTLS server job cancelled\n"
    );
    let rebound = std::net::TcpListener::bind(address)
        .expect("cancelling TLS serve during accept must close its listener");
    drop(rebound);
}

#[test]
fn bundled_http2_tls_disconnect_cancels_only_one_sibling_handler() {
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);

    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"import {{ TLSListener }} from std.net::tls;
import {{ HTTPError, HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
share class RequestStats {{
    private mut active: Int32;
    private mut maximum: Int32;
    construct() {{ this.active = 0; this.maximum = 0; }}
    enter(): Void {{
        this.active += 1;
        if this.active > this.maximum {{ this.maximum = this.active; }}
    }}
    leave(): Void {{ this.active -= 1; }}
    max_active(): Int32 {{ return this.maximum; }}
}}
fn main(): Void {{
    mut stats: RequestStats = RequestStats();
    mut cert: Array<Byte> = [{certificate}];
    mut key: Array<Byte> = [{private_key}];
    mut chain: Array<Array<Byte>> = [cert];
    match HTTPServer.bind_tls("tcp://127.0.0.1:{port}", chain, key) {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => {{
            stdout.println("ready");
            mut router: HTTPRouter = HTTPRouter();
            match router.route(HTTPMethod.Get, "/ping/:id", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => {{
                stats.enter();
                if request.path() == "/ping/one" {{ Timer.sleep(10000ms); }}
                else {{ Timer.sleep(100ms); }}
                stats.leave();
                return HTTPResponse.from_text(200, "h2 " + request.path());
            }}) {{
                Error(_) => stdout.println("route failed");
                Ok(_) => match listener.accept() {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(stream) => match router.serve_tls_many(stream, 2) {{
                        Ok(_) => {{ stdout.println("served"); stdout.println("max-active " + stats.max_active().to_string()); }}
                        Error(error) => stdout.println(error.to_string());
                    }};
                }};
            }}
        }}
    }}
}}"#,
        certificate = byte_literal(certificate),
        private_key = byte_literal(private_key),
    );

    let project = workspace("approved_std_http2_tls_server");
    let home = workspace("approved_std_http2_tls_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http2.tls.server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP/2 listen permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start compiled Zirk HTTP/2 server");
    let client = std::thread::spawn(move || -> Result<(u16, String), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => return Err(format!("could not connect to HTTP/2 listener: {error}")),
            }
        };
        let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                root.to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let mut config = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"h2".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        socket
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        runtime.block_on(async move {
            let socket =
                tokio::net::TcpStream::from_std(socket).map_err(|error| error.to_string())?;
            let name =
                tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
                    .map_err(|error| error.to_string())?;
            let tls = connector
                .connect(name, socket)
                .await
                .map_err(|error| error.to_string())?;
            if tls.get_ref().1.alpn_protocol() != Some(b"h2") {
                return Err(format!(
                    "server did not negotiate h2: {:?}",
                    tls.get_ref().1.alpn_protocol()
                ));
            }
            let (sender, connection) = hyper::client::conn::http2::handshake(
                hyper_util::rt::TokioExecutor::new(),
                hyper_util::rt::TokioIo::new(tls),
            )
            .await
            .map_err(|error| error.to_string())?;
            let driver = tokio::spawn(connection);
            let make_request = |id: &str| -> Result<_, String> {
                hyper::Request::builder()
                    .method("GET")
                    .uri(format!("https://localhost:{port}/ping/{id}"))
                    .body(http_body_util::Empty::<bytes::Bytes>::new())
                    .map_err(|error| error.to_string())
            };
            let mut sender_one = sender.clone();
            let mut sender_two = sender.clone();
            let first_request = make_request("one")?;
            let second_request = make_request("two")?;
            let first_task =
                tokio::spawn(async move { sender_one.send_request(first_request).await });
            let second_task =
                tokio::spawn(async move { sender_two.send_request(second_request).await });
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            first_task.abort();
            let _ = first_task.await;
            let second = tokio::time::timeout(std::time::Duration::from_secs(5), second_task)
                .await
                .map_err(|error| format!("sibling stream timed out: {error}"))?
                .map_err(|error| format!("second stream task: {error}"))?
                .map_err(|error| format!("send second stream: {error}"))?;
            let status = second.status().as_u16();
            let body = http_body_util::BodyExt::collect(second.into_body())
                .await
                .map_err(|error| format!("collect {status}: {error}"))?
                .to_bytes();
            let result = (
                status,
                String::from_utf8(body.to_vec()).map_err(|error| error.to_string())?,
            );
            driver.abort();
            Ok(result)
        })
    });

    let result = client
        .join()
        .expect("HTTP/2 client thread should not panic");
    if let Err(error) = result {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "public HTTP/2 request failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let response = result.unwrap();
    assert_eq!(response, (200, "h2 /ping/two".to_owned()));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "HTTP/2 server did not finish after sibling completion; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    assert!(
        stdout.contains("served\n"),
        "stdout: {stdout}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("max-active 2\n"),
        "HTTP/2 handlers did not overlap:\n{stdout}"
    );
}

#[test]
fn bundled_http2_websocket_server_accepts_extended_connect_and_echoes_messages() {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, protocol::Role};

    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);
    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let source = format!(
        r#"import {{ HTTPError, HTTPServer, HTTPWebSocket, HTTPWebSocketMessage }} from std.http;
import {{ TLSListener }} from std.net::tls;
import {{ stdout }} from std.io;
fn ignore(): Void {{ return; }}
fn echo(socket: HTTPWebSocket, value: String): Void {{
    _ = socket.send(HTTPWebSocketMessage.Text("h2:" + value));
}}
fn process(socket: HTTPWebSocket, message: HTTPWebSocketMessage): Void {{
    match message {{
        HTTPWebSocketMessage.Text(value) => echo(socket, value);
        _ => ignore();
    }};
}}
fn handle(socket: HTTPWebSocket): Void {{
    match socket.receive() {{
        Ok(message) => process(socket, message);
        Error(_) => ignore();
    }}
    match socket.receive() {{
        Ok(message) => process(socket, message);
        Error(_) => ignore();
    }}
}}
fn main(): Void {{
    mut cert: Array<Byte> = [{certificate}];
    mut key: Array<Byte> = [{private_key}];
    mut chain: Array<Array<Byte>> = [cert];
    mut origins: List<String> = List(); origins.add("https://app.example");
    mut protocols: List<String> = List(); protocols.add("zirk.v2");
    match HTTPServer.bind_tls("tcp://127.0.0.1:{port}", chain, key) {{
        Error(error) => stdout.println(error.to_string());
        Ok(listener) => match listener.accept() {{
            Error(_) => stdout.println("accept failed");
            Ok(stream) => match HTTPServer.serve_tls_websocket_once(stream, "/socket", origins, protocols, 1, handle) {{
                Ok(_) => stdout.println("served");
                Error(error) => stdout.println("server-error:" + error.to_string());
            }};
        }};
    }}
}}"#,
        certificate = byte_literal(certificate),
        private_key = byte_literal(private_key),
    );
    let project = workspace("approved_std_http2_websocket_server");
    let home = workspace("approved_std_http2_websocket_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http2.websocket.server.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{port}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(project.join("src/main.zrk"), source).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut child = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let client = std::thread::spawn(move || -> Result<(u16, String), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let socket = loop {
            match std::net::TcpStream::connect(("127.0.0.1", port)) {
                Ok(socket) => break socket,
                Err(error) if std::time::Instant::now() < deadline => {
                    let _ = error;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                Err(error) => {
                    return Err(format!(
                        "could not connect to HTTP/2 WebSocket listener: {error}"
                    ));
                }
            }
        };
        let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
        let mut roots = tokio_rustls::rustls::RootCertStore::empty();
        roots
            .add(tokio_rustls::rustls::pki_types::CertificateDer::from(
                root.to_vec(),
            ))
            .map_err(|error| error.to_string())?;
        let mut config = tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"h2".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        socket
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        runtime.block_on(async move {
            let socket =
                tokio::net::TcpStream::from_std(socket).map_err(|error| error.to_string())?;
            let name =
                tokio_rustls::rustls::pki_types::ServerName::try_from("localhost".to_owned())
                    .map_err(|error| error.to_string())?;
            let tls = connector
                .connect(name, socket)
                .await
                .map_err(|error| error.to_string())?;
            if tls.get_ref().1.alpn_protocol() != Some(b"h2") {
                return Err(format!(
                    "server did not negotiate h2: {:?}",
                    tls.get_ref().1.alpn_protocol()
                ));
            }
            let (mut sender, connection) = hyper::client::conn::http2::handshake(
                hyper_util::rt::TokioExecutor::new(),
                hyper_util::rt::TokioIo::new(tls),
            )
            .await
            .map_err(|error| error.to_string())?;
            let driver = tokio::spawn(connection);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let mut request = hyper::Request::builder()
                .method(http::Method::CONNECT)
                .uri(format!("https://localhost:{port}/socket"))
                .header("origin", "https://app.example")
                .header("sec-websocket-protocol", "zirk.v2")
                .body(http_body_util::Empty::<bytes::Bytes>::new())
                .map_err(|error| error.to_string())?;
            request
                .extensions_mut()
                .insert(hyper::ext::Protocol::from_static("websocket"));
            let mut response = sender
                .send_request(request)
                .await
                .map_err(|error| format!("send CONNECT: {error}"))?;
            if response.status() != http::StatusCode::OK {
                return Err(format!("unexpected CONNECT status: {}", response.status()));
            }
            if response
                .headers()
                .get("sec-websocket-protocol")
                .and_then(|value| value.to_str().ok())
                != Some("zirk.v2")
            {
                return Err("server did not select the offered subprotocol".to_owned());
            }
            let upgraded = hyper::upgrade::on(&mut response)
                .await
                .map_err(|error| format!("await CONNECT upgrade: {error}"))?;
            let io = hyper_util::rt::TokioIo::new(upgraded);
            let mut websocket =
                tokio_tungstenite::WebSocketStream::from_raw_socket(io, Role::Client, None).await;
            websocket
                .send(Message::Text("hello".into()))
                .await
                .map_err(|error| format!("send WebSocket frame: {error}"))?;
            let message = tokio::time::timeout(std::time::Duration::from_secs(5), websocket.next())
                .await
                .map_err(|_| "WebSocket echo timed out".to_owned())?
                .ok_or_else(|| "WebSocket closed before echo".to_owned())?
                .map_err(|error| format!("receive WebSocket echo: {error}"))?;
            let Message::Text(text) = message else {
                return Err(format!("unexpected WebSocket frame: {message:?}"));
            };
            websocket
                .close(None)
                .await
                .map_err(|error| error.to_string())?;
            let close = tokio::time::timeout(std::time::Duration::from_secs(5), websocket.next())
                .await
                .map_err(|_| "WebSocket close handshake timed out".to_owned())?;
            if !matches!(close, Some(Ok(Message::Close(_)))) {
                return Err(format!(
                    "expected WebSocket close reply, received {close:?}"
                ));
            }
            drop(websocket);
            drop(sender);
            tokio::time::timeout(std::time::Duration::from_secs(5), driver)
                .await
                .map_err(|_| "HTTP/2 connection shutdown timed out".to_owned())?
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            Ok((200, text.to_string()))
        })
    });

    let result = client
        .join()
        .expect("HTTP/2 WebSocket peer should not panic");
    if let Err(error) = result {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!(
            "HTTP/2 WebSocket exchange failed: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(result.unwrap(), (200, "h2:hello".to_owned()));
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "served\n",
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_std_net_tls_client_uses_approved_origin_and_tls_handshake() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("https://127.0.0.1:{}", address.port());
    let server = std::thread::spawn(move || {
        use std::io::Read;
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut record = [0; 5];
        stream.read_exact(&mut record).unwrap();
        assert_eq!(record[0], 0x16, "client must start a TLS handshake");
    });

    let project = workspace("approved_std_net_tls");
    let home = workspace("approved_std_net_tls_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tls.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"{origin}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ TLSReadResult, TLSError, TLSResourceError, TLSStream }} from std.net::tls;\n\
             import {{ stdout }} from std.io;\n\
             fn tls_read(stream: TLSStream): TLSReadResult<Array<Byte>> {{ return stream.read(8); }}\n\
             fn tls_write(stream: TLSStream, bytes: Array<Byte>): Result<Void, TLSError> {{ return stream.write_all(bytes); }}\n\
             fn tls_shutdown(stream: TLSStream): Result<Void, TLSError> {{ return stream.shutdown(); }}\n\
             fn tls_close(stream: TLSStream): Result<Void, TLSResourceError> {{ return stream.close(); }}\n\
             fn main(): Void {{\n\
                 match TLSStream.connect(\"{origin}\") {{\n\
                     Ok(_) => stdout.println(\"unexpected handshake success\");\n\
                     Error(_) => stdout.println(\"TLS handshake rejected\");\n\
                 }};\n\
             }}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TLS permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved std.net TLS client");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "TLS handshake rejected\n"
    );
    server.join().unwrap();
}

#[test]
fn bundled_std_net_tcp_stream_runs_an_approved_loopback_exchange() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4];
        std::io::Read::read_exact(&mut stream, &mut request).unwrap();
        assert_eq!(&request, b"ping");
        std::io::Write::write_all(&mut stream, b"pong").unwrap();
        let mut trailing = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut trailing).unwrap();
        assert!(trailing.is_empty());
    });

    let project = workspace("approved_std_net_tcp");
    let home = workspace("approved_std_net_tcp_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.tcp.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"tcp://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ TCPStream }} from std.net::tcp;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
                 mut payload = [0x70 as UInt8, 0x69 as UInt8, 0x6e as UInt8, 0x67 as UInt8];\n\
                 match TCPStream.connect(\"tcp://127.0.0.1:{}\") with stream {{\n\
                     Ok(stream) => {{\n\
                         match stream.peer_address() {{\n\
                             Ok(peer) => stdout.println(peer);\n\
                             Error(_) => stdout.println(\"peer address failed\");\n\
                         }}\n\
                         match stream.read(0 as UInt64) {{\n\
                             TCPReadResult.Value(bytes) => {{\n\
                                 if bytes.length == 0 {{ stdout.println(\"empty read\"); }}\n\
                                 else {{ stdout.println(\"empty read returned data\"); }}\n\
                             }}\n\
                             TCPReadResult.End => stdout.println(\"empty read treated as EOF\");\n\
                             TCPReadResult.Error(_) => stdout.println(\"empty read failed\");\n\
                         }}\n\
                         mut sent = stream.write_all(payload);\n\
                         _ = sent;\n\
                         match stream.read(4 as UInt64) {{\n\
                             TCPReadResult.Value(_) => stdout.println(\"read\");\n\
                             TCPReadResult.End => stdout.println(\"end\");\n\
                             TCPReadResult.Error(_) => stdout.println(\"read failed\");\n\
                         }}\n\
                     }}\n\
                     Error(_) => stdout.println(\"connect failed\");\n\
                 }}\n\
                 stdout.println(\"finished\");\n\
             }}",
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TCP permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved std.net TCP program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let expected_peer = address.to_string();
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        format!("{expected_peer}\nempty read\nread\nfinished\n")
    );
    server.join().unwrap();
}

#[test]
fn bundled_std_net_udp_socket_runs_an_approved_loopback_exchange() {
    let server = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    server
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let address = server.local_addr().unwrap();
    let server_thread = std::thread::spawn(move || {
        let mut request = [0; 8];
        let (length, sender) = server.recv_from(&mut request).unwrap();
        assert_eq!(&request[..length], b"ping");
        server.send_to(b"pong", sender).unwrap();
    });

    let project = workspace("approved_std_net_udp");
    let home = workspace("approved_std_net_udp_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.udp.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nudp_send: {{ destinations: [\"udp://{address}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ UDPDatagram, UDPSocket }} from std.net::udp;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
             match UDPSocket.connect(\"udp://{address}\") {{\n\
                 Ok(socket) => {{\n\
                     match socket.local_address() {{\n\
                         Ok(local) => stdout.println(local);\n\
                         Error(error) => stdout.println(error.to_string());\n\
                     }}\n\
                     match socket.send([112 as Byte, 105 as Byte, 110 as Byte, 103 as Byte]) {{\n\
                         Ok(count) => {{\n\
                             stdout.println(count.to_string());\n\
                             match socket.receive(8 as UInt64) {{\n\
                                 Ok(datagram) => {{\n\
                                     stdout.println(datagram.peer());\n\
                                     stdout.println(datagram.bytes().length.to_string());\n\
                                 }}\n\
                                 Error(error) => stdout.println(error.to_string());\n\
                             }}\n\
                         }}\n\
                         Error(error) => stdout.println(error.to_string());\n\
                     }}\n\
                     _ = socket.close();\n\
                 }}\n\
                 Error(error) => stdout.println(error.to_string());\n\
             }}\n\
             }}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit UDP permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved std.net UDP program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let output = normalize(&String::from_utf8_lossy(&run.stdout));
    let mut lines = output.lines();
    let local = lines.next().expect("UDP local address");
    assert!(local.starts_with("udp://127.0.0.1:"), "{local}");
    assert!(local.rsplit_once(':').unwrap().1.parse::<u16>().unwrap() > 0);
    assert_eq!(lines.next(), Some("4"));
    assert_eq!(lines.next(), Some(format!("udp://{address}").as_str()));
    assert_eq!(lines.next(), Some("4"));
    assert_eq!(lines.next(), None);
    server_thread.join().unwrap();
}

#[test]
fn bundled_std_net_udp_bound_socket_uses_separate_listen_and_send_grants() {
    use std::io::{BufRead, BufReader, Read};

    let reserved = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let local = reserved.local_addr().unwrap();
    drop(reserved);
    let sender = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    sender
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let sender_address = sender.local_addr().unwrap();

    let project = workspace("approved_std_net_udp_bound");
    let home = workspace("approved_std_net_udp_bound_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.udp.bound.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nudp_send: {{ destinations: [\"udp://{sender_address}\"]; during: runtime; }}\nudp_listen: {{ addresses: [\"{local}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ UDPSocket }} from std.net::udp;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
                 match UDPSocket.bind(\"udp://{local}\") {{\n\
                     Ok(socket) => {{\n\
                         stdout.println(\"ready\");\n\
                         match socket.receive(8 as UInt64) {{\n\
                             Ok(datagram) => {{\n\
                                 stdout.println(datagram.peer());\n\
                                 stdout.println(datagram.bytes().length.to_string());\n\
                                 match socket.join_multicast(\"239.1.2.3\") {{\n\
                                     Ok(_) => stdout.println(\"unexpected multicast membership\");\n\
                                     Error(error) => stdout.println(error.message());\n\
                                 }}\n\
                                 match socket.leave_multicast(\"239.1.2.3\") {{\n\
                                     Ok(_) => stdout.println(\"unexpected multicast leave\");\n\
                                     Error(error) => stdout.println(error.message());\n\
                                 }}\n\
                                 match socket.send_broadcast([112 as Byte], \"udp://255.255.255.255:9000\") {{\n\
                                     Ok(_) => stdout.println(\"unexpected broadcast send\");\n\
                                     Error(error) => stdout.println(error.to_string());\n\
                                 }}\n\
                                 match socket.send_multicast([112 as Byte], \"udp://239.1.2.3:9000\") {{\n\
                                     Ok(_) => stdout.println(\"unexpected multicast send\");\n\
                                     Error(error) => stdout.println(error.to_string());\n\
                                 }}\n\
                                 match socket.send_to([112 as Byte, 111 as Byte, 110 as Byte, 103 as Byte], datagram.peer()) {{\n\
                                     Ok(count) => stdout.println(count.to_string());\n\
                                     Error(error) => stdout.println(error.to_string());\n\
                                 }}\n\
                             }}\n\
                             Error(error) => stdout.println(error.to_string());\n\
                         }}\n\
                         _ = socket.close();\n\
                     }}\n\
                     Error(error) => stdout.println(error.to_string());\n\
                 }}\n\
             }}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit UDP bind and send permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start approved bound UDP program");
    let mut output = BufReader::new(run.stdout.take().unwrap());
    let mut ready = String::new();
    output.read_line(&mut ready).unwrap();
    assert_eq!(normalize(&ready), "ready\n");
    sender.send_to(b"ping", local).unwrap();
    let mut response = [0; 8];
    let (length, peer) = sender.recv_from(&mut response).unwrap();
    assert_eq!(&response[..length], b"pong");
    assert_eq!(peer, local);

    let mut remaining = String::new();
    output.read_to_string(&mut remaining).unwrap();
    let status = run.wait_with_output().unwrap();
    assert!(
        status.status.success(),
        "stdout: {remaining}\nstderr: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(
        normalize(&remaining),
        format!(
            "udp://{sender_address}\n4\nPermissionDenied\nPermissionDenied\nPermissionDenied\nPermissionDenied\n4\n"
        )
    );
}

#[test]
fn bundled_std_net_dns_resolves_only_an_approved_origin() {
    let project = workspace("approved_std_net_dns");
    let home = workspace("approved_std_net_dns_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.net.dns.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {\nnetwork {\nconnect: { origins: [\"tcp://127.0.0.1:80\", \"tcp://localhost:80\"]; during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        "import { DNS, DNSError } from std.net::dns;\n\
         import { stdout } from std.io;\n\
         fn main(): Void {\n\
             match DNS.resolve(\"tcp://127.0.0.1:80\") {\n\
                 Ok(addresses) => { for address in addresses { stdout.println(address); } }\n\
                 Error(_) => stdout.println(\"resolution failed\");\n\
             };\n\
             match DNS.resolve(\"tcp://localhost:80\") {\n\
                 Ok(_) => stdout.println(\"unexpected private result\");\n\
                 Error(_) => stdout.println(\"hostname private addresses rejected\");\n\
             };\n\
         }",
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit DNS permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved std.net DNS program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "127.0.0.1\nhostname private addresses rejected\n"
    );
}

#[test]
fn bundled_std_net_tcp_listener_accepts_only_with_listen_authority() {
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);

    let project = workspace("approved_std_net_listener");
    let home = workspace("approved_std_net_listener_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.listener.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ TCPListener }} from std.net::tcp;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
                 match TCPListener.bind(\"tcp://127.0.0.1:{}\") with listener {{\n\
                     Ok(listener) => {{\n\
                         match listener.accept() with stream {{\n\
                             Ok(stream) => {{\n\
                                 mut payload = [0x70 as UInt8, 0x69 as UInt8, 0x6e as UInt8, 0x67 as UInt8];\n\
                                 mut sent = stream.write_all(payload);\n\
                                 _ = sent;\n\
                                 stdout.println(\"accepted\");\n\
                             }}\n\
                             Error(_) => stdout.println(\"accept failed\");\n\
                         }}\n\
                     }}\n\
                     Error(_) => stdout.println(\"listen failed\");\n\
                 }}\n\
                 stdout.println(\"finished\");\n\
             }}",
            address.port()
        ),
    )
    .unwrap();

    let denied_run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run listener without listen approval");
    assert!(
        denied_run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&denied_run.stdout),
        String::from_utf8_lossy(&denied_run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&denied_run.stdout)),
        "listen failed\nfinished\n"
    );

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit TCP listen approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let address_for_client = address;
    let client = std::thread::spawn(move || {
        let mut stream = None;
        for _ in 0..100 {
            if let Ok(connected) = std::net::TcpStream::connect(address_for_client) {
                stream = Some(connected);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut stream = stream.expect("Zirk TCP listener should accept loopback client");
        let mut request = [0; 4];
        std::io::Read::read_exact(&mut stream, &mut request).unwrap();
        assert_eq!(&request, b"ping");
        let mut response = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut response).unwrap();
        assert!(response.is_empty());
    });

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved std.net listener program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "accepted\nfinished\n"
    );
    client.join().unwrap();
}

#[test]
fn capturing_callable_passed_through_a_function_parameter_executes() {
    let output = crate_of(
        "capturing_callable_forwarding",
        &[(
            "main.zrk",
            "import { stdout } from std.io;\n\
             fn invoke(callback: Fn() => Int32): Int32 { return callback(); }\n\
             fn forward(callback: Fn() => Int32): Int32 { return invoke(callback); }\n\
             fn main(): Void { inmut captured: Int32 = 17; stdout.println(forward((): Int32 => captured)); }",
        )],
    );
    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "17\n");
}

#[test]
fn captured_http_style_handler_survives_list_storage() {
    let output = crate_of(
        "captured_handler_list_storage",
        &[(
            "main.zrk",
            "import { stdout } from std.io;\n\
             share class HandlerBox {\n\
                 private inmut handler: Fn(Int32) => Int32;\n\
                 construct(handler: Fn(Int32) => Int32) { this.handler = handler; }\n\
                 invoke(value: Int32): Int32 { mut callback: Fn(Int32) => Int32 = this.handler; return callback(value); }\n\
             }\n\
             fn main(): Void {\n\
                 inmut route_offset: Int32 = 41;\n\
                 mut handlers: List<HandlerBox> = List();\n\
                 handlers.add(HandlerBox((value: Int32): Int32 => value + route_offset));\n\
                 stdout.println(handlers[0].invoke(1));\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "42\n");
}

#[test]
fn bundled_http_server_sse_stream_emits_configured_heartbeats() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let project = workspace("http_server_sse_heartbeat");
    let home = workspace("http_server_sse_heartbeat_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.sse.heartbeat.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match HTTPResponse.from_sse_stream((index: UInt64): Array<Byte> => {{ mut empty: Array<Byte> = Array(0); return empty; }}, 999) {{
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("invalid heartbeat accepted");
    }}
    match HTTPResponse.from_sse_stream((index: UInt64): Array<Byte> => {{ mut empty: Array<Byte> = Array(0); return empty; }}, 3600001) {{
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("oversized heartbeat accepted");
    }}
    match HTTPServer.bind("tcp://127.0.0.1:{port}") {{
        Error(_) => stdout.println("listen failed");
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            _ = router.route(HTTPMethod.Get, "/events", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => {{
                return match HTTPResponse.from_sse_stream((index: UInt64): Array<Byte> => match index as Int32 {{
                    0 => [100 as Byte, 97 as Byte, 116 as Byte, 97 as Byte, 58 as Byte, 32 as Byte, 102 as Byte, 105 as Byte, 114 as Byte, 115 as Byte, 116 as Byte, 10 as Byte, 10 as Byte];
                    1 => {{ Timer.sleep(1600ms); [100 as Byte, 97 as Byte, 116 as Byte, 97 as Byte, 58 as Byte, 32 as Byte, 115 as Byte, 101 as Byte, 99 as Byte, 111 as Byte, 110 as Byte, 100 as Byte, 10 as Byte, 10 as Byte]; }}
                    _ => {{ mut empty: Array<Byte> = Array(0); empty; }};
                }}, 1000) {{
                    Error(error) => {{ stdout.println(error.to_string()); mut empty: Array<Byte> = Array(0); HTTPResponse.from_bytes(500, empty); }}
                    Ok(response) => response;
                }};
            }});
            _ = router.route(HTTPMethod.Get, "/quiet", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => {{
                return match HTTPResponse.from_sse_stream((index: UInt64): Array<Byte> => match index as Int32 {{
                    0 => [100 as Byte, 97 as Byte, 116 as Byte, 97 as Byte, 58 as Byte, 32 as Byte, 113 as Byte, 117 as Byte, 105 as Byte, 101 as Byte, 116 as Byte, 10 as Byte, 10 as Byte];
                    _ => {{ mut empty: Array<Byte> = Array(0); empty; }};
                }}) {{
                    Error(_) => {{ mut empty: Array<Byte> = Array(0); HTTPResponse.from_bytes(500, empty); }}
                    Ok(response) => response;
                }};
            }});
            _ = router.serve_and_close(listener, 2);
        }}
    }}
}}"#,
            port = address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("approve SSE listen access");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let mut server = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start SSE server");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut client = loop {
        match TcpStream::connect(address) {
            Ok(stream) => break stream,
            Err(error) if Instant::now() < deadline => {
                if server.try_wait().unwrap().is_some() {
                    let output = server.wait_with_output().unwrap();
                    panic!(
                        "SSE server exited before listening: stdout={} stderr={}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
                let _ = error;
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("SSE server did not start before deadline: {error}"),
        }
    };
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .write_all(b"GET /events HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    let response = String::from_utf8(response).unwrap();
    assert!(
        response.contains("content-type: text/event-stream; charset=utf-8\r\n"),
        "{response:?}"
    );
    assert!(response.contains("data: first\n\n"), "{response:?}");
    assert!(response.contains(": keep-alive\n\n"), "{response:?}");
    assert!(response.contains("data: second\n\n"), "{response:?}");

    let mut quiet = TcpStream::connect(address).unwrap();
    quiet
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    quiet
        .write_all(b"GET /quiet HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut quiet_response = Vec::new();
    quiet.read_to_end(&mut quiet_response).unwrap();
    let quiet_response = String::from_utf8(quiet_response).unwrap();
    assert!(
        quiet_response.contains("data: quiet\n\n"),
        "{quiet_response:?}"
    );
    assert!(
        !quiet_response.contains(": keep-alive\n\n"),
        "omitting heartbeat_ms must disable heartbeats: {quiet_response:?}"
    );

    let output = server.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        normalize(&String::from_utf8_lossy(&output.stdout))
            .matches("Protocol\n")
            .count()
            == 2,
        "invalid intervals must be rejected before registering a producer: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn bundled_http_server_invokes_handler_on_accepted_stream() {
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let project = workspace("http_server_callback_project");
    let home = workspace("http_server_callback_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.server.callback.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ HTTPFormData, HTTPHeaders, HTTPMethod, HTTPNext, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;\n\
             import {{ TCPListener }} from std.net::tcp;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
             inmut captured_status: Int32 = 201;\n\
                 match HTTPServer.bind(\"tcp://127.0.0.1:{}\") {{\n\
                     Ok(listener) => {{\n\
                         mut router: HTTPRouter = HTTPRouter();\n\
                         _ = router.add_middleware((request: HTTPRequest, next: HTTPNext): HTTPResponse => {{ if request.path() == \"/captured/41\" {{ stdout.println(\"outer-before\"); }} mut response: HTTPResponse = next.proceed(request); if request.path() == \"/captured/41\" {{ mut ignored: HTTPResponse = next.proceed(request); stdout.println(\"outer-after\"); }} return response; }});\n\
                         _ = router.add_middleware((request: HTTPRequest, next: HTTPNext): HTTPResponse => {{ if request.path() == \"/captured/41\" {{ stdout.println(\"inner-before\"); }} mut response: HTTPResponse = next.proceed(request); if request.path() == \"/captured/41\" {{ stdout.println(\"inner-after\"); }} return response; }});\n\
                         match router.route(HTTPMethod.Get, \"/captured/:id\", \n\
                         (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => {{\n\
                             stdout.println(request.target());\n\
                             stdout.println(params.value(0));\n\
                             stdout.println(params.get(\"id\") ?? \"missing\");\n\
                             stdout.println(params.get(\"unknown\") ?? \"absent\");\n\
                             match request.header_values(\"x-tag\") {{ Ok(values) => {{ stdout.println(values.length); stdout.println(values[0]); stdout.println(values[1]); }} Error(_) => stdout.println(\"header lookup failed\"); }}\n\
             mut response_body: Array<Byte> = [0x63 as Byte, 0x61 as Byte, 0x70 as Byte];\n\
             mut response_headers = HTTPHeaders.create();\n\
             _ = response_headers.add(\"x-out\", \"first\");\n\
             _ = response_headers.add(\"X-Out\", \"second\");\n\
             if params.value(0) == \"41\" {{\n\
                 mut stream_headers = HTTPHeaders.create();\n\
                 _ = stream_headers.add(\"content-type\", \"text/event-stream; charset=utf-8\");\n\
                 _ = stream_headers.add(\"x-out\", \"first\");\n\
                 _ = stream_headers.add(\"X-Out\", \"second\");\n\
                 return match HTTPResponse.from_stream(captured_status, stream_headers, (index: UInt64): Array<Byte> => match index as Int32 {{
\
                     0 => if captured_status == 201 {{ [100 as Byte, 97 as Byte, 116 as Byte, 97 as Byte, 58 as Byte, 32 as Byte, 102 as Byte, 105 as Byte, 114 as Byte, 115 as Byte, 116 as Byte, 10 as Byte, 10 as Byte]; }} else {{ [0 as Byte]; }};
\
                     1 => [100 as Byte, 97 as Byte, 116 as Byte, 97 as Byte, 58 as Byte, 32 as Byte, 115 as Byte, 101 as Byte, 99 as Byte, 111 as Byte, 110 as Byte, 100 as Byte, 10 as Byte, 10 as Byte];
\
                     _ => {{ mut empty: Array<Byte> = Array(0); empty; }};
\
                 }}) {{\n\
                     Ok(response) => {{ stdout.println(response.status_code()); response; }}\n\
                     Error(_) => {{ mut failure_body: Array<Byte> = Array(0); HTTPResponse.from_bytes(500, failure_body); }}\n\
                 }};\n\
             }}\n\
             return match HTTPResponse.try_with_headers(captured_status, response_body, response_headers) {{\n\
                 Ok(response) => {{ stdout.println(response.status_code()); response; }}\n\
                 Error(_) => {{ mut failure_body: Array<Byte> = Array(0); HTTPResponse.from_bytes(500, failure_body); }}\n\
             }};\n\
                             }}) {{ Ok(_) => match router.route(HTTPMethod.Post, \"/captured/:id\", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => match HTTPFormData.parse(request) {{ Ok(form) => {{ stdout.println(form.get(\"q\") ?? \"missing q\"); stdout.println(form.get_all(\"tag\").length); HTTPResponse.from_bytes(202, [0x6f as Byte]); }} Error(error) => {{ stdout.println(error.to_string()); mut empty_form_body: Array<Byte> = Array(0); HTTPResponse.from_bytes(400, empty_form_body); }} }}) {{ Ok(_) => match router.serve_and_close(listener, 6) {{ Ok(count) => stdout.println(count); Error(error) => stdout.println(error.to_string()); }}; Error(error) => stdout.println(error.to_string()); }}; Error(error) => stdout.println(error.to_string()); }}\n\
                     }}\n\
                     Error(_) => stdout.println(\"listen failed\");\n\
                 }}\n\
             }}",
            address.port()
        ),
    )
    .unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP listen approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let client_address = address;
    let client = std::thread::spawn(move || {
        let mut stream = None;
        for _ in 0..100 {
            if let Ok(connected) = std::net::TcpStream::connect(client_address) {
                stream = Some(connected);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut stream = stream.expect("Zirk HTTP server should accept loopback client");
        use std::io::{Read, Write};
        stream
            .write_all(b"GET /missing HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut missing_response = Vec::new();
        stream.read_to_end(&mut missing_response).unwrap();
        drop(stream);
        let mut wrong_method = std::net::TcpStream::connect(client_address).unwrap();
        wrong_method
            .write_all(b"PUT /captured/40 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut wrong_method_response = Vec::new();
        wrong_method
            .read_to_end(&mut wrong_method_response)
            .unwrap();
        drop(wrong_method);
        let mut first = std::net::TcpStream::connect(client_address).unwrap();
        first.write_all(b"GET /captured/41 HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Tag: first\r\nx-tag: second\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = Vec::new();
        first.read_to_end(&mut response).unwrap();
        drop(first);
        let mut second = std::net::TcpStream::connect(client_address).unwrap();
        second.write_all(b"POST /captured/42 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded; charset=utf-8\r\nContent-Length: 25\r\nConnection: close\r\n\r\nq=hello+world&tag=a&tag=b").unwrap();
        let mut second_response = Vec::new();
        second.read_to_end(&mut second_response).unwrap();
        drop(second);
        let mut malformed = std::net::TcpStream::connect(client_address).unwrap();
        malformed.write_all(b"POST /captured/43 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: 5\r\nConnection: close\r\n\r\nq=%GG").unwrap();
        let mut malformed_response = Vec::new();
        malformed.read_to_end(&mut malformed_response).unwrap();
        drop(malformed);
        let many_fields = (0..257)
            .map(|index| format!("x={index}"))
            .collect::<Vec<_>>()
            .join("&");
        let mut excessive = std::net::TcpStream::connect(client_address).unwrap();
        write!(excessive, "POST /captured/44 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", many_fields.len(), many_fields).unwrap();
        let mut excessive_response = Vec::new();
        excessive.read_to_end(&mut excessive_response).unwrap();
        let mut all_responses = missing_response;
        all_responses.extend_from_slice(&wrong_method_response);
        all_responses.extend_from_slice(&response);
        all_responses.extend_from_slice(&second_response);
        all_responses.extend_from_slice(&malformed_response);
        all_responses.extend_from_slice(&excessive_response);
        all_responses
    });
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HTTP server program");
    assert!(
        run.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let response = String::from_utf8(client.join().unwrap()).unwrap();
    assert_eq!(
        response.matches("HTTP/1.1 201 Created").count(),
        1,
        "{response}"
    );
    assert_eq!(
        response.matches("HTTP/1.1 404 Not Found").count(),
        1,
        "{response}"
    );
    assert_eq!(
        response.matches("HTTP/1.1 405 Method Not Allowed").count(),
        1,
        "{response}"
    );
    assert_eq!(
        response.matches("HTTP/1.1 400 Bad Request").count(),
        2,
        "{response}"
    );
    assert!(
        response.contains("allow: GET, POST\r\n"),
        "405 response must advertise its accepted method: {response}"
    );
    assert!(response.contains("data: first"), "{response}");
    assert!(response.contains("data: second"), "{response}");
    assert_eq!(
        response.matches("x-out: first\r\n").count(),
        1,
        "{response}"
    );
    assert_eq!(
        response.matches("x-out: second\r\n").count(),
        1,
        "{response}"
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "outer-before\ninner-before\n/captured/41\n41\n41\nabsent\n2\nfirst\nsecond\n201\ninner-after\nouter-after\nhello world\n2\nProtocol\nTooManyFields\n6\n"
    );
    assert!(
        std::net::TcpStream::connect(address).is_err(),
        "serve_and_close must release the bound listener after six requests"
    );
}

#[test]
fn bundled_http_multipart_parser_preserves_binary_parts_and_untrusted_filename_metadata() {
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Duration;

    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let project = workspace("http_multipart_server_project");
    let home = workspace("http_multipart_server_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.multipart.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPError, HTTPMethod, HTTPMultipartData, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer, HTTPStreamChunk }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match HTTPServer.bind("tcp://127.0.0.1:{}") {{
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            match router.route(HTTPMethod.Post, "/upload", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => {{
                return match HTTPMultipartData.parse(request) {{
                    Error(error) => {{ stdout.println(error.to_string()); HTTPResponse.from_text(400, "invalid"); }}
                    Ok(data) => {{
                        stdout.println(data.part_count());
                        match data.part(0) {{
                            null => {{ stdout.println("missing first part"); HTTPResponse.from_text(500, "missing"); }}
                            first => {{
                                stdout.println(first.name());
                                stdout.println(first.filename() ?? "no filename");
                                stdout.println(first.content_type() ?? "no type");
                                mut first_bytes = first.bytes();
                                stdout.println(first_bytes.length);
                                stdout.println(first_bytes[0] as Int32);
                                match data.part(1) {{
                                    null => {{ stdout.println("missing second part"); HTTPResponse.from_text(500, "missing"); }}
                                    second => {{
                                        stdout.println(second.name());
                                        stdout.println(second.filename() ?? "no filename");
                                        stdout.println(second.has_temporary_upload());
                                        match second.read_temporary_upload(0, 1) {{
                                            HTTPStreamChunk.Bytes(chunk) => {{
                                                stdout.println(chunk.length);
                                                stdout.println(chunk[0] as Int32);
                                            }}
                                            HTTPStreamChunk.End => stdout.println("upload EOF");
                                            HTTPStreamChunk.Error(error) => stdout.println(error.to_string());
                                        }}
                                        match second.read_temporary_upload(1, 1) {{
                                            HTTPStreamChunk.Bytes(chunk) => stdout.println(chunk[0] as Int32);
                                            HTTPStreamChunk.End => stdout.println("upload EOF");
                                            HTTPStreamChunk.Error(error) => stdout.println(error.to_string());
                                        }}
                                        match second.read_temporary_upload(0, 65537) {{
                                            HTTPStreamChunk.Bytes(_) => stdout.println("upload limit missing");
                                            HTTPStreamChunk.End => stdout.println("upload limit missing");
                                            HTTPStreamChunk.Error(error) => match error {{ HTTPError.InvalidArgument => stdout.println("invalid upload range"); _ => stdout.println(error.to_string()); }}
                                        }}
                                        mut second_bytes = second.bytes();
                                        stdout.println(second_bytes.length);
                                        stdout.println(second_bytes[1] as Int32);
                                        stdout.println(second.close_temporary_upload());
                                        stdout.println(second.close_temporary_upload());
                                        HTTPResponse.from_text(200, "parsed");
                                    }}
                                }}
                            }}
                        }}
                    }}
                }}
            }}) {{
                Ok(_) => match router.serve(listener, 2) {{ Ok(_) => stdout.println("served"); Error(_) => stdout.println("serve failed"); }};
                Error(_) => stdout.println("route failed");
            }}
        }}
        Error(_) => stdout.println("listen failed");
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit multipart listener approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run multipart server");
    let client_address = address;
    let client = thread::spawn(move || {
        let requests: [&[u8]; 2] = [
            b"--zirk-boundary\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\nA\r\n--zirk-boundary\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"../outside.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n\x00\xff\r\n--zirk-boundary--\r\n",
            b"--zirk-boundary\r\nContent-Disposition: form-data; name=x\r\n\r\npartial",
        ];
        let mut responses = Vec::new();
        for body in requests {
            let mut stream = None;
            // First run compiles the server in this workspace; allow slower
            // native Linux CI hosts enough time before treating startup as a
            // bind failure.
            for _ in 0..500 {
                if let Ok(connected) = std::net::TcpStream::connect(client_address) {
                    stream = Some(connected);
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            let mut stream = stream.expect("multipart server should accept a connection");
            write!(stream, "POST /upload HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: multipart/form-data; boundary=\"zirk-boundary\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
            stream.write_all(body).unwrap();
            let mut response = Vec::new();
            stream.read_to_end(&mut response).unwrap();
            responses.push(String::from_utf8(response).unwrap());
        }
        responses
    });
    let output = run.wait_with_output().expect("wait for multipart server");
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = client.join().unwrap();
    assert!(responses[0].starts_with("HTTP/1.1 200"), "{}", responses[0]);
    assert!(responses[0].ends_with("parsed"), "{}", responses[0]);
    assert!(responses[1].starts_with("HTTP/1.1 400"), "{}", responses[1]);
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "2\ncaption\nno filename\nno type\n1\n65\nupload\n../outside.bin\ntrue\n1\n0\n255\ninvalid upload range\n2\n255\ntrue\ntrue\nProtocol\nserved\n"
    );
}

#[test]
fn bundled_http_server_routes_query_with_content_and_content_type() {
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let project = workspace("http_server_query_project");
    let home = workspace("http_server_query_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.server.query.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlisten: {{ addresses: [\"127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
        r#"import {{ HTTPHeaders, HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match HTTPServer.bind("tcp://127.0.0.1:{}") {{
        Ok(listener) => {{
            mut router: HTTPRouter = HTTPRouter();
            match router.route(HTTPMethod.Query, "/search", (request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse => {{
                stdout.println("query-handler");
                stdout.println(request.body().length);
                mut headers = HTTPHeaders.create();
                return match headers.add("accept-query", "\"application/query+json\"") {{
                    Ok(_) => {{
                        mut body: Array<Byte> = [113 as Byte, 117 as Byte, 101 as Byte, 114 as Byte, 121 as Byte, 45 as Byte, 111 as Byte, 107 as Byte];
                        match HTTPResponse.try_with_headers(200, body, headers) {{
                            Ok(response) => response;
                            Error(_) => HTTPResponse.from_text(500, "response failed");
                        }};
                    }}
                    Error(_) => HTTPResponse.from_text(500, "header failed");
                }};
            }}) {{
                Ok(_) => match router.serve(listener, 5) {{
                    Ok(count) => stdout.println(count);
                    Error(_) => stdout.println("serve failed");
                }};
                Error(_) => stdout.println("query route rejected");
            }}
        }}
        Error(_) => stdout.println("listen failed");
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP QUERY listen approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let client_address = address;
    let client = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let mut stream = None;
        for _ in 0..100 {
            if let Ok(connected) = std::net::TcpStream::connect(client_address) {
                stream = Some(connected);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut stream = stream.expect("Zirk QUERY server should accept loopback client");
        stream
            .write_all(b"QUERY /search HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut empty_invalid_response = Vec::new();
        stream.read_to_end(&mut empty_invalid_response).unwrap();
        drop(stream);
        let mut stream = std::net::TcpStream::connect(client_address)
            .expect("Zirk QUERY server should reject missing media metadata");
        stream
            .write_all(b"QUERY /search HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 7\r\nConnection: close\r\n\r\n{\"q\":1}")
            .unwrap();
        let mut invalid_response = Vec::new();
        stream.read_to_end(&mut invalid_response).unwrap();
        drop(stream);
        let mut stream = std::net::TcpStream::connect(client_address)
            .expect("Zirk QUERY server should reject duplicate media metadata");
        stream
            .write_all(b"QUERY /search HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/query+json\r\nContent-Type: application/query+json\r\nContent-Length: 7\r\nConnection: close\r\n\r\n{\"q\":1}")
            .unwrap();
        let mut duplicate_invalid_response = Vec::new();
        stream.read_to_end(&mut duplicate_invalid_response).unwrap();
        drop(stream);
        let mut stream = std::net::TcpStream::connect(client_address)
            .expect("Zirk QUERY server should reject wildcard media metadata");
        stream
            .write_all(b"QUERY /search HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/*\r\nContent-Length: 7\r\nConnection: close\r\n\r\n{\"q\":1}")
            .unwrap();
        let mut wildcard_invalid_response = Vec::new();
        stream.read_to_end(&mut wildcard_invalid_response).unwrap();
        drop(stream);
        let mut stream = std::net::TcpStream::connect(client_address)
            .expect("Zirk QUERY server should accept the valid request");
        stream
            .write_all(b"QUERY /search HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/query+json\r\nContent-Length: 7\r\nConnection: close\r\n\r\n{\"q\":1}")
            .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        (
            empty_invalid_response,
            invalid_response,
            duplicate_invalid_response,
            wildcard_invalid_response,
            response,
        )
    });
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved HTTP QUERY server program");
    assert!(
        run.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let (empty_invalid, invalid, duplicate_invalid, wildcard_invalid, response) =
        client.join().unwrap();
    let empty_invalid = String::from_utf8(empty_invalid).unwrap();
    assert!(
        empty_invalid.starts_with("HTTP/1.1 400 Bad Request\r\n"),
        "{empty_invalid}"
    );
    let invalid = String::from_utf8(invalid).unwrap();
    assert!(
        invalid.starts_with("HTTP/1.1 400 Bad Request\r\n"),
        "{invalid}"
    );
    let duplicate_invalid = String::from_utf8(duplicate_invalid).unwrap();
    assert!(
        duplicate_invalid.starts_with("HTTP/1.1 400 Bad Request\r\n"),
        "{duplicate_invalid}"
    );
    let wildcard_invalid = String::from_utf8(wildcard_invalid).unwrap();
    assert!(
        wildcard_invalid.starts_with("HTTP/1.1 400 Bad Request\r\n"),
        "{wildcard_invalid}"
    );
    let response = String::from_utf8(response).unwrap();
    assert!(
        response.starts_with("HTTP/1.1 200 OK\r\n"),
        "{response}; Zirk stdout: {}; stderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        response
            .to_ascii_lowercase()
            .contains("accept-query: \"application/query+json\"\r\n"),
        "server must preserve the configured Accept-Query field: {response}"
    );
    assert!(response.ends_with("query-ok"), "{response}");
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "query-handler\n7\n5\n"
    );
}

#[test]
fn bundled_http_router_rejects_duplicate_and_ambiguous_route_registrations() {
    let output = crate_of(
        "http_router_conflicts",
        &[(
            "main.zrk",
            "import { HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter } from std.http;\n\
             import { stdout } from std.io;\n\
             fn handler(request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse { return HTTPResponse.from_bytes(200, [0x6f as Byte]); }\n\
             fn main(): Void {\n\
                 mut router: HTTPRouter = HTTPRouter();\n\
                 match router.route(HTTPMethod.Get, \"/captured/:id\", handler) { Ok(_) => stdout.println(\"first registered\"); Error(_) => stdout.println(\"first rejected\"); }\n\
                 match router.route(HTTPMethod.Get, \"/captured/:id\", handler) { Ok(_) => stdout.println(\"duplicate accepted\"); Error(_) => stdout.println(\"duplicate rejected\"); }\n\
                 match router.route(HTTPMethod.Get, \"/:prefix/captured\", handler) { Ok(_) => stdout.println(\"ambiguous accepted\"); Error(_) => stdout.println(\"ambiguous rejected\"); }\n\
             }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "first registered\nduplicate rejected\nambiguous rejected\n"
    );
}

#[test]
fn standard_input_line_is_read_by_the_native_backend() {
    let output = crate_of_with_input(
        "standard_input_native",
        "import { stdin, stdout } from std.io;\nfn main(): Void { stdout.println(stdin.read_line()); }",
        "hola\n".as_bytes(),
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "hola\n");
}

#[test]
fn standard_error_is_written_to_the_process_error_stream() {
    let output = crate_of(
        "standard_error_native",
        &[(
            "main.zrk",
            "import { stderr } from std.io;\nfn main(): Void { stderr.print(\"warning\"); }",
        )],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(output.stdout, "");
    // CI enables the optional LLVM progress trace to diagnose native backend
    // failures. Keep the assertion focused on the program's stderr payload.
    let program_stderr = output
        .stderr
        .lines()
        .filter(|line| !line.starts_with("[zirk-codegen] "))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(program_stderr, "warning");
}

#[test]
fn an_external_package_reexport_compiles_and_runs_through_the_native_backend() {
    let dir = workspace("external_package_native");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.math ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { stdout } from std.io;\nimport { math } from acme.math;\nfn main(): Void { stdout.println(math.answer()); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.math\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.math;\nshare mod math from \"./math\";",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/math.zrk"),
        "share fn answer(): Int32 { return 42; }",
    )
    .unwrap();
    let manifest = zirk_cli::project::read_manifest(&dir.join("dep/.zkinit")).unwrap();
    let hash = zirk_cli::project::package_integrity_hash(&dir.join("dep"), &manifest).unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        format!("acme.math 1.0 local sha256:{hash} src/package.zrk\n"),
    )
    .unwrap();

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&dir)
        .output()
        .expect("run external package");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(normalize(&String::from_utf8_lossy(&output.stdout)), "42\n");
}

#[test]
fn a_declaration_without_share_is_private_to_its_file() {
    let output = crate_of(
        "modules_private",
        &[
            (
                "main.zrk",
                "import { stdout } from std.io;\n\
                 import { helper } from \"./other\";\n\
                 fn main(): Void { stdout.println(helper()); }\n",
            ),
            ("other.zrk", "fn helper(): String { return \"x\"; }\n"),
        ],
    );

    assert_ne!(output.status, 0, "the private name should not resolve");
    assert!(
        output.stderr.contains("not accessible from this file"),
        "stderr:\n{}",
        output.stderr
    );
}

#[test]
fn two_files_may_import_from_each_other() {
    // Mutual imports are a normal program: nothing in this phase depends on the
    // order files are read. What matters is that each is read once.
    let output = crate_of(
        "modules_mutual",
        &[
            (
                "a.zrk",
                "import { stdout } from std.io;\n\
                 import { b } from \"./b\";\n\
                 share fn a(): String { return b(); }\n\
                 fn main(): Void { stdout.println(a()); }\n",
            ),
            (
                "b.zrk",
                "import { a } from \"./a\";\n\
                 share fn b(): String { return \"desde b\"; }\n",
            ),
        ],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "desde b\n");
}

#[test]
fn a_missing_imported_file_is_reported_at_the_import() {
    let output = crate_of(
        "modules_missing",
        &[(
            "main.zrk",
            "import { thing } from \"./nowhere\";\nfn main(): Void { }\n",
        )],
    );

    assert_ne!(output.status, 0);
    assert!(
        output.stderr.contains("nowhere.zrk"),
        "the diagnostic must name the file it could not read:\n{}",
        output.stderr
    );
}

#[test]
fn two_shared_declarations_cannot_share_a_name() {
    // A crate has one namespace in this phase, so the collision is an error
    // and the diagnostic has to name the other file: a bare line number says
    // nothing when the two declarations live in different ones.
    let output = crate_of(
        "modules_collision",
        &[
            (
                "main.zrk",
                "import { helper } from \"./other\";\n\
                 share fn helper(): String { return \"a\"; }\n\
                 fn main(): Void { stdout.println(helper()); }\n",
            ),
            ("other.zrk", "share fn helper(): String { return \"b\"; }\n"),
        ],
    );

    assert_ne!(output.status, 0);
    assert!(
        output.stderr.contains("already defined") && output.stderr.contains("other.zrk"),
        "the diagnostic must name the other file:\n{}",
        output.stderr
    );
}

#[test]
fn an_import_alias_binds_only_the_alias() {
    // The declaration keeps its own name; the importing file only gets the one
    // it asked for. Otherwise the alias would add a name instead of renaming.
    let output = crate_of(
        "modules_alias",
        &[
            (
                "main.zrk",
                "import { stdout } from std.io;\n\
                 import { Role -> R, describe -> name } from \"./lib\";\n\
                 fn main(): Void { stdout.println(name(R.Guest)); }\n",
            ),
            (
                "lib.zrk",
                "share enum Role { Admin, Guest }\n\
                 share fn describe(r: Role): String {\n\
                 return match r { Role.Admin => \"admin\", Role.Guest => \"guest\" };\n\
                 }\n",
            ),
        ],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "guest\n");
}

#[test]
fn the_original_name_is_not_available_under_an_alias() {
    let output = crate_of(
        "modules_alias_hides",
        &[
            (
                "main.zrk",
                "import { stdout } from std.io;\n\
                 import { valor -> v } from \"./lib\";\n\
                 fn main(): Void { stdout.println(valor()); }\n",
            ),
            ("lib.zrk", "share fn valor(): Int32 { return 8; }\n"),
        ],
    );

    assert_ne!(output.status, 0, "the original name must not resolve");
    assert!(
        output.stderr.contains("not imported"),
        "stderr:\n{}",
        output.stderr
    );
}

#[test]
fn a_shared_declaration_still_has_to_be_imported() {
    // Being `share` publishes a declaration; it does not put it in scope
    // everywhere. Otherwise `import` would be decoration.
    let output = crate_of(
        "modules_needs_import",
        &[
            (
                "main.zrk",
                "import { stdout } from std.io;\n\
                 import { one } from \"./lib\";\n\
                 fn main(): Void { stdout.println(one() + two()); }\n",
            ),
            (
                "lib.zrk",
                "share fn one(): Int32 { return 1; }\n\
                 share fn two(): Int32 { return 2; }\n",
            ),
        ],
    );

    assert_ne!(output.status, 0);
    assert!(
        output.stderr.contains("not imported"),
        "stderr:\n{}",
        output.stderr
    );
}

#[test]
fn a_private_enum_is_not_reachable_from_another_file() {
    let output = crate_of(
        "modules_private_enum",
        &[
            (
                "main.zrk",
                "import { stdout } from std.io;\n\
                 import { E } from \"./lib\";\n\
                 fn main(): Void { stdout.println(match E.A { _ => \"x\" }); }\n",
            ),
            ("lib.zrk", "enum E { A }\n"),
        ],
    );

    assert_ne!(output.status, 0);
    assert!(
        output.stderr.contains("not accessible from this file"),
        "stderr:\n{}",
        output.stderr
    );
}

// --- Collector soundness (`fase-4e-colector-mark-sweep`, section 4) --------
//
// These are the load-bearing tests of that change: real `.zrk` programs,
// compiled and run as actual processes, against the real mark-sweep
// collector — not the runtime's own synthetic-object unit tests
// (`crates/zirk-runtime/src/collector.rs`), which prove the same properties
// at the mark/sweep level directly but never touch codegen's own shadow-stack
// instrumentation (design D2/D4/D5). `ZIRK_GC_THRESHOLD` (bytes) forces
// collections to happen deterministically at a specific point, rather than
// hoping the default 1 MiB threshold happens to be crossed.

/// Like [`zirk`], but with extra environment variables set on the child
/// process and the source given inline rather than copied from the corpus —
/// these fixtures exist only to control `ZIRK_GC_THRESHOLD`, so they do not
/// belong alongside the generic corpus (whose runner has no per-file way to
/// set one).
fn zirk_with_env(source_text: &str, name: &str, env: &[(&str, &str)]) -> Output {
    let dir = workspace(name);
    let file = dir.join("main.zrk");
    std::fs::write(&file, source_text).expect("write the source fixture");

    let mut command = Command::new(compiler());
    command
        .arg("run")
        .arg(file.file_name().expect("file name"))
        .current_dir(&dir);
    for (key, value) in env {
        command.env(key, value);
    }

    let output = command.output().expect("run the compiler");
    Output {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Design D4's own motivating hazard, reproduced end to end: `show(Marker(111),
/// Marker(222))` evaluates its two managed-reference arguments left to
/// right. With the collection threshold forced down to 1 byte, allocating
/// `Marker(222)` cannot help but cross it, triggering a real mark-sweep
/// collection while `Marker(111)`'s result lives *only* in whatever D4's
/// synthetic slot spilled it to (it has not reached a named local, and the
/// `show` call that would consume it has not happened yet). If that spill,
/// or the zero-init that keeps the shadow stack from reading garbage in the
/// slot before it (D5), were missing or wrong, this either prints the wrong
/// id, prints garbage, or the process crashes outright — not a subtle
/// off-by-one, a use-after-free.
#[test]
fn d4_hazard_the_first_constructor_argument_survives_a_collection_triggered_by_the_second() {
    let source = "import { stdout } from std.io;\nclass Marker {\n\
        id: Int32;\n\
        construct(id: Int32) { this.id = id; }\n\
    }\n\n\
    fn show(a: Marker, b: Marker): Void {\n\
        stdout.println(a.id);\n\
        stdout.println(b.id);\n\
    }\n\n\
    fn main(): Void {\n\
        show(Marker(111), Marker(222));\n\
    }";

    let output = zirk_with_env(source, "gc_d4_hazard", &[("ZIRK_GC_THRESHOLD", "1")]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "111\n222\n",
        "the first argument's object must survive intact, not be collected \n\
         out from under the second argument's own construction"
    );
}

/// A sanity control for the test above: the same program, with the default
/// (generous) threshold, so a regression that happens to depend on the
/// *particular* low threshold used above (rather than on D4 itself) would
/// still be caught by comparison.
#[test]
fn d4_hazard_fixture_is_correct_without_a_forced_collection_too() {
    let source = "import { stdout } from std.io;\nclass Marker {\n\
        id: Int32;\n\
        construct(id: Int32) { this.id = id; }\n\
    }\n\n\
    fn show(a: Marker, b: Marker): Void {\n\
        stdout.println(a.id);\n\
        stdout.println(b.id);\n\
    }\n\n\
    fn main(): Void {\n\
        show(Marker(111), Marker(222));\n\
    }";

    let output = zirk_with_env(source, "gc_d4_hazard_control", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "111\n222\n");
}

/// `docs/decisions/ADR-003-investigacion-fase-4.md`'s probe 1 cycle (two
/// `Node`s referencing each other through a nullable field), discarded
/// immediately, repeated under sustained allocation pressure with a small
/// forced threshold — mirroring probe 5's own loop shape, scaled down to a
/// bound this suite can run quickly.
///
/// What this proves directly: the collector runs repeatedly against a
/// program that is *entirely* built from unreachable cycles (nothing else
/// this program allocates is ever kept) without crashing, hanging, or
/// corrupting the run — real evidence, not merely that the process exits
/// zero, since a cycle is exactly the shape a naive "collect only what has
/// zero incoming references" scheme would leak forever. What it does *not*
/// prove by itself — this suite has no portable way to observe another
/// process' RSS — is the exact byte count reclaimed; that half is covered
/// directly, at the byte level, by `crates/zirk-runtime/src/collector.rs`'s
/// own `an_unreachable_cycle_is_fully_reclaimed` unit test, which asserts
/// live-byte accounting drops to exactly zero for this identical shape.
#[test]
fn a_sustained_loop_of_discarded_reference_cycles_completes_under_a_small_threshold() {
    let source = "import { stdout } from std.io;\nclass Node {\n\
        mut next: Node?;\n\
        construct() { }\n\
    }\n\n\
    fn churn(n: Int32): Void {\n\
        mut i = 0;\n\
        while i < n {\n\
            mut a = Node();\n\
            mut b = Node();\n\
            a.next = b;\n\
            b.next = a;\n\
            i = i + 1;\n\
        }\n\
    }\n\n\
    fn main(): Void {\n\
        churn(300000);\n\
        stdout.println(\"done\");\n\
    }";

    let output = zirk_with_env(source, "gc_cycle_churn", &[("ZIRK_GC_THRESHOLD", "8192")]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "done\n");
}

/// `String` values built and discarded in a tight loop, with the GC threshold
/// forced down to a small value, must complete without leaking or corrupting
/// — evidence that the new string descriptor makes string allocations ordinary
/// GC objects.
#[test]
fn a_sustained_loop_of_discarded_strings_completes_under_a_small_threshold() {
    let source = "import { stdout } from std.io;\nfn churn(n: Int32): Void {\n\
        mut i = 0;\n\
        while i < n {\n\
            mut s = \"x\" + i.to_string();\n\
            i = i + 1;\n\
        }\n\
    }\n\
    \n\
    fn main(): Void {\n\
        churn(50000);\n\
        stdout.println(\"done\");\n\
    }";

    let output = zirk_with_env(source, "gc_string_churn", &[("ZIRK_GC_THRESHOLD", "8192")]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "done\n");
}

/// A regression test for a real bug found while implementing
/// `parallel-cpu-regions`: `crates/zirk-codegen-llvm/src/emit.rs`'s
/// `gc_reference_paths` was missing match arms for `Array<T>`/`List<T>`/
/// `Range`/`Map<K,V>`/`Set<T>` — those types are correctly flagged by
/// `IrType::is_managed_reference` (so a slot of one of them gets zero-
/// initialized at function entry), but the table that turns a gc-root slot
/// into an actual root *address* silently produced zero entries for them, so
/// such a local variable was invisible to every collection. `List<T>`
/// exposed it easily: each `.add()` can trigger its own buffer-growth
/// allocation, and if a collection lands during one of those while nothing
/// else roots the list, the collector can reclaim the list itself (or the
/// buffer it referenced) while it is still very much in use — silently
/// wrong results (elements read back as zero) or a crash, depending on
/// exact memory reuse. `Array<T>` (built in one allocation, no further
/// allocation while filling it) happened not to expose it in ordinary use.
/// This grows a `List<Int32>` one element at a time under a small forced
/// threshold — this exact shape crashed or printed the wrong sum before the
/// fix (verified by re-reverting the fix locally and re-running this test).
#[test]
fn a_list_survives_repeated_buffer_growth_under_a_small_threshold() {
    let source = "import { stdout } from std.io;\nfn main(): Void {\n\
        mut values: List<Int32> = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];\n\
        mut total: Int32 = 0;\n\
        for v in values {\n\
            total = total + v * v;\n\
        }\n\
        stdout.println(total);\n\
    }";

    let output = zirk_with_env(source, "gc_list_growth", &[("ZIRK_GC_THRESHOLD", "200")]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "385\n");
}

/// An object reachable only from a still-active *outer* frame (`main`'s own
/// `kept`) survives collections triggered by unrelated allocation happening
/// entirely inside an *inner*, unrelated call (`churn`) — design D2's own
/// claim that the shadow stack is a stack of every active frame, not just
/// the innermost one.
#[test]
fn a_still_active_outer_frames_root_survives_unrelated_allocation_in_an_inner_call() {
    let source = "import { stdout } from std.io;\nclass Holder {\n\
        value: Int32;\n\
        construct(value: Int32) { this.value = value; }\n\
    }\n\n\
    fn churn(n: Int32): Void {\n\
        mut i = 0;\n\
        while i < n {\n\
            mut throwaway = Holder(0);\n\
            i = i + 1;\n\
        }\n\
    }\n\n\
    fn main(): Void {\n\
        mut kept = Holder(999);\n\
        churn(50000);\n\
        stdout.println(kept.value);\n\
    }";

    let output = zirk_with_env(
        source,
        "gc_outer_frame_survives",
        &[("ZIRK_GC_THRESHOLD", "4096")],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "999\n",
        "`kept` must survive every collection `churn` triggers, since main's \n\
         own frame — where `kept` is rooted — is still active the entire time"
    );
}

/// An object local to `use_once` becomes unreachable the moment its frame
/// pops (`return`, design D2's `zirk_rt_pop_frame`) — called 200,000 times
/// under a small forced threshold, so the collector must repeatedly notice
/// and reclaim each call's own now-dead `Scratch` while correctly leaving
/// every *other* call's still-in-progress state alone. Same evidence shape
/// and same honest limit as the cycle-churn test above: this proves the
/// program runs correctly to completion under that pressure, not the exact
/// byte count reclaimed — that half is `crates/zirk-runtime/src/collector.rs`'s
/// own `an_object_reachable_only_through_a_pushed_frame_survives` unit test,
/// which directly asserts an object becomes unreachable (and is reclaimed on
/// the next collection) the moment its frame pops.
#[test]
fn an_object_unreachable_after_its_frame_pops_does_not_derail_a_sustained_call_loop() {
    let source = "import { stdout } from std.io;\nclass Scratch {\n\
        value: Int32;\n\
        construct(value: Int32) { this.value = value; }\n\
    }\n\n\
    fn use_once(v: Int32): Int32 {\n\
        mut s = Scratch(v);\n\
        return s.value;\n\
    }\n\n\
    fn main(): Void {\n\
        mut i = 0;\n\
        mut total = 0;\n\
        while i < 200000 {\n\
            total = total + use_once(1);\n\
            i = i + 1;\n\
        }\n\
        stdout.println(total);\n\
    }";

    let output = zirk_with_env(
        source,
        "gc_frame_pop_reclaims",
        &[("ZIRK_GC_THRESHOLD", "4096")],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "200000\n");
}

// --- Weak<T> soundness (`fase-4e-weak`, section 5) --------------------------
//
// Same standard as the collector soundness tests above: real `.zrk` programs,
// compiled and run as actual processes, so the weak-clearing pass (design D3)
// is proven end to end through real codegen — not just
// `crates/zirk-runtime/src/collector.rs`'s own synthetic-object unit tests,
// which prove the same properties at the mark/sweep level directly but never
// touch codegen's own `WeakFrom`/`WeakUpgrade`/`WeakIsAlive` lowering.

/// `Weak.from(m)` while `m` is still reachable through its own local:
/// `.upgrade()` returns it and `.is_alive` is `true` (spec scenario "Upgrade
/// while the referent is alive").
#[test]
fn weak_upgrade_and_is_alive_see_a_still_reachable_referent() {
    let source = "import { stdout } from std.io;\nclass Marker { id: Int32; construct(id: Int32) { this.id = id; } }\n\n\
    fn main(): Void {\n\
        mut m: Marker = Marker(42);\n\
        mut w: Weak<Marker> = Weak.from(m);\n\
        stdout.println(w.is_alive);\n\
        match w.upgrade() {\n\
            null => stdout.println(\"gone\");\n\
            found => stdout.println(found.id);\n\
        }\n\
    }";

    let output = zirk_with_env(source, "weak_upgrade_alive", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "true\n42\n");
}

/// The only strong reference to `Marker(7)` is `m`, local to `make`; once
/// `make` returns, nothing but the `Weak<Marker>` handle `main` kept still
/// points at it. A collection forced by a tiny `ZIRK_GC_THRESHOLD` runs while
/// evaluating `stdout.println(0)` — allocating the `String`/formatting work
/// underneath `println` is enough to cross it — after which `.upgrade()`
/// must return `null` and `.is_alive` must be `false` (spec scenario "Weak
/// referent was reclaimed"). This is the load-bearing test of this change:
/// if the weak-clearing pass (design D3) did not really run between mark and
/// sweep, `.upgrade()` would either dereference freed memory or return a
/// stale, dangling pointer instead of `null`.
#[test]
fn weak_upgrade_and_is_alive_see_a_collected_referent_as_gone() {
    let source = "import { stdout } from std.io;\nclass Marker { id: Int32; construct(id: Int32) { this.id = id; } }\n\n\
    fn make(): Weak<Marker> {\n\
        mut m: Marker = Marker(7);\n\
        return Weak.from(m);\n\
    }\n\n\
    fn main(): Void {\n\
        mut w: Weak<Marker> = make();\n\
        // `println`ing an `Int32` never allocates through the collector\n\
        // (`String` conversion is its own allocation path, outside\n\
        // `zirk_rt_alloc`'s reach) — an ordinary object construction is\n\
        // what actually crosses `ZIRK_GC_THRESHOLD` and forces a real\n\
        // mark-sweep collection.\n\
        mut trigger: Marker = Marker(0);\n\
        stdout.println(trigger.id);\n\
        stdout.println(w.is_alive);\n\
        match w.upgrade() {\n\
            null => stdout.println(\"gone\");\n\
            found => stdout.println(found.id);\n\
        }\n\
    }";

    let output = zirk_with_env(
        source,
        "weak_upgrade_collected",
        &[("ZIRK_GC_THRESHOLD", "1")],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "0\nfalse\ngone\n",
        "once the only strong reference is gone and a collection has run, \n\
         `.upgrade()` must return null and `.is_alive` must be false — the \n\
         weak-clearing pass must have actually run end to end"
    );
}

/// A `Weak<T>` handle does not keep its referent alive by itself (spec: "SHALL
/// NOT keep its referent alive"). `w` stays reachable the entire time (it is
/// `main`'s own local), but `Marker(9)`'s only *strong* reference is `make`'s
/// own local `m`, gone the moment `make` returns. If a `Weak<T>` handle were
/// mistakenly treated as a strong root (the very bug design D2's sentinel
/// descriptor exists to prevent), `.upgrade()` would keep returning the
/// object forever, since `w` never goes out of scope in `main`.
#[test]
fn a_reachable_weak_handle_does_not_keep_its_referent_alive() {
    let source = "import { stdout } from std.io;\nclass Marker { id: Int32; construct(id: Int32) { this.id = id; } }\n\n\
    fn make(): Weak<Marker> {\n\
        mut m: Marker = Marker(9);\n\
        return Weak.from(m);\n\
    }\n\n\
    fn main(): Void {\n\
        mut w: Weak<Marker> = make();\n\
        // See `weak_upgrade_and_is_alive_see_a_collected_referent_as_gone`'s\n\
        // own comment: an object construction, not a `println`, is what\n\
        // actually crosses `ZIRK_GC_THRESHOLD`.\n\
        mut trigger: Marker = Marker(0);\n\
        stdout.println(trigger.id);\n\
        mut stillAlive: Boolean = w.is_alive;\n\
        stdout.println(stillAlive);\n\
    }";

    let output = zirk_with_env(
        source,
        "weak_handle_does_not_root_its_referent",
        &[("ZIRK_GC_THRESHOLD", "1")],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "0\nfalse\n",
        "the Weak<Marker> handle `w` is still reachable throughout, but its \n\
         referent must be collected anyway once nothing strong reaches it"
    );
}

// --- Clone soundness (`fase-4e-clone`, section 6) ---------------------------
//
// Same standard as the collector/Weak<T> soundness tests above: real `.zrk`
// programs, compiled and run as actual processes, so `zirk_rt_clone`'s
// sharing/cycle/collector-safety guarantees (design D2/D3) are proven end to
// end through real codegen, not just `crates/zirk-runtime/src/clone.rs`'s
// own synthetic-object unit tests.

/// A class with a couple of scalar fields: the clone has independent
/// identity (`is` false against the source) and equal field values.
#[test]
fn clone_of_a_simple_object_has_independent_identity_and_equal_fields() {
    let source = "import { stdout } from std.io;\nclass Point {\n\
        mut x: Int32;\n\
        mut y: Int32;\n\
        construct(x: Int32, y: Int32) { this.x = x; this.y = y; }\n\
    }\n\n\
    fn main(): Void {\n\
        mut a: Point = Point(1, 2);\n\
        mut b: Point = a.clone();\n\
        stdout.println(a is b);\n\
        stdout.println(b.x);\n\
        stdout.println(b.y);\n\
    }";

    let output = zirk_with_env(source, "clone_simple_object", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "false\n1\n2\n");
}

/// The spec's own named invariant (`zirk-memory-safety`'s "Deep clone
/// contract", scenario "Graph contains shared child"): `a.left is a.right`
/// before cloning (both fields alias the same child); after `mut b =
/// a.clone()`, `b.left is b.right` must be `true` (the shared child clones
/// to *one* new shared clone, not two independent copies) and `b.left is
/// a.left` must be `false` (the clone never retains an alias to the source).
/// The single most important test in this change.
#[test]
fn clone_preserves_internal_sharing_of_a_shared_child() {
    let source = "import { stdout } from std.io;\nclass Child {\n\
        mut value: Int32;\n\
        construct(value: Int32) { this.value = value; }\n\
    }\n\n\
    class Parent {\n\
        mut left: Child;\n\
        mut right: Child;\n\
        construct(left: Child, right: Child) { this.left = left; this.right = right; }\n\
    }\n\n\
    fn main(): Void {\n\
        mut c: Child = Child(1);\n\
        mut a: Parent = Parent(c, c);\n\
        stdout.println(a.left is a.right);\n\n\
        mut b: Parent = a.clone();\n\
        stdout.println(b.left is b.right);\n\
        stdout.println(b.left is a.left);\n\
    }";

    let output = zirk_with_env(source, "clone_shared_child", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "true\ntrue\nfalse\n",
        "b.left is b.right must be true (one shared clone) and b.left is \n\
         a.left must be false (never an alias back into the source)"
    );
}

/// A cyclic graph (a node whose field points back to itself) clones without
/// stack overflow or an infinite loop, and the cloned cycle points within
/// the new graph, never back into the source graph (spec scenario "Graph
/// contains a cycle").
#[test]
fn clone_of_a_self_referential_cycle_terminates_within_the_new_graph() {
    let source = "import { stdout } from std.io;\nclass Node {\n\
        mut next: Node?;\n\
        construct() { }\n\
    }\n\n\
    fn main(): Void {\n\
        mut a: Node = Node();\n\
        a.next = a;\n\
        stdout.println(a.next is a);\n\n\
        mut b: Node = a.clone();\n\
        match b.next {\n\
            null => stdout.println(\"null\");\n\
            n => {\n\
                stdout.println(n is b);\n\
                stdout.println(n is a);\n\
            }\n\
        }\n\
    }";

    let output = zirk_with_env(source, "clone_cycle", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "true\ntrue\nfalse\n",
        "the cloned cycle must point within the new graph (n is b) and \n\
         never back into the source (n is a must be false)"
    );
}

/// Mutating the clone does not affect the source, and vice versa (the
/// "never retains an alias to a cloned mutable source node" guarantee,
/// observed concretely rather than only through identity comparisons).
#[test]
fn mutating_a_clone_does_not_affect_the_source_or_vice_versa() {
    let source = "import { stdout } from std.io;\nclass Counter {\n\
        mut value: Int32;\n\
        construct(value: Int32) { this.value = value; }\n\
    }\n\n\
    fn main(): Void {\n\
        mut a: Counter = Counter(1);\n\
        mut b: Counter = a.clone();\n\n\
        b.value = 99;\n\
        stdout.println(a.value);\n\
        stdout.println(b.value);\n\n\
        a.value = -1;\n\
        stdout.println(a.value);\n\
        stdout.println(b.value);\n\
    }";

    let output = zirk_with_env(source, "clone_mutation_independence", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(normalize(&output.stdout), "1\n99\n-1\n99\n");
}

/// Forces a collection mid-clone (`ZIRK_GC_THRESHOLD=1`, the same mechanism
/// the collector/`Weak<T>` e2e tests above use) on a chain long enough that
/// cloning it crosses the threshold partway through — confirms the finished
/// clone is fully correct (nothing partially built gets collected), the
/// concrete end-to-end test for `zirk-object-memory`'s own "Deep-clone
/// traversal state is collector-safe for its whole duration" requirement.
#[test]
fn a_collection_forced_mid_clone_does_not_corrupt_the_finished_clone() {
    let source = "import { stdout } from std.io;\nclass Node {\n\
        mut value: Int32;\n\
        mut next: Node?;\n\
        construct(value: Int32) { this.value = value; this.next = null; }\n\
    }\n\n\
    fn build(n: Int32): Node {\n\
        mut head: Node = Node(0);\n\
        mut i: Int32 = 1;\n\
        while i < n {\n\
            mut fresh: Node = Node(i);\n\
            fresh.next = head;\n\
            head = fresh;\n\
            i = i + 1;\n\
        }\n\
        return head;\n\
    }\n\n\
    fn sum(start: Node): Int32 {\n\
        mut total: Int32 = 0;\n\
        mut current: Node? = start;\n\
        while true {\n\
            match current {\n\
                null => { return total; }\n\
                n => {\n\
                    total = total + n.value;\n\
                    current = n.next;\n\
                }\n\
            }\n\
        }\n\
        return total;\n\
    }\n\n\
    fn main(): Void {\n\
        mut a: Node = build(200);\n\
        mut b: Node = a.clone();\n\
        stdout.println(sum(a));\n\
        stdout.println(sum(b));\n\
        stdout.println(a is b);\n\
    }";

    let output = zirk_with_env(
        source,
        "clone_gc_mid_clone",
        &[("ZIRK_GC_THRESHOLD", "256")],
    );

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "19900\n19900\nfalse\n",
        "the clone must be complete and correct even though a collection \n\
         was forced partway through building it"
    );
}

/// A class with a `Pointer<T>` field fails to compile `.clone()` with a
/// diagnostic naming that field (spec scenario "Graph reaches a non-Clone
/// member").
#[test]
fn clone_of_a_class_reaching_a_pointer_field_is_rejected_at_compile_time() {
    let source = "import { stdout } from std.io;\nclass Holder {\n\
        mut p: Pointer<Int32>;\n\
        construct(p: Pointer<Int32>) { this.p = p; }\n\
    }\n\n\
    fn main(): Void {\n\
        mut x: Int32 = 1;\n\
        unsafe {\n\
            mut h: Holder = Holder(Pointer.from(x));\n\
            mut clone: Holder = h.clone();\n\
        }\n\
    }";

    let output = zirk_with_env(source, "clone_rejects_pointer_field", &[]);

    assert_ne!(output.status, 0, "a Pointer<T> field must reject Clone");
    assert!(
        output.stderr.contains("E0450"),
        "stderr:\n{}",
        output.stderr
    );
    assert!(output.stderr.contains("p"), "stderr:\n{}", output.stderr);
}

#[test]
fn bundled_std_http_post_json_value_serializes_and_sends_json_media_type() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "POST / HTTP/1.1\r\n");
            let mut length = 0usize;
            let mut has_json_content_type = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                let normalized = header.to_ascii_lowercase();
                if let Some(value) = normalized.strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                has_json_content_type |= normalized.starts_with("content-type: application/json");
            }
            assert!(has_json_content_type);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            assert_eq!(body, b"true");
            reader
                .get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        }
    });

    let project = workspace("approved_std_http_post_json");
    let home = workspace("approved_std_http_post_json_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.post-json.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTP, HTTPBody, HTTPJsonSerializable, URL }} from std.http;
import {{ stdout }} from std.io;
class JsonFlag implements HTTPJsonSerializable<JsonFlag> {{
    value: Boolean;
    construct(value: Boolean) {{ this.value = value; }}
    static to_json(value: JsonFlag): Result<String, String> {{
        if value.value {{ return Ok("true"); }}
        return Error("serialization rejected");
    }}
}}
             fn main(): Void {{
    match URL.parse("http://127.0.0.1:{}") {{
        Ok(url) => {{
            match HTTP.post_json(url, "not-json") {{
                Ok(_) => stdout.println("invalid JSON accepted");
                Error(_) => stdout.println("invalid JSON rejected");
            }}
            match HTTP.post_json_value<JsonFlag>(url, JsonFlag(false)) {{
                Ok(_) => stdout.println("serialization error accepted");
                Error(_) => stdout.println("serialization error rejected");
            }}
            match HTTP.post_json_value<JsonFlag>(url, JsonFlag(true)) {{
                Ok(response) => stdout.println(response.status_code());
                Error(_) => stdout.println("JSON post failed");
            }}
            match HTTP.post_body(url, HTTPBody.Json("true")) {{
                Ok(response) => stdout.println(response.status_code());
                Error(_) => stdout.println("JSON body variant failed");
            }}
        }}
        Error(_) => stdout.println("invalid URL");
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP POST JSON approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "invalid JSON rejected\nserialization error rejected\n200\n200\n"
    );
    server.join().unwrap();
}

#[test]
fn bundled_http_form_builder_encodes_and_sends_a_bounded_request_body() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "POST / HTTP/1.1\r\n");
        let mut length = 0usize;
        let mut has_form_content_type = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            let normalized = header.to_ascii_lowercase();
            if let Some(value) = normalized.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            has_form_content_type |=
                normalized.starts_with("content-type: application/x-www-form-urlencoded");
        }
        assert!(has_form_content_type);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        assert_eq!(body, b"q=hello+world&tag=a%2Bb&tag=%E4%BA%8C");
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();

        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "POST / HTTP/1.1\r\n");
        let mut length = 0usize;
        let mut has_multipart_content_type = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            let normalized = header.to_ascii_lowercase();
            if let Some(value) = normalized.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            has_multipart_content_type |= normalized
                .starts_with("content-type: multipart/form-data; boundary=\"zirk-boundary\"");
        }
        assert!(has_multipart_content_type);
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let mut expected = b"--zirk-boundary\r\nContent-Disposition: form-data; name=\"upload\"; filename=\"sample.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
        expected.extend_from_slice(&[0, 0xff, b'\r', b'\n']);
        expected.extend_from_slice(b"\r\n--zirk-boundary--\r\n");
        assert_eq!(body, expected);
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });

    let project = workspace("approved_std_http_form_builder");
    let home = workspace("approved_std_http_form_builder_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.form-builder.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
        r#"import {{ HTTP, HTTPBody, HTTPError, HTTPFormData, HTTPMultipartBuilder, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut capped = HTTPFormData.create();
    mut field_index: Int32 = 0;
    while field_index < 256 {{
        match capped.add("field", "value") {{
            Ok(_) => {{ }}
            Error(_) => {{ stdout.println("field rejected too early"); return; }}
        }}
        field_index += 1;
    }}
    match capped.add("overflow", "value") {{
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("field limit missing");
    }}
    mut form = HTTPFormData.create();
    match form.add("q", "hello world") {{ Ok(_) => {{ }} Error(error) => {{ stdout.println(error.to_string()); return; }} }}
    match form.add("tag", "a+b") {{ Ok(_) => {{ }} Error(error) => {{ stdout.println(error.to_string()); return; }} }}
    match form.add("tag", "二") {{ Ok(_) => {{ }} Error(error) => {{ stdout.println(error.to_string()); return; }} }}
    match URL.parse("http://127.0.0.1:{}/") {{
        Ok(url) => match HTTP.post_body(url, HTTPBody.Form(form)) {{
            Ok(response) => stdout.println(response.status_code());
            Error(error) => stdout.println(error.to_string());
        }}
        Error(_) => stdout.println("invalid URL");
    }}
    mut multipart = HTTPMultipartBuilder.create();
    match multipart.add("upload", [0 as Byte, 255 as Byte, 13 as Byte, 10 as Byte], "sample.bin", "application/octet-stream") {{
        Error(error) => {{ stdout.println(error.to_string()); return; }}
        Ok(_) => {{ }}
    }}
    match URL.parse("http://127.0.0.1:{}/") {{
        Ok(url) => match HTTP.post_body(url, HTTPBody.Multipart(multipart, "zirk-boundary")) {{
            Ok(response) => stdout.println(response.status_code());
            Error(error) => stdout.println(error.to_string());
        }}
        Error(_) => stdout.println("invalid URL");
    }}
}}"#,
            address.port(),
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP form permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run compiled URL-encoded form request");
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        normalize(&String::from_utf8_lossy(&output.stdout)),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "TooManyFields\n200\n201\n"
    );
    server.join().unwrap();
}

#[test]
fn bundled_http_sse_client_returns_typed_events_and_reconnect_metadata() {
    use std::io::{BufRead, BufReader, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = b": keepalive\nevent: update\nid: cursor-2\nretry: 100\ndata: one\ndata: continued\n\ndata: two\n\n";
    let server = std::thread::spawn(move || {
        for request_index in 0..6 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line, "GET /events HTTP/1.1\r\n");
            let mut last_event_id = String::new();
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if line.to_ascii_lowercase().starts_with("last-event-id: ") {
                    last_event_id = line[15..].trim().to_owned();
                }
            }
            let expected_last_event_id = match request_index {
                0 => "",
                1 => "cursor-2",
                2 => "",
                3 => "auto-1",
                _ => "",
            };
            assert_eq!(last_event_id, expected_last_event_id);
            let response_body = if request_index == 0 {
                &body[..]
            } else if request_index == 1 {
                &b"id: cursor-3\ndata: resumed\n\n"[..]
            } else if request_index == 2 {
                &b": auto keepalive\nretry: 0\nid: auto-1\ndata: automatic\n\n"[..]
            } else if request_index == 3 {
                &b"id: auto-2\ndata: resumed automatically\n\n"[..]
            } else {
                &b""[..]
            };
            let status = if request_index == 5 {
                "204 No Content"
            } else {
                "200 OK"
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/event-stream; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            )
            .unwrap();
            for fragment in response_body.chunks(3) {
                stream.write_all(fragment).unwrap();
            }
        }
    });

    let project = workspace("bundled_std_http_sse_client");
    let home = workspace("bundled_std_http_sse_client_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.sse.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTPEventStream, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("http://127.0.0.1:{}/events") {{
        Error(_) => stdout.println("invalid url");
        Ok(url) => match HTTPEventStream.connect(url) {{
            Error(error) => stdout.println(error.to_string());
            Ok(stream) => {{
                match stream.next_event() {{
                    Error(error) => stdout.println("first:" + error.to_string());
                    Ok(event) => match event {{
                        null => stdout.println("missing first event");
                        value => {{
                            stdout.println(value.event_type() + ":" + value.data() + ":" + value.last_event_id());
                            match value.retry_ms() {{ null => stdout.println("no retry"); retry => stdout.println(retry); }}
                        }}
                    }}
                }}
                stdout.println(stream.heartbeat_count());
                match stream.next_event() {{
                    Error(error) => stdout.println("second:" + error.to_string());
                    Ok(event) => match event {{
                        null => stdout.println("missing second event");
                        value => stdout.println(value.event_type() + ":" + value.data() + ":" + value.last_event_id());
                    }}
                }}
                match stream.next_event() {{
                    Error(error) => stdout.println("end:" + error.to_string());
                    Ok(event) => match event {{
                        null => stdout.println("end");
                        value => stdout.println("unexpected extra event:" + value.data());
                    }}
                }}
                stdout.println(stream.close());
                match HTTPEventStream.reconnect(url, "cursor-2\nInjected: yes") {{
                    Error(error) => stdout.println("invalid-id:" + error.to_string());
                    Ok(invalid) => {{ stdout.println("injected-id-accepted"); invalid.close(); }}
                }}
                match HTTPEventStream.reconnect(url, "cursor-2") {{
                    Error(error) => stdout.println("reconnect:" + error.to_string());
                    Ok(resumed) => match resumed.next_event() {{
                        Error(error) => stdout.println("resumed:" + error.to_string());
                        Ok(event) => match event {{
                            null => stdout.println("missing resumed event");
                            value => stdout.println(value.event_type() + ":" + value.data() + ":" + value.last_event_id());
                        }}
                    }}
                }}
                match HTTPEventStream.connect_with_reconnect(url) {{
                    Error(error) => stdout.println("auto-connect:" + error.to_string());
                    Ok(auto) => {{
                        match auto.next_event() {{
                            Error(error) => stdout.println("auto-first:" + error.to_string());
                            Ok(event) => match event {{ null => stdout.println("auto-missing first"); value => stdout.println(value.data()); }}
                        }}
                        stdout.println(auto.heartbeat_count());
                        match auto.next_event() {{
                            Error(error) => stdout.println("auto-second:" + error.to_string());
                            Ok(event) => match event {{ null => stdout.println("auto-missing second"); value => stdout.println(value.data()); }}
                        }}
                        stdout.println(auto.heartbeat_count());
                        _ = auto.close();
                        stdout.println("auto closed");
                    }}
                }}
                match HTTPEventStream.connect_with_reconnect(url) {{
                    Error(error) => stdout.println("no-content:" + error.to_string());
                    Ok(finished) => match finished.next_event() {{
                        Error(error) => stdout.println("no-content:" + error.to_string());
                        Ok(event) => match event {{ null => stdout.println("no-content stopped"); value => stdout.println("unexpected:" + value.data()); }}
                    }}
                }}
            }}
        }}
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit SSE network approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run compiled-Zirk SSE client");
    assert!(
        output.status.success(),
        "status: {}; stdout: {}; stderr: {}",
        output.status,
        normalize(&String::from_utf8_lossy(&output.stdout)),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "update:one\ncontinued:cursor-2\n100\n1\nmessage:two:cursor-2\nend\ntrue\ninvalid-id:Protocol\nmessage:resumed:cursor-3\nautomatic\n1\nresumed automatically\n1\nauto closed\nno-content stopped\n"
    );
    server.join().unwrap();
}

#[test]
fn bundled_http_streaming_post_sends_bounded_chunks_with_backpressure() {
    use std::io::{BufRead, BufReader, Read, Write};

    fn read_chunked_upload(
        listener: &std::net::TcpListener,
        expected_method: &str,
        expected_content_type: &str,
        expected_body: &[u8],
    ) {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, format!("{expected_method} /upload HTTP/1.1\r\n"));
        let mut chunked = false;
        let mut content_type = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            let normalized = header.to_ascii_lowercase();
            chunked |= normalized.starts_with("transfer-encoding: chunked");
            content_type |= normalized.starts_with(&format!(
                "content-type: {}",
                expected_content_type.to_ascii_lowercase()
            ));
        }
        assert!(chunked, "streamed HTTP/1.1 upload must use chunked framing");
        assert!(content_type);
        let mut body = Vec::new();
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).unwrap();
            let size = usize::from_str_radix(size.trim(), 16).unwrap();
            if size == 0 {
                let mut trailing = String::new();
                reader.read_line(&mut trailing).unwrap();
                assert_eq!(trailing, "\r\n");
                break;
            }
            let start = body.len();
            body.resize(start + size, 0);
            reader.read_exact(&mut body[start..]).unwrap();
            let mut crlf = [0; 2];
            reader.read_exact(&mut crlf).unwrap();
            assert_eq!(&crlf, b"\r\n");
        }
        assert_eq!(body, expected_body);
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        read_chunked_upload(
            &listener,
            "POST",
            "application/octet-stream",
            &[b"first-".as_slice(), &[0, 255], b"last"].concat(),
        );
        read_chunked_upload(&listener, "QUERY", "application/json", b"query");
        read_chunked_upload(&listener, "M-SEARCH", "application/octet-stream", b"search");
    });

    let project = workspace("approved_std_http_streaming_post");
    let home = workspace("approved_std_http_streaming_post_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.streaming.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTP, HTTPBody, HTTPHeaders, HTTPMethod, HTTPStreamProducerError, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut headers = HTTPHeaders.create();
    match headers.add("content-type", "application/octet-stream") {{
        Error(_) => {{ stdout.println("header failed"); return; }}
        Ok(_) => {{ }}
    }}
    match URL.parse("http://127.0.0.1:{}/upload") {{
        Error(_) => stdout.println("invalid URL");
        Ok(url) => match HTTP.post_body(url, HTTPBody.Stream(headers, (index: UInt64): Array<Byte> => match index as Int32 {{
            0 => [102 as Byte, 105 as Byte, 114 as Byte, 115 as Byte, 116 as Byte, 45 as Byte];
            1 => [0 as Byte, 255 as Byte];
            2 => [108 as Byte, 97 as Byte, 115 as Byte, 116 as Byte];
            _ => {{ mut empty: Array<Byte> = Array(0); empty; }};
        }})) {{
            Error(error) => {{ stdout.println(error.to_string()); false; }}
            Ok(response) => {{ stdout.println(response.status_code()); response.close(); }}
        }}
    }}
    mut query_headers = HTTPHeaders.create();
    match query_headers.add("content-type", "application/json") {{
        Error(_) => {{ stdout.println("query header failed"); return; }}
        Ok(_) => {{ }}
    }}
    match URL.parse("http://127.0.0.1:{}/upload") {{
        Error(_) => stdout.println("invalid QUERY URL");
        Ok(url) => match HTTP.request_stream_result(HTTPMethod.Query, url, query_headers, (index: UInt64): Result<Array<Byte>, HTTPStreamProducerError> => match index as Int32 {{
            0 => Ok<Array<Byte>, HTTPStreamProducerError>([113 as Byte, 117 as Byte, 101 as Byte, 114 as Byte, 121 as Byte]);
            _ => {{ mut empty: Array<Byte> = Array(0); Ok<Array<Byte>, HTTPStreamProducerError>(empty); }};
        }}) {{
            Error(error) => {{ stdout.println(error.to_string()); false; }}
            Ok(response) => {{ stdout.println(response.status_code()); response.close(); }}
        }}
    }}
    match URL.parse("http://127.0.0.1:{}/upload") {{
        Error(_) => stdout.println("invalid extension URL");
        Ok(url) => match HTTP.request_stream_named("M-SEARCH", url, headers, (index: UInt64): Array<Byte> => match index as Int32 {{
            0 => [115 as Byte, 101 as Byte, 97 as Byte, 114 as Byte, 99 as Byte, 104 as Byte];
            _ => {{ mut empty: Array<Byte> = Array(0); empty; }};
        }}) {{
            Error(error) => {{ stdout.println(error.to_string()); false; }}
            Ok(response) => {{ stdout.println(response.status_code()); response.close(); }}
        }}
    }}
    match URL.parse("http://127.0.0.1:{}/upload") {{
        Error(_) => stdout.println("invalid validation URL");
        Ok(url) => match HTTP.request_stream_named("BAD METHOD", url, headers, (index: UInt64): Array<Byte> => match index as Int32 {{
            _ => {{ mut empty: Array<Byte> = Array(0); empty; }};
        }}) {{
            Error(error) => {{ stdout.println(error.to_string()); false; }}
            Ok(response) => {{ stdout.println("invalid method connected"); response.close(); }}
        }}
    }}
}}"#,
            address.port(),
            address.port(),
            address.port(),
            address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP upload permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run compiled streaming HTTP upload");
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        normalize(&String::from_utf8_lossy(&output.stdout)),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "201\n201\n201\nInvalidArgument\n"
    );
    server.join().unwrap();
}

#[test]
fn http_stream_producer_errors_are_typed_and_abort_before_connecting() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let failure_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let failure_address = failure_listener.local_addr().unwrap();
    let failure_server = std::thread::spawn(move || {
        let (mut stream, _) = failure_listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        stream.read_to_end(&mut request).unwrap();
        request
    });
    let project = workspace("http_stream_producer_errors");
    let home = workspace("http_stream_producer_errors_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.stream.producer-errors.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\", \"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port(),
            failure_address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTP, HTTPError, HTTPHeaders, HTTPMethod, HTTPStreamProducerError, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    mut headers = HTTPHeaders.create();
    match URL.parse("http://127.0.0.1:{}/upload") {{
        Error(_) => stdout.println("invalid URL");
        Ok(url) => {{
            match HTTP.request_stream_result(HTTPMethod.Post, url, headers, (index: UInt64): Result<Array<Byte>, HTTPStreamProducerError> => Error<Array<Byte>, HTTPStreamProducerError>(HTTPStreamProducerError.Cancelled)) {{
                Error(error) => match error {{ HTTPError.Cancelled => stdout.println("cancelled"); _ => stdout.println("wrong cancellation error"); }}
                Ok(response) => {{ stdout.println("unexpected response"); response.close(); }}
            }}
            match HTTP.request_stream_named_result("PURGE", url, headers, (index: UInt64): Result<Array<Byte>, HTTPStreamProducerError> => Error<Array<Byte>, HTTPStreamProducerError>(HTTPStreamProducerError.Failed)) {{
                Error(error) => match error {{ HTTPError.Transport => stdout.println("producer failed"); _ => stdout.println("wrong producer error"); }}
                Ok(response) => {{ stdout.println("unexpected response"); response.close(); }}
            }}
        }}
    }}
    match URL.parse("http://127.0.0.1:{}/upload") {{
        Error(_) => stdout.println("invalid failure URL");
        Ok(url) => match HTTP.request_stream_result(HTTPMethod.Post, url, headers, (index: UInt64): Result<Array<Byte>, HTTPStreamProducerError> => match index as Int32 {{
            0 => Ok<Array<Byte>, HTTPStreamProducerError>([97 as Byte, 98 as Byte, 99 as Byte]);
            _ => Error<Array<Byte>, HTTPStreamProducerError>(HTTPStreamProducerError.Failed);
        }}) {{
            Error(error) => match error {{ HTTPError.Transport => stdout.println("midstream failed"); _ => stdout.println("wrong midstream error"); }}
            Ok(response) => {{ stdout.println("unexpected midstream response"); response.close(); stdout.println("closed"); }}
        }}
    }}
}}"#,
            address.port(),
            failure_address.port()
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit streaming producer permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run streaming producer error fixture");
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        normalize(&String::from_utf8_lossy(&output.stdout)),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "cancelled\nproducer failed\nmidstream failed\n"
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    let request = failure_server.join().unwrap();
    if !request.is_empty() {
        assert!(request.starts_with(b"POST /upload HTTP/1.1\r\n"));
        assert!(
            request
                .windows(b"Transfer-Encoding: chunked\r\n".len())
                .any(|window| window.eq_ignore_ascii_case(b"Transfer-Encoding: chunked\r\n"))
        );
        assert!(!request.ends_with(b"0\r\n\r\n"));
    }
}

#[test]
fn buffered_http_response_can_transfer_its_one_shot_body_to_streaming_reader() {
    use std::io::{Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 1024];
        let read = stream.read(&mut request).unwrap();
        assert!(String::from_utf8_lossy(&request[..read]).starts_with("GET / HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello")
            .unwrap();
    });

    let project = workspace("approved_std_http_buffered_to_stream");
    let home = workspace("approved_std_http_buffered_to_stream_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.buffered-to-stream.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ HTTP, HTTPError, HTTPStreamChunk, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("http://127.0.0.1:{}/") {{
        Error(_) => stdout.println("invalid URL");
        Ok(url) => match HTTP.get(url) {{
            Error(error) => stdout.println(error.to_string());
            Ok(response) => {{
                stdout.println(response.status_code());
                match response.read_chunk(0) {{
                    HTTPStreamChunk.Error(error) => match error {{
                        HTTPError.InvalidArgument => stdout.println("invalid buffered chunk size");
                        _ => stdout.println("wrong buffered chunk error");
                    }}
                    _ => stdout.println("invalid buffered chunk size accepted");
                }}
                match response.stream() {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(stream) => {{
                        match response.bytes() {{
                            Error(error) => match error {{
                                HTTPError.BodyConsumed => stdout.println("body consumed");
                                _ => stdout.println("wrong body error");
                            }}
                            Ok(_) => stdout.println("buffered body still readable");
                        }}
                        match stream.read_chunk(2) {{
                            HTTPStreamChunk.Bytes(part) => {{ stdout.println(part.length); stdout.println(part[0]); stdout.println(part[1]); }}
                            HTTPStreamChunk.End => stdout.println("early EOF");
                            HTTPStreamChunk.Error(_) => stdout.println("read failed");
                        }}
                        match stream.read_chunk(2) {{
                            HTTPStreamChunk.Bytes(part) => {{ stdout.println(part.length); stdout.println(part[0]); stdout.println(part[1]); }}
                            HTTPStreamChunk.End => stdout.println("early EOF");
                            HTTPStreamChunk.Error(_) => stdout.println("read failed");
                        }}
                        match stream.read_chunk(2) {{
                            HTTPStreamChunk.Bytes(part) => {{ stdout.println(part.length); stdout.println(part[0]); }}
                            HTTPStreamChunk.End => stdout.println("early EOF");
                            HTTPStreamChunk.Error(_) => stdout.println("read failed");
                        }}
                        match stream.read_chunk(2) {{
                            HTTPStreamChunk.Bytes(_) => stdout.println("extra bytes");
                            HTTPStreamChunk.End => stdout.println("stream EOF");
                            HTTPStreamChunk.Error(_) => stdout.println("read failed");
                        }}
                        stream.close();
                        stdout.println("stream closed");
                    }}
                }}
            }}
        }}
    }}
}}"#,
            address.port()
        ),
    )
    .unwrap();
    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit buffered response stream approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run buffered response stream transfer");
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        normalize(&String::from_utf8_lossy(&output.stdout)),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&output.stdout)),
        "200\ninvalid buffered chunk size\nbody consumed\n2\n104\n101\n2\n108\n108\n1\n111\nstream EOF\nstream closed\n"
    );
    server.join().unwrap();
}

#[test]
fn cancelling_structured_task_aborts_streaming_http_upload() {
    use std::io::{BufRead, BufReader, Read, Write};

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "POST /upload HTTP/1.1\r\n");
        let mut chunked = false;
        loop {
            line.clear();
            let count = reader.read_line(&mut line).unwrap();
            assert_ne!(count, 0, "request headers ended unexpectedly");
            if line == "\r\n" {
                break;
            }
            chunked |= line.eq_ignore_ascii_case("transfer-encoding: chunked\r\n");
        }
        assert!(chunked);
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(usize::from_str_radix(line.trim(), 16).unwrap(), 4);
        let mut data = [0; 4];
        reader.read_exact(&mut data).unwrap();
        assert_eq!(&data, b"part");
        let mut crlf = [0; 2];
        reader.read_exact(&mut crlf).unwrap();
        assert_eq!(&crlf, b"\r\n");
        line.clear();
        let _ = reader.read_line(&mut line).unwrap();
        line
    });

    let project = workspace("http_stream_upload_task_cancel");
    let home = workspace("http_stream_upload_task_cancel_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(project.join(".zkinit"), format!(
        "project \"http.stream.cancel.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n", address.port())).unwrap();
    std::fs::write(project.join("src/main.zrk"), format!(r#"import {{ HTTP, HTTPError, HTTPHeaders, URL }} from std.http;
import {{ stdout }} from std.io;
fn upload(endpoint: String): Void {{
    mut headers = HTTPHeaders.create();
    match URL.parse(endpoint) {{
        Error(_) => stdout.println("invalid URL");
        Ok(url) => match HTTP.post_stream(url, headers, (index: UInt64): Array<Byte> => match index as Int32 {{
            0 => [112 as Byte, 97 as Byte, 114 as Byte, 116 as Byte];
            _ => {{ Timer.sleep(10000ms); mut empty: Array<Byte> = Array(0); empty; }};
        }}) {{
            Error(error) => {{ match error {{ HTTPError.Cancelled => stdout.println("request cancelled"); _ => stdout.println("request stopped"); }} false; }}
            Ok(response) => {{ stdout.println("unexpected response"); response.close(); }}
        }}
    }}
}}
fn main(): Void {{
    inmut request_job = spawn upload("http://127.0.0.1:{}/upload");
    Timer.sleep(500ms);
    request_job.cancel();
    stdout.println("upload task joined");
}}"#, address.port())).unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let output = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status {}; stdout {}; stderr {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(normalize(&String::from_utf8_lossy(&output.stdout)).ends_with("upload task joined\n"));
    let next_frame = server.join().unwrap();
    assert!(
        next_frame.is_empty(),
        "cancelled upload wrote another chunk: {next_frame:?}; child stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_http_multipart_builder_encodes_binary_parts_with_bounded_metadata() {
    let project = workspace("http_multipart_builder");
    let source = project.join("multipart.zrk");
    std::fs::write(
        &source,
        r#"import { HTTP, HTTPError, HTTPMultipartBuilder, URL } from std.http;
import { stdout } from std.io;
fn main(): Void {
    mut builder = HTTPMultipartBuilder.create();
    mut payload: Array<Byte> = [0, 255, 13, 10];
    match builder.add("upload", payload, "sample.bin", "application/octet-stream") {
        Ok(_) => stdout.println("added");
        Error(error) => { stdout.println(error.to_string()); return; }
    }
    match builder.encode("zirk-boundary") {
        Ok(body) => {
            stdout.println(body.length);
            stdout.println(body[0] as Int32);
            stdout.println(body[body.length - 1] as Int32);
        }
        Error(error) => stdout.println(error.to_string());
    }
    mut invalid = HTTPMultipartBuilder.create();
    match invalid.add("field\r\nX-Evil: yes", payload) {
        Ok(_) => { stdout.println("invalid metadata retained"); match invalid.encode("safe") {
            Error(error) => stdout.println(error.to_string());
            Ok(_) => stdout.println("header injection accepted");
        } }
        Error(error) => stdout.println(error.to_string());
    }
    mut bad_boundary = HTTPMultipartBuilder.create();
    match bad_boundary.encode("bad\r\nvalue") {
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("invalid boundary accepted");
    }
    mut collision = HTTPMultipartBuilder.create();
    mut collision_body: Array<Byte> = [112 as Byte, 114 as Byte, 101 as Byte, 102 as Byte,
        105 as Byte, 120 as Byte, 13 as Byte, 10 as Byte, 45 as Byte, 45 as Byte,
        122 as Byte, 105 as Byte, 114 as Byte, 107 as Byte, 45 as Byte, 98 as Byte,
        111 as Byte, 117 as Byte, 110 as Byte, 100 as Byte, 97 as Byte, 114 as Byte,
        121 as Byte, 13 as Byte, 10 as Byte, 115 as Byte, 117 as Byte, 102 as Byte,
        102 as Byte, 105 as Byte, 120 as Byte];
    match collision.add("field", collision_body) {
        Error(error) => stdout.println(error.to_string());
        Ok(_) => match collision.encode("zirk-boundary") {
            Error(error) => stdout.println(error.to_string());
            Ok(_) => stdout.println("boundary collision accepted");
        }
    }
    match URL.parse("http://127.0.0.1:1/upload") {
        Error(_) => stdout.println("invalid URL");
        Ok(url) => match HTTP.post_multipart(url, builder, "zirk-boundary") {
            Error(error) => stdout.println(error.to_string());
            Ok(response) => stdout.println(response.status_code());
        }
    }
    mut many = HTTPMultipartBuilder.create();
    mut index: Int32 = 0;
    while index < 256 {
        match many.add("field", [1 as Byte]) {
            Ok(_) => { }
            Error(error) => { stdout.println(error.to_string()); return; }
        }
        index += 1;
    }
    match many.add("overflow", [1 as Byte]) {
        Error(error) => stdout.println(error.to_string());
        Ok(_) => stdout.println("part limit missing");
    }
}"#,
    )
    .unwrap();

    let output = zirk("run", &source, "http_multipart_builder_run", &[]);
    assert_eq!(output.status, 0, "{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "added\n154\n45\n10\ninvalid metadata retained\nProtocol\nProtocol\nProtocol\nPermissionDenied\nTooManyParts\n"
    );
}

#[test]
fn bundled_std_http_authenticated_proxy_uses_environment_secret_only_for_proxy() {
    use std::io::{BufRead, BufReader, Write};

    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let proxy_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_address = proxy_listener.local_addr().unwrap();
    let proxy = std::thread::spawn(move || {
        let (stream, _) = proxy_listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(request_line, "GET http://127.0.0.1:1/path HTTP/1.1\r\n");
        let mut saw_auth = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("proxy-authorization")
            {
                saw_auth = value.trim() == "Basic dXNlcjpzZWNyZXQ=";
            }
        }
        assert!(saw_auth, "proxy authorization was not sent to the proxy");
        reader
            .into_inner()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nproxy!")
            .unwrap();
    });

    let project = workspace("std_http_authenticated_proxy");
    let home = workspace("std_http_authenticated_proxy_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.proxy.auth.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nenvironment {{\nsecrets: {{ names: [\"PROXY_AUTH\"]; during: runtime; }}\n}}\nnetwork {{\nconnect: {{ origins: [\"http://127.0.0.1:1\", \"http://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            proxy_address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ Env }} from std.environment;
import {{ HTTPClient, HTTPProxy, HTTPMethod, HTTPHeaders, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("http://127.0.0.1:{}") {{
        Error(_) => stdout.println("proxy URL invalid");
        Ok(proxy_url) => match Env.get_secret("PROXY_AUTH") {{
            Error(_) => stdout.println("proxy secret denied");
            Ok(secret) => match HTTPProxy.create_authenticated(proxy_url, secret) {{
                Error(_) => stdout.println("proxy configuration rejected");
                Ok(proxy) => {{
                    mut certificate: Array<Byte> = [{certificate}];
                    mut roots: Array<Array<Byte>> = [certificate];
                    match HTTPClient.create_with_proxy_and_trust_roots(proxy, roots, 2) {{
                    Error(error) => stdout.println(error.to_string());
                    Ok(client) => match URL.parse("http://127.0.0.1:1/path") {{
                        Error(_) => stdout.println("destination URL invalid");
                        Ok(url) => {{
                            mut empty: Array<Byte> = [];
                            match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{
                                Error(error) => stdout.println(error.to_string());
                                Ok(response) => stdout.println(response.status_code());
                            }}
                            _ = client.close();
                            stdout.println("client closed");
                        }}
                    }}
                    }}
                }}
            }}
        }}
    }}
}}"#,
            proxy_address.port(),
            certificate = byte_literal(certificate)
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit HTTP proxy and secret approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .env("PROXY_AUTH", "user:secret")
        .output()
        .expect("run authenticated HTTP proxy fixture");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nclient closed\n"
    );
    proxy.join().unwrap();
}

#[test]
fn bundled_std_http_authenticated_https_proxy_uses_custom_trust_root() {
    use std::io::{BufRead, BufReader, Write};

    let cert = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let server_config = std::sync::Arc::new(
        tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                    cert.to_vec(),
                )],
                tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(key.to_vec()).unwrap(),
            )
            .unwrap(),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let config = std::sync::Arc::clone(&server_config);
    let proxy = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let tls = tokio_rustls::rustls::StreamOwned::new(
            tokio_rustls::rustls::ServerConnection::new(config).unwrap(),
            socket,
        );
        let mut reader = BufReader::new(tls);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        assert_eq!(
            request_line,
            "GET http://example.invalid/resource HTTP/1.1\r\n"
        );
        let mut saw_auth = false;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("proxy-authorization")
            {
                saw_auth = value.trim() == "Basic dXNlcjpzZWNyZXQ=";
            }
        }
        assert!(saw_auth);
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
            .unwrap();
    });

    let project = workspace("std_http_authenticated_https_proxy");
    let home = workspace("std_http_authenticated_https_proxy_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.http.proxy.https.auth\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nenvironment {{\nsecrets: {{ names: [\"PROXY_AUTH\"]; during: runtime; }}\n}}\nnetwork {{\nconnect: {{ origins: [\"http://example.invalid:80\", \"https://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            address.port()
        ),
    )
    .unwrap();
    let root_literal = root
        .iter()
        .map(|byte| format!("{byte} as Byte"))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ Env }} from std.environment;
import {{ HTTPClient, HTTPProxy, HTTPMethod, HTTPHeaders, URL }} from std.http;
import {{ stdout }} from std.io;
fn main(): Void {{
    match URL.parse("https://127.0.0.1:{proxy_port}") {{
        Error(_) => stdout.println("bad proxy URL");
        Ok(proxy_url) => match Env.get_secret("PROXY_AUTH") {{
            Error(_) => stdout.println("secret denied");
            Ok(secret) => match HTTPProxy.create_authenticated(proxy_url, secret) {{
                Error(_) => stdout.println("bad proxy config");
                Ok(proxy) => {{
                    mut roots: Array<Array<Byte>> = [[{root}]];
                    match HTTPClient.create_with_proxy_and_trust_roots(proxy, roots, 2) {{
                        Error(error) => stdout.println("create: " + error.to_string());
                        Ok(client) => match URL.parse("http://example.invalid:80/resource") {{
                            Error(_) => stdout.println("bad destination URL");
                            Ok(url) => {{
                                mut body: Array<Byte> = [];
                                match client.request(HTTPMethod.Get, url, body, HTTPHeaders.create()) {{
                                    Error(error) => stdout.println("request: " + error.to_string());
                                    Ok(response) => stdout.println(response.status_code());
                                }}
                                _ = client.close();
                                stdout.println("client closed");
                            }}
                        }}
                    }}
                }}
            }}
        }}
    }}
}}"#,
            proxy_port = address.port(),
            root = root_literal,
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}",
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .env("PROXY_AUTH", "user:secret")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nclient closed\n"
    );
    proxy.join().unwrap();
}

#[test]
fn bundled_http_client_uses_authenticated_proxy_for_custom_root_tunnel() {
    use std::io::{BufRead, BufReader, Write};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    let destination_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let destination_address = destination_listener.local_addr().unwrap();
    let proxy_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_address = proxy_listener.local_addr().unwrap();

    let certificate = include_bytes!("../../zirk-runtime/testdata/tls-loopback-cert.der");
    let private_key = include_bytes!("../../zirk-runtime/testdata/tls-loopback-key.der");
    let root = include_bytes!("../../zirk-runtime/testdata/tls-loopback-root.der");
    let server_config = std::sync::Arc::new(
        tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                    certificate.to_vec(),
                )],
                tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(private_key.to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let proxy_server_config = std::sync::Arc::new(
        tokio_rustls::rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![tokio_rustls::rustls::pki_types::CertificateDer::from(
                    certificate.to_vec(),
                )],
                tokio_rustls::rustls::pki_types::PrivateKeyDer::try_from(private_key.to_vec())
                    .unwrap(),
            )
            .unwrap(),
    );
    let destination = std::thread::spawn(move || {
        let (socket, _) = destination_listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let tls = tokio_rustls::rustls::StreamOwned::new(
            tokio_rustls::rustls::ServerConnection::new(server_config).unwrap(),
            socket,
        );
        let mut reader = BufReader::new(tls);
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        assert_eq!(request, "GET /through-tunnel HTTP/1.1\r\n");
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header == "\r\n" || header.is_empty() {
                break;
            }
            assert!(
                !header
                    .to_ascii_lowercase()
                    .starts_with("proxy-authorization:")
            );
        }
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\ntunneled")
            .unwrap();
        reader.get_mut().flush().unwrap();
    });

    proxy_listener.set_nonblocking(true).unwrap();
    let proxy = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(proxy_listener).unwrap();
            let (socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(10), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let acceptor = tokio_rustls::TlsAcceptor::from(proxy_server_config);
            let tls =
                tokio::time::timeout(std::time::Duration::from_secs(10), acceptor.accept(socket))
                    .await
                    .unwrap()
                    .unwrap();
            let mut reader = tokio::io::BufReader::new(tls);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).await.unwrap();
            assert_eq!(
                request_line,
                format!(
                    "CONNECT 127.0.0.1:{} HTTP/1.1\r\n",
                    destination_address.port()
                )
            );
            let mut saw_proxy_auth = false;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).await.unwrap();
                if header == "\r\n" || header.is_empty() {
                    break;
                }
                assert!(!header.to_ascii_lowercase().starts_with("authorization:"));
                if header
                    .to_ascii_lowercase()
                    .starts_with("proxy-authorization:")
                {
                    assert_eq!(header.trim(), "Proxy-Authorization: Basic dXNlcjpzZWNyZXQ=");
                    saw_proxy_auth = true;
                }
            }
            assert!(saw_proxy_auth, "CONNECT credentials were not sent to proxy");
            let mut upstream = tokio::net::TcpStream::connect(destination_address)
                .await
                .unwrap();
            let mut tls = reader.into_inner();
            tls.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .unwrap();
            tokio::io::copy_bidirectional(&mut tls, &mut upstream)
                .await
                .unwrap();
        });
    });

    let byte_literal = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte} as Byte"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let project = workspace("http_client_proxy_custom_roots");
    let home = workspace("http_client_proxy_custom_roots_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"http.proxy.custom.roots\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nenvironment {{\nsecrets: {{ names: [\"PROXY_AUTH\"]; during: runtime; }}\n}}\nnetwork {{\nconnect: {{ origins: [\"https://127.0.0.1:{}\", \"https://127.0.0.1:{}\"]; during: runtime; }}\n}}\n}}\n",
            destination_address.port(),
            proxy_address.port()
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            r#"import {{ Env }} from std.environment;
import {{ HTTPClient, HTTPClientOptions, HTTPHeaders, HTTPMethod, HTTPProxy, URL }} from std.http;
import {{ stdout }} from std.io;
fn close_probe(client: HTTPClient): Void {{ _ = client.close(); }}
fn request_through_proxy(proxy: HTTPProxy, roots: Array<Array<Byte>>, destination_port: UInt16): Void {{
    match HTTPClientOptions.create(
        connect_timeout_ms: 5000,
        response_headers_timeout_ms: 10000,
        response_idle_timeout_ms: 15000,
        max_origins: 2,
    ) {{
        Error(_) => stdout.println("client options invalid");
        Ok(options) => {{
            match HTTPClient.create_with_options_and_trust_roots(options, roots) {{
                Error(_) => stdout.println("custom-root options creation failed");
                Ok(client) => close_probe(client);
            }}
            match HTTPClient.create_with_options_and_proxy(options, proxy) {{
                Error(_) => stdout.println("proxy options creation failed");
                Ok(client) => close_probe(client);
            }}
            match HTTPClient.create_with_options_and_proxy_and_trust_roots(options, proxy, roots) {{
                Error(_) => stdout.println("client creation failed");
                Ok(client) => match URL.parse("https://127.0.0.1:" + destination_port.to_string() + "/through-tunnel") {{
                    Error(_) => stdout.println("destination URL invalid");
                    Ok(url) => {{
                        mut empty: Array<Byte> = [];
                        match client.request(HTTPMethod.Get, url, empty, HTTPHeaders.create()) {{
                            Error(error) => stdout.println(error.to_string());
                            Ok(response) => stdout.println(response.status_code());
                        }}
                        _ = client.close();
                        stdout.println("client closed");
                    }}
                }}
            }}
        }}
    }}
}}
fn main(): Void {{
    mut roots: Array<Array<Byte>> = [[{root}]];
    match URL.parse("https://127.0.0.1:{proxy_port}") {{
        Error(_) => stdout.println("proxy URL invalid");
        Ok(proxy_url) => match Env.get_secret("PROXY_AUTH") {{
            Error(_) => stdout.println("proxy credential denied");
            Ok(secret) => match HTTPProxy.create_authenticated(proxy_url, secret) {{
                Error(_) => stdout.println("proxy config invalid");
                Ok(proxy) => request_through_proxy(proxy, roots, {destination_port} as UInt16);
            }}
        }}
    }}
}}"#,
            root = byte_literal(root),
            proxy_port = proxy_address.port(),
            destination_port = destination_address.port(),
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start HTTPS proxy and destination grant approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );
    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .env("PROXY_AUTH", "user:secret")
        .output()
        .expect("run HTTPS client through explicit proxy");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "200\nclient closed\n"
    );
    proxy.join().unwrap();
    destination.join().unwrap();
}

#[test]
fn bundled_std_net_interfaces_returns_an_approved_typed_snapshot() {
    let project = workspace("approved_std_net_interfaces");
    let home = workspace("approved_std_net_interfaces_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.net.interfaces.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {\nnetwork {\ninspect_interfaces: { enabled: true; during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        "import { NetworkInterfaces } from std.net::interfaces;\n\
         import { stdout } from std.io;\n\
         fn main(): Void {\n\
             match NetworkInterfaces.snapshot() {\n\
                 Ok(addresses) => {\n\
                     stdout.println(addresses.length.to_string());\n\
                     mut saw_loopback: Boolean = false;\n\
                     for address in addresses {\n\
                         if address.is_loopback { saw_loopback = true; }\n\
                         if address.prefix_length > 128 as UInt8 { stdout.println(\"invalid prefix\"); }\n\
                     }\n\
                     stdout.println(saw_loopback.to_string());\n\
                 }\n\
                 Error(error) => stdout.println(error.to_string());\n\
             }\n\
         }",
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit interface inspection approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved interface inspection program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let output = normalize(&String::from_utf8_lossy(&run.stdout));
    let mut lines = output.lines();
    let count = lines
        .next()
        .expect("snapshot count")
        .parse::<usize>()
        .unwrap();
    assert!(count > 0 && count <= 256);
    assert_eq!(lines.next(), Some("true"));
    assert_eq!(lines.next(), None);
}

#[test]
fn bundled_std_net_interfaces_denies_snapshot_without_authority() {
    let project = workspace("denied_std_net_interfaces");
    let home = workspace("denied_std_net_interfaces_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.net.interfaces.denied.fixture\"\nkind application\nentry \"src/main.zrk\"\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        "import { NetworkInterfaces } from std.net::interfaces;\n\
         import { stdout } from std.io;\n\
         fn main(): Void {\n\
             match NetworkInterfaces.snapshot() {\n\
                 Ok(_) => stdout.println(\"unexpected access\");\n\
                 Error(error) => stdout.println(error.to_string());\n\
             }\n\
         }",
    )
    .unwrap();

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run interface inspection without authority");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "PermissionDenied\n"
    );
}

#[cfg(unix)]
#[test]
fn bundled_std_net_local_streams_run_an_approved_unix_socket_exchange() {
    let project = workspace("approved_std_net_local");
    let home = workspace("approved_std_net_local_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let socket = std::env::temp_dir().join(format!("zl-{}.sock", std::process::id()));
    let endpoint = format!("unix://{}", socket.display());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.local.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlocal_connect: {{ endpoints: [\"{endpoint}\"]; during: runtime; }}\nlocal_listen: {{ endpoints: [\"{endpoint}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ LocalListener, LocalReadResult, LocalStaleEndpointPolicy, LocalStream }} from std.net::local;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
                 match LocalListener.bind(\"{endpoint}\", LocalStaleEndpointPolicy.RejectExisting) {{\n\
                     Ok(listener) => {{\n\
                         match LocalStream.connect(\"{endpoint}\") {{\n\
                             Ok(client) => {{\n\
                                 match listener.accept() {{\n\
                                     Ok(server) => {{\n\
                                         match client.write_all([112 as Byte, 105 as Byte, 110 as Byte, 103 as Byte]) {{\n\
                                             Ok(_) => {{\n\
                                                 match server.read(4 as UInt64) {{\n\
                                                     LocalReadResult.Value(bytes) => stdout.println(bytes.length.to_string());\n\
                                                     LocalReadResult.End => stdout.println(\"eof\");\n\
                                                     LocalReadResult.Error(error) => stdout.println(error.to_string());\n\
                                                 }}\n\
                                             }}\n\
                                             Error(error) => stdout.println(error.to_string());\n\
                                         }}\n\
                                         _ = client.close();\n\
                                         _ = server.close();\n\
                                     }}\n\
                                     Error(error) => stdout.println(error.to_string());\n\
                                 }}\n\
                             }}\n\
                             Error(error) => stdout.println(error.to_string());\n\
                         }}\n\
                         _ = listener.close();\n\
                     }}\n\
                     Error(error) => stdout.println(error.to_string());\n\
                 }}\n\
             }}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit local endpoint permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved local socket program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(normalize(&String::from_utf8_lossy(&run.stdout)), "4\n");
    assert!(
        !socket.exists(),
        "listener cleanup removes its owned socket"
    );
}

#[cfg(windows)]
#[test]
fn bundled_std_net_local_streams_run_an_approved_named_pipe_exchange() {
    let project = workspace("approved_std_net_local_named_pipe");
    let home = workspace("approved_std_net_local_named_pipe_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let endpoint = format!("npipe://./pipe/zirk-local-e2e-{}", std::process::id());
    std::fs::write(
        project.join(".zkinit"),
        format!(
            "project \"std.net.local.fixture\"\nkind application\nentry \"src/main.zrk\"\npermissions {{\nnetwork {{\nlocal_connect: {{ endpoints: [\"{endpoint}\"]; during: runtime; }}\nlocal_listen: {{ endpoints: [\"{endpoint}\"]; during: runtime; }}\n}}\n}}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ LocalListener, LocalReadResult, LocalStaleEndpointPolicy, LocalStream }} from std.net::local;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
                 match LocalListener.bind(\"{endpoint}\", LocalStaleEndpointPolicy.RejectExisting) {{\n\
                     Ok(listener) => {{\n\
                         match LocalStream.connect(\"{endpoint}\") {{\n\
                             Ok(client) => {{\n\
                                 match listener.accept() {{\n\
                                     Ok(server) => {{\n\
                                         match client.write_all([112 as Byte, 105 as Byte, 110 as Byte, 103 as Byte]) {{\n\
                                             Ok(_) => {{\n\
                                                 match server.read(4 as UInt64) {{\n\
                                                     LocalReadResult.Value(bytes) => stdout.println(bytes.length.to_string());\n\
                                                     LocalReadResult.End => stdout.println(\"eof\");\n\
                                                     LocalReadResult.Error(error) => stdout.println(error.to_string());\n\
                                                 }}\n\
                                             }}\n\
                                             Error(error) => stdout.println(error.to_string());\n\
                                         }}\n\
                                         _ = client.close();\n\
                                         _ = server.close();\n\
                                     }}\n\
                                     Error(error) => stdout.println(error.to_string());\n\
                                 }}\n\
                             }}\n\
                             Error(error) => stdout.println(error.to_string());\n\
                         }}\n\
                         _ = listener.close();\n\
                     }}\n\
                     Error(error) => stdout.println(error.to_string());\n\
                 }}\n\
             }}"
        ),
    )
    .unwrap();

    let mut approval = Command::new(compiler())
        .args(["permissions", "approve", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start explicit named pipe permission approval");
    approval
        .stdin
        .take()
        .unwrap()
        .write_all(b"approve\n")
        .unwrap();
    let approval = approval.wait_with_output().unwrap();
    assert!(
        approval.status.success(),
        "{}{}",
        String::from_utf8_lossy(&approval.stdout),
        String::from_utf8_lossy(&approval.stderr)
    );

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run approved named pipe program");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(normalize(&String::from_utf8_lossy(&run.stdout)), "4\n");
}

#[cfg(unix)]
#[test]
fn bundled_std_net_local_listener_denies_missing_authority_before_path_creation() {
    let project = workspace("denied_std_net_local");
    let home = workspace("denied_std_net_local_home");
    std::fs::create_dir_all(project.join("src")).unwrap();
    let socket = std::env::temp_dir().join(format!("zd-{}.sock", std::process::id()));
    let endpoint = format!("unix://{}", socket.display());
    std::fs::write(
        project.join(".zkinit"),
        "project \"std.net.local.denied.fixture\"\nkind application\nentry \"src/main.zrk\"\n",
    )
    .unwrap();
    std::fs::write(
        project.join("src/main.zrk"),
        format!(
            "import {{ LocalListener, LocalStaleEndpointPolicy }} from std.net::local;\n\
             import {{ stdout }} from std.io;\n\
             fn main(): Void {{\n\
                 match LocalListener.bind(\"{endpoint}\", LocalStaleEndpointPolicy.RejectExisting) {{\n\
                     Ok(listener) => {{ stdout.println(\"unexpectedly bound\"); _ = listener.close(); }}\n\
                     Error(error) => stdout.println(error.to_string());\n\
                 }}\n\
             }}"
        ),
    )
    .unwrap();

    let run = Command::new(compiler())
        .args(["run", "."])
        .current_dir(&project)
        .env("HOME", &home)
        .output()
        .expect("run local listener without listen authority");
    assert!(
        run.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        normalize(&String::from_utf8_lossy(&run.stdout)),
        "PermissionDenied\n"
    );
    assert!(
        !socket.exists(),
        "a denied bind must not create the endpoint path"
    );
}
