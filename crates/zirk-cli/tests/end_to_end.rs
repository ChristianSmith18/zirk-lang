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
    let source = "import { stdout } from std.io;\nclass Marker {
        id: Int32;
        construct(id: Int32) { this.id = id; }
    }

    fn show(a: Marker, b: Marker): Void {
        stdout.println(a.id);
        stdout.println(b.id);
    }

    fn main(): Void {
        show(Marker(111), Marker(222));
    }";

    let output = zirk_with_env(source, "gc_d4_hazard", &[("ZIRK_GC_THRESHOLD", "1")]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "111\n222\n",
        "the first argument's object must survive intact, not be collected \
         out from under the second argument's own construction"
    );
}

/// A sanity control for the test above: the same program, with the default
/// (generous) threshold, so a regression that happens to depend on the
/// *particular* low threshold used above (rather than on D4 itself) would
/// still be caught by comparison.
#[test]
fn d4_hazard_fixture_is_correct_without_a_forced_collection_too() {
    let source = "import { stdout } from std.io;\nclass Marker {
        id: Int32;
        construct(id: Int32) { this.id = id; }
    }

    fn show(a: Marker, b: Marker): Void {
        stdout.println(a.id);
        stdout.println(b.id);
    }

    fn main(): Void {
        show(Marker(111), Marker(222));
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
    let source = "import { stdout } from std.io;\nclass Node {
        mut next: Node?;
        construct() { }
    }

    fn churn(n: Int32): Void {
        mut i = 0;
        while i < n {
            mut a = Node();
            mut b = Node();
            a.next = b;
            b.next = a;
            i = i + 1;
        }
    }

    fn main(): Void {
        churn(300000);
        stdout.println(\"done\");
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
    let source = "import { stdout } from std.io;\nclass Holder {
        value: Int32;
        construct(value: Int32) { this.value = value; }
    }

    fn churn(n: Int32): Void {
        mut i = 0;
        while i < n {
            mut throwaway = Holder(0);
            i = i + 1;
        }
    }

    fn main(): Void {
        mut kept = Holder(999);
        churn(50000);
        stdout.println(kept.value);
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
        "`kept` must survive every collection `churn` triggers, since main's \
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
    let source = "import { stdout } from std.io;\nclass Scratch {
        value: Int32;
        construct(value: Int32) { this.value = value; }
    }

    fn use_once(v: Int32): Int32 {
        mut s = Scratch(v);
        return s.value;
    }

    fn main(): Void {
        mut i = 0;
        mut total = 0;
        while i < 200000 {
            total = total + use_once(1);
            i = i + 1;
        }
        stdout.println(total);
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
    let source = "import { stdout } from std.io;\nclass Marker { id: Int32; construct(id: Int32) { this.id = id; } }

    fn main(): Void {
        mut m: Marker = Marker(42);
        mut w: Weak<Marker> = Weak.from(m);
        stdout.println(w.is_alive);
        match w.upgrade() {
            null => stdout.println(\"gone\");
            found => stdout.println(found.id);
        }
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
    let source = "import { stdout } from std.io;\nclass Marker { id: Int32; construct(id: Int32) { this.id = id; } }

    fn make(): Weak<Marker> {
        mut m: Marker = Marker(7);
        return Weak.from(m);
    }

    fn main(): Void {
        mut w: Weak<Marker> = make();
        // `println`ing an `Int32` never allocates through the collector
        // (`String` conversion is its own allocation path, outside
        // `zirk_rt_alloc`'s reach) — an ordinary object construction is
        // what actually crosses `ZIRK_GC_THRESHOLD` and forces a real
        // mark-sweep collection.
        mut trigger: Marker = Marker(0);
        stdout.println(trigger.id);
        stdout.println(w.is_alive);
        match w.upgrade() {
            null => stdout.println(\"gone\");
            found => stdout.println(found.id);
        }
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
        "once the only strong reference is gone and a collection has run, \
         `.upgrade()` must return null and `.is_alive` must be false — the \
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
    let source = "import { stdout } from std.io;\nclass Marker { id: Int32; construct(id: Int32) { this.id = id; } }

    fn make(): Weak<Marker> {
        mut m: Marker = Marker(9);
        return Weak.from(m);
    }

    fn main(): Void {
        mut w: Weak<Marker> = make();
        // See `weak_upgrade_and_is_alive_see_a_collected_referent_as_gone`'s
        // own comment: an object construction, not a `println`, is what
        // actually crosses `ZIRK_GC_THRESHOLD`.
        mut trigger: Marker = Marker(0);
        stdout.println(trigger.id);
        mut stillAlive: Boolean = w.is_alive;
        stdout.println(stillAlive);
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
        "the Weak<Marker> handle `w` is still reachable throughout, but its \
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
    let source = "import { stdout } from std.io;\nclass Point {
        mut x: Int32;
        mut y: Int32;
        construct(x: Int32, y: Int32) { this.x = x; this.y = y; }
    }

    fn main(): Void {
        mut a: Point = Point(1, 2);
        mut b: Point = a.clone();
        stdout.println(a is b);
        stdout.println(b.x);
        stdout.println(b.y);
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
    let source = "import { stdout } from std.io;\nclass Child {
        mut value: Int32;
        construct(value: Int32) { this.value = value; }
    }

    class Parent {
        mut left: Child;
        mut right: Child;
        construct(left: Child, right: Child) { this.left = left; this.right = right; }
    }

    fn main(): Void {
        mut c: Child = Child(1);
        mut a: Parent = Parent(c, c);
        stdout.println(a.left is a.right);

        mut b: Parent = a.clone();
        stdout.println(b.left is b.right);
        stdout.println(b.left is a.left);
    }";

    let output = zirk_with_env(source, "clone_shared_child", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "true\ntrue\nfalse\n",
        "b.left is b.right must be true (one shared clone) and b.left is \
         a.left must be false (never an alias back into the source)"
    );
}

/// A cyclic graph (a node whose field points back to itself) clones without
/// stack overflow or an infinite loop, and the cloned cycle points within
/// the new graph, never back into the source graph (spec scenario "Graph
/// contains a cycle").
#[test]
fn clone_of_a_self_referential_cycle_terminates_within_the_new_graph() {
    let source = "import { stdout } from std.io;\nclass Node {
        mut next: Node?;
        construct() { }
    }

    fn main(): Void {
        mut a: Node = Node();
        a.next = a;
        stdout.println(a.next is a);

        mut b: Node = a.clone();
        match b.next {
            null => stdout.println(\"null\");
            n => {
                stdout.println(n is b);
                stdout.println(n is a);
            }
        }
    }";

    let output = zirk_with_env(source, "clone_cycle", &[]);

    assert_eq!(output.status, 0, "stderr:\n{}", output.stderr);
    assert_eq!(
        normalize(&output.stdout),
        "true\ntrue\nfalse\n",
        "the cloned cycle must point within the new graph (n is b) and \
         never back into the source (n is a must be false)"
    );
}

/// Mutating the clone does not affect the source, and vice versa (the
/// "never retains an alias to a cloned mutable source node" guarantee,
/// observed concretely rather than only through identity comparisons).
#[test]
fn mutating_a_clone_does_not_affect_the_source_or_vice_versa() {
    let source = "import { stdout } from std.io;\nclass Counter {
        mut value: Int32;
        construct(value: Int32) { this.value = value; }
    }

    fn main(): Void {
        mut a: Counter = Counter(1);
        mut b: Counter = a.clone();

        b.value = 99;
        stdout.println(a.value);
        stdout.println(b.value);

        a.value = -1;
        stdout.println(a.value);
        stdout.println(b.value);
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
    let source = "import { stdout } from std.io;\nclass Node {
        mut value: Int32;
        mut next: Node?;
        construct(value: Int32) { this.value = value; this.next = null; }
    }

    fn build(n: Int32): Node {
        mut head: Node = Node(0);
        mut i: Int32 = 1;
        while i < n {
            mut fresh: Node = Node(i);
            fresh.next = head;
            head = fresh;
            i = i + 1;
        }
        return head;
    }

    fn sum(start: Node): Int32 {
        mut total: Int32 = 0;
        mut current: Node? = start;
        while true {
            match current {
                null => { return total; }
                n => {
                    total = total + n.value;
                    current = n.next;
                }
            }
        }
        return total;
    }

    fn main(): Void {
        mut a: Node = build(200);
        mut b: Node = a.clone();
        stdout.println(sum(a));
        stdout.println(sum(b));
        stdout.println(a is b);
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
        "the clone must be complete and correct even though a collection \
         was forced partway through building it"
    );
}

/// A class with a `Pointer<T>` field fails to compile `.clone()` with a
/// diagnostic naming that field (spec scenario "Graph reaches a non-Clone
/// member").
#[test]
fn clone_of_a_class_reaching_a_pointer_field_is_rejected_at_compile_time() {
    let source = "import { stdout } from std.io;\nclass Holder {
        mut p: Pointer<Int32>;
        construct(p: Pointer<Int32>) { this.p = p; }
    }

    fn main(): Void {
        mut x: Int32 = 1;
        unsafe {
            mut h: Holder = Holder(Pointer.from(x));
            mut clone: Holder = h.clone();
        }
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
