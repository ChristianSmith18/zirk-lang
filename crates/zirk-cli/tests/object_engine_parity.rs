//! Native/interpreter parity checks for the unified dynamic-object model.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_CASE: AtomicUsize = AtomicUsize::new(0);

fn interpreter_runner() -> PathBuf {
    if let Some(path) = std::env::var_os("ZIRK_REPRO") {
        return PathBuf::from(path);
    }
    let test_executable = std::env::current_exe().expect("test executable path");
    let profile = test_executable
        .parent()
        .and_then(Path::parent)
        .expect("Cargo test executable is under <profile>/deps");
    let runner = profile.join("examples").join(if cfg!(windows) {
        "interp_repro.exe"
    } else {
        "interp_repro"
    });
    assert!(
        runner.is_file(),
        "missing interpreter harness at {}; build it with `cargo build -p zirk-browser --example interp_repro` or set ZIRK_REPRO",
        runner.display()
    );
    runner
}

fn run_same_program(name: &str, program: &str) -> (Output, Output) {
    let case = NEXT_CASE.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "object_engine_parity_{}_{}_{}",
        std::process::id(),
        case,
        name
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated case directory");
    let source = dir.join(format!("{name}.zrk"));
    std::fs::write(&source, program).expect("write source fixture");
    let native = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg(&source)
        .current_dir(&dir)
        .output()
        .expect("run native compiler");
    let interpreted = Command::new(interpreter_runner())
        .arg(&source)
        .output()
        .expect("run interpreter harness");
    (native, interpreted)
}

fn normalize(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn interpreted_payload(output: &Output) -> String {
    normalize(&output.stdout)
        .lines()
        .skip(1)
        .filter(|line| !line.starts_with("[completion: "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_parity(name: &str, program: &str, expected: &str) {
    let (native, interpreted) = run_same_program(name, program);
    assert!(
        native.status.success(),
        "native `{name}` failed:\n{}\n{}",
        normalize(&native.stdout),
        normalize(&native.stderr)
    );
    assert!(
        interpreted.status.success(),
        "interpreter `{name}` failed:\n{}\n{}",
        normalize(&interpreted.stdout),
        normalize(&interpreted.stderr)
    );
    assert_eq!(
        normalize(&native.stdout),
        expected,
        "native `{name}` output"
    );
    let mut payload = interpreted_payload(&interpreted);
    if !payload.is_empty() {
        payload.push('\n');
    }
    assert_eq!(payload, expected, "interpreter `{name}` output");
}

#[test]
fn dynamic_object_patterns_and_rest_match_across_engines() {
    let program = r#"import { stdout } from std.io;
fn main(): Void {
    inmut profile: Object = { id: 7, name: "Ada", active: true };
    inmut { id, ... details } = profile;
    stdout.println(id as Int32);
    stdout.println(details.name as String);
    stdout.println(details.active as Boolean);
    inmut { id: profile_id, name: profile_name, ... flags } = profile;
    stdout.println(profile_id as Int32);
    stdout.println(profile_name as String);
    stdout.println(flags.active as Boolean);
}
"#;
    assert_parity("object_rest", program, "7\nAda\ntrue\n7\nAda\ntrue\n");
}

#[test]
fn contract_sealing_missing_reads_and_class_properties_match_across_engines() {
    for (name, source, expected) in [
        (
            "dynamic_object_named_read",
            include_str!("corpus/valid/dynamic_object_named_read.zrk"),
            include_str!("corpus/valid/dynamic_object_named_read.out"),
        ),
        (
            "json_stringify_dynamic",
            include_str!("corpus/valid/json_stringify_dynamic.zrk"),
            include_str!("corpus/valid/json_stringify_dynamic.out"),
        ),
        (
            "contract_conformance",
            include_str!("corpus/valid/dynamic_object_contract_conformance.zrk"),
            include_str!("corpus/valid/dynamic_object_contract_conformance.out"),
        ),
        (
            "callable_contract_operations",
            include_str!("corpus/valid/object_callable_contract_operations.zrk"),
            include_str!("corpus/valid/object_callable_contract_operations.out"),
        ),
        (
            "missing_property",
            include_str!("corpus/valid/dynamic_object_missing_read_catch.zrk"),
            include_str!("corpus/valid/dynamic_object_missing_read_catch.out"),
        ),
        (
            "class_dynamic_properties",
            include_str!("corpus/valid/class_dynamic_object_properties.zrk"),
            include_str!("corpus/valid/class_dynamic_object_properties.out"),
        ),
        (
            "dynamic_cast_mismatch",
            include_str!("corpus/valid/dynamic_cast_mismatch_catch.zrk"),
            include_str!("corpus/valid/dynamic_cast_mismatch_catch.out"),
        ),
        (
            "object_identity",
            include_str!("corpus/valid/structural_equality.zrk"),
            include_str!("corpus/valid/structural_equality.out"),
        ),
        (
            "spread_and_rest",
            include_str!("corpus/valid/spread_rest_operators.zrk"),
            include_str!("corpus/valid/spread_rest_operators.out"),
        ),
    ] {
        assert_parity(name, source, expected);
    }
}
