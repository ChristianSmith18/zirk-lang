//! Full valid-corpus differential check for the opt-in LLVM codegen levels.
//!
//! Run manually with:
//! `cargo test -p zirk-browser --test native_optimization_parity -- --ignored --nocapture`

use std::path::{Path, PathBuf};
use std::process::Command;

use zirk_cli::driver::CodegenOptimizationLevel;
use zirk_diagnostics::{DiagnosticSink, RenderStyle};
use zirk_interpreter::{
    CapturedHost, ExecutionResult, Interpreter, InterpreterLimits, OutputDestination,
};

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn interpreter_stream(host: &CapturedHost, destination: OutputDestination) -> String {
    host.events()
        .iter()
        .filter(|event| event.destination == destination && !event.truncated)
        .map(|event| event.text.as_str())
        .collect()
}

fn native_failure_category(stderr: &str) -> &'static str {
    let message = stderr.to_ascii_lowercase();
    if message.contains("invalid cast") {
        "CastError"
    } else if message.contains("arithmetic overflow") {
        "ArithmeticOverflowError"
    } else if message.contains("nan") {
        "FloatNanError"
    } else {
        panic!("unclassified native runtime failure: {stderr}")
    }
}

fn temporary_root() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("zirk-native-parity-{}-{nonce}", std::process::id()))
}

fn expose_runtime_to_the_driver() -> Option<PathBuf> {
    let executable = std::env::current_exe().expect("test executable path");
    let dependency_dir = executable.parent().expect("test executable directory");
    let target_dir = dependency_dir.parent().expect("Cargo target directory");
    let source = target_dir.join(if cfg!(windows) {
        "zirk_runtime.lib"
    } else {
        "libzirk_runtime.a"
    });
    assert!(
        source.is_file(),
        "build the runtime with `cargo build -p zirk-runtime`"
    );
    let destination = dependency_dir.join(source.file_name().expect("runtime filename"));
    if destination.exists() {
        return None;
    }
    std::fs::copy(source, &destination).expect("make runtime visible to the test-process driver");
    Some(destination)
}

#[test]
#[ignore = "manual full valid-corpus native/interpreter/LLVM-level parity gate"]
fn candidate_llvm_level_matches_interpreter_and_unoptimized_native_corpus() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../zirk-cli/tests/corpus/valid");
    let mut sources: Vec<_> = std::fs::read_dir(&corpus)
        .expect("valid corpus")
        .map(|entry| entry.expect("corpus entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "zrk"))
        .collect();
    sources.sort();

    let root = temporary_root();
    std::fs::create_dir_all(&root).expect("temporary parity directory");
    let copied_runtime = expose_runtime_to_the_driver();
    let mut compared = 0;
    let mut capability_exclusions = 0;

    for source in sources {
        let mut sink = DiagnosticSink::new();
        let frontend = zirk_cli::frontend::run_frontend(&source, &mut sink).unwrap_or_else(|| {
            panic!(
                "frontend rejected {}:\n{}",
                source.display(),
                sink.render(RenderStyle::Human)
            )
        });
        let ir = zirk_ir::lower(&frontend.program, &frontend.checked);
        let mut interpreter =
            Interpreter::new(&ir, InterpreterLimits::default()).unwrap_or_else(|failure| {
                panic!(
                    "interpreter setup failed for {}: {failure:?}",
                    source.display()
                )
            });
        let mut host = CapturedHost::default();
        match interpreter.run_main_with_host(&mut host) {
            ExecutionResult::Completed { .. } => {}
            ExecutionResult::CapabilityFailure(_) => {
                capability_exclusions += 1;
                continue;
            }
            other => panic!(
                "interpreter did not complete {}: {other:?}",
                source.display()
            ),
        }
        let expected_stdout = interpreter_stream(&host, OutputDestination::Stdout);
        let expected_stderr = interpreter_stream(&host, OutputDestination::Stderr);
        let stem = source.file_stem().expect("fixture stem").to_string_lossy();

        for (label, optimization) in [
            ("O0", CodegenOptimizationLevel::None),
            ("less", CodegenOptimizationLevel::Less),
            ("default", CodegenOptimizationLevel::Default),
            ("aggressive", CodegenOptimizationLevel::Aggressive),
        ] {
            let output_dir = root.join(format!("{stem}-{label}"));
            let compilation =
                zirk_cli::driver::compile_with_optimization(&source, &output_dir, optimization);
            let executable = compilation.executable.unwrap_or_else(|| {
                panic!(
                    "{label} compile failed for {}:\n{}",
                    source.display(),
                    compilation.sink.render(RenderStyle::Human)
                )
            });
            let result = Command::new(&executable)
                .output()
                .unwrap_or_else(|error| panic!("could not run {}: {error}", executable.display()));
            assert!(
                result.status.success(),
                "{label} run failed for {} with {:?}:\n{}",
                source.display(),
                result.status.code(),
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                normalized(&String::from_utf8_lossy(&result.stdout)),
                normalized(&expected_stdout),
                "stdout mismatch at {label} for {}",
                source.display()
            );
            assert_eq!(
                normalized(&String::from_utf8_lossy(&result.stderr)),
                normalized(&expected_stderr),
                "stderr mismatch at {label} for {}",
                source.display()
            );
        }
        compared += 1;
    }

    let failure_source = root.join("float_to_int_out_of_range.zrk");
    std::fs::write(
        &failure_source,
        "import { stdout } from std.io;\nfn main(): Void { stdout.println((2147483648.0f as Int32).to_string()); }",
    )
    .expect("write runtime-failure parity fixture");
    let mut sink = DiagnosticSink::new();
    let frontend = zirk_cli::frontend::run_frontend(&failure_source, &mut sink).unwrap();
    let ir = zirk_ir::lower(&frontend.program, &frontend.checked);
    let mut interpreter = Interpreter::new(&ir, InterpreterLimits::default()).unwrap();
    let mut host = CapturedHost::default();
    let expected_failure = match interpreter.run_main_with_host(&mut host) {
        ExecutionResult::RuntimeFailure(failure) => failure.category,
        other => panic!("expected interpreter runtime failure, got {other:?}"),
    };
    assert_eq!(expected_failure, "CastError");
    for (label, optimization) in [
        ("O0", CodegenOptimizationLevel::None),
        ("less", CodegenOptimizationLevel::Less),
        ("default", CodegenOptimizationLevel::Default),
        ("aggressive", CodegenOptimizationLevel::Aggressive),
    ] {
        let output_dir = root.join(format!("runtime-failure-{label}"));
        let compilation =
            zirk_cli::driver::compile_with_optimization(&failure_source, &output_dir, optimization);
        let executable = compilation.executable.unwrap_or_else(|| {
            panic!(
                "{label} failure fixture did not compile:\n{}",
                compilation.sink.render(RenderStyle::Human)
            )
        });
        let result = Command::new(&executable)
            .output()
            .expect("run failure parity fixture");
        assert!(
            !result.status.success(),
            "{label} failure unexpectedly succeeded"
        );
        assert_eq!(
            native_failure_category(&String::from_utf8_lossy(&result.stderr)),
            expected_failure,
            "failure category at {label}"
        );
    }

    assert!(
        compared >= 20,
        "only {compared} fixtures were interpreter-compatible"
    );
    eprintln!(
        "compared {compared} fixtures at O0, LLVM Less, Default, and Aggressive; skipped {capability_exclusions} unsupported interpreter capabilities"
    );
    std::fs::remove_dir_all(root).expect("clean temporary parity directory");
    if let Some(runtime) = copied_runtime {
        std::fs::remove_file(runtime).expect("remove copied runtime");
    }
}
