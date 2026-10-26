//! Focused native/interpreter parity checks for numeric and temporal edges.
//!
//! Run `cargo build -p zirk-browser --example interp_repro` before this test.
//! The interpreter helper is an existing source-level harness and the native
//! side uses Cargo's `zirk` integration-test binary.

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

fn case_dir(name: &str) -> PathBuf {
    let index = NEXT_CASE.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "numeric_engine_parity_{name}_{}_{}",
        std::process::id(),
        index
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated case directory");
    dir
}

fn run_native(source: &Path, cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg(source)
        .current_dir(cwd)
        .output()
        .expect("run native compiler")
}

fn run_interpreter(source: &Path) -> Output {
    Command::new(interpreter_runner())
        .arg(source)
        .output()
        .expect("run existing interpreter harness")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

fn run_same_program(name: &str, program: &str) -> (Output, Output) {
    let dir = case_dir(name);
    let source = dir.join(format!("{name}.zrk"));
    std::fs::write(&source, program).expect("write test program");

    let native = run_native(&source, &dir);
    let interpreted = run_interpreter(&source);
    (native, interpreted)
}

fn interpreter_payload(output: &Output) -> String {
    let stdout = text(&output.stdout);
    let mut lines = stdout.lines();
    let header = lines.next().expect("interpreter harness header");
    assert!(header.starts_with("=== "), "unexpected header: {header:?}");
    let payload: Vec<_> = lines
        .filter(|line| !line.starts_with("[completion: "))
        .collect();
    payload.join("\n")
}

fn assert_success_parity(name: &str, program: &str, expected: &str) {
    let (native, interpreted) = run_same_program(name, program);
    assert!(
        native.status.success(),
        "native `{name}` failed ({}):\n{}\n{}",
        native.status,
        text(&native.stdout),
        text(&native.stderr)
    );
    assert!(
        interpreted.status.success(),
        "interpreter `{name}` failed ({}):\n{}\n{}",
        interpreted.status,
        text(&interpreted.stdout),
        text(&interpreted.stderr)
    );
    assert!(
        text(&interpreted.stdout).contains("[completion: Completed]"),
        "interpreter `{name}` did not report successful completion:\n{}",
        text(&interpreted.stdout)
    );

    let native_output = text(&native.stdout);
    let mut interpreted_output = interpreter_payload(&interpreted);
    if !interpreted_output.is_empty() {
        interpreted_output.push('\n');
    }
    assert_eq!(native_output, expected, "native output for `{name}`");
    assert_eq!(
        interpreted_output, expected,
        "interpreter output for `{name}`"
    );
    assert_eq!(
        native_output, interpreted_output,
        "engine parity for `{name}`"
    );
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

fn assert_failure_parity(name: &str, program: &str, expected_category: &str) {
    let (native, interpreted) = run_same_program(name, program);
    assert!(
        !native.status.success(),
        "native `{name}` unexpectedly succeeded"
    );

    let native_category = native_failure_category(&text(&native.stderr));
    assert_eq!(
        native_category, expected_category,
        "native `{name}` category"
    );

    let payload = interpreter_payload(&interpreted);
    let failure = payload
        .lines()
        .find(|line| line.starts_with("[failure: "))
        .unwrap_or_else(|| panic!("interpreter `{name}` has no structured failure: {payload}"));
    let mut fields = failure.trim_start_matches("[failure: ").splitn(3, ' ');
    let kind = fields.next().expect("failure kind");
    let category = fields.next().expect("failure category");
    assert_eq!(kind, "Runtime", "interpreter `{name}` failure kind");
    assert_eq!(category, expected_category, "interpreter `{name}` category");
}

#[test]
fn float_singleton_match_patterns_match_native_observations() {
    assert_success_parity(
        "float_singleton_match_patterns",
        r#"import { stdout } from std.io;
fn classify(value: Float64): String {
    return match value { 1.5 => "fraction"; 0.0 => "zero"; _ => "other"; };
}
fn main(): Void {
    stdout.println(classify(1.5));
    stdout.println(classify(-0.0));
    stdout.println(classify(2.5));
}
stdout.println(classify(1.5));
stdout.println(classify(-0.0));
stdout.println(classify(2.5));"#,
        "fraction\nzero\nother\n",
    );
}

#[test]
fn char_singleton_match_patterns_match_unicode_graphemes_in_both_engines() {
    assert_success_parity(
        "char_singleton_match_patterns",
        r#"import { stdout } from std.io;
fn classify(value: Char): String {
    return match value { '👩‍💻' => "emoji"; _ => "other"; };
}
fn main(): Void {
    stdout.println(classify('👩‍💻'));
    stdout.println(classify('x'));
}
stdout.println(classify('👩‍💻'));
stdout.println(classify('x'));"#,
        "emoji\nother\n",
    );
}

#[test]
fn temporal_singleton_match_patterns_match_in_both_engines() {
    assert_success_parity(
        "temporal_singleton_match_patterns",
        r#"import { stdout } from std.io;
fn classify_duration(value: Duration): String {
    return match value { 250ms => "short"; _ => "other"; };
}
fn classify_date(value: Date): String {
    return match value { Date(2026, 12, 25) => "holiday"; _ => "other"; };
}
fn classify_time(value: Time): String {
    return match value { Time(9, 30) => "morning"; _ => "other"; };
}
fn classify_datetime(value: DateTime): String {
    return match value { DateTime(Date(2026, 12, 25), Time(9, 30)) => "event"; _ => "other"; };
}
fn main(): Void {
    stdout.println(classify_duration(250ms));
    stdout.println(classify_date(Date(2026, 12, 25)));
    stdout.println(classify_time(Time(9, 30)));
    stdout.println(classify_datetime(DateTime(Date(2026, 12, 25), Time(9, 30))));
}
stdout.println(classify_duration(250ms));
stdout.println(classify_date(Date(2026, 12, 25)));
stdout.println(classify_time(Time(9, 30)));
stdout.println(classify_datetime(DateTime(Date(2026, 12, 25), Time(9, 30))));"#,
        "short\nholiday\nmorning\nevent\n",
    );
}

#[test]
fn inline_tagged_union_representation_prototype_crosses_calls_in_both_engines() {
    assert_success_parity(
        "inline_tagged_union_representation_prototype",
        r#"import { stdout } from std.io;
enum Value { Number(number: Int32), Text(text: String) }
fn render(value: Value): String {
    return match value {
        Value.Number(number) => number.to_string();
        Value.Text(text) => text;
    };
}
fn pass_through(value: Value): String { return render(value); }
fn main(): Void {
    stdout.println(pass_through(Value.Number(42)));
    stdout.println(pass_through(Value.Text("inline")));
}
stdout.println(pass_through(Value.Number(42)));
stdout.println(pass_through(Value.Text("inline")));"#,
        "42\ninline\n",
    );
}

#[test]
fn source_union_int32_string_injects_matches_and_crosses_calls_in_both_engines() {
    assert_success_parity(
        "source_union_int32_string",
        r#"import { stdout } from std.io;
type Value = Int32 | String;
fn identity(value: Value): Value { return value; }
fn classify(value: Value): String {
    return match value {
        42 => "number";
        "inline" => "text";
        _ => "other";
    };
}

fn main(): Void {
    stdout.println(classify(identity(42)));
    stdout.println(classify(identity("inline")));
}

stdout.println(classify(identity(42)));
stdout.println(classify(identity("inline")));"#,
        "number\ntext\n",
    );
}

#[test]
fn satisfies_expression_is_a_checked_runtime_no_op_in_both_engines() {
    assert_success_parity(
        "satisfies_expression",
        r#"import { stdout } from std.io;
fn keep(value: Int32): Int32 { return value satisfies Int32; }
fn main(): Void { stdout.println(keep(42)); }
stdout.println(keep(42));"#,
        "42\n",
    );
}

#[test]
fn numeric_and_temporal_edges_match_native_observations() {
    assert_success_parity(
        "contextual_result_projection",
        r#"import { stdout } from std.io;
fn main(): Void {
    mut success: Result<Int32, String> = Ok(7);
    mut failure: Result<Int32, String> = Error("hidden");
    stdout.println(success);
    stdout.println(failure);
    stdout.println(success ?? 0);
    stdout.println(match failure { Ok(value) => value.to_string(); Error(error) => error; });
}
main();"#,
        "7\n<Null>\n7\nhidden\n",
    );

    assert_success_parity(
        "float_to_int_edges",
        r#"import { stdout } from std.io;
fn main(): Void {
    stdout.println((1.9f as Int32).to_string());
    stdout.println((-1.9f as Int32).to_string());
    stdout.println((2147483520.0f as Int32).to_string());
    stdout.println((-2147483520.0f as Int32).to_string());
}
main();"#,
        "1\n-1\n2147483520\n-2147483520\n",
    );

    for (name, expression) in [
        ("float_to_int_positive_infinity", "(1.0f / 0.0f) as Int32"),
        ("float_to_int_negative_infinity", "(-1.0f / 0.0f) as Int32"),
        ("float_to_int_out_of_range", "(2147483648.0f as Int32)"),
        ("float_to_int_lower_edge", "(-2147483648.0f as Int32)"),
        (
            "float_to_uint64_upper_edge",
            "(18446744073709551616.0f as UInt64)",
        ),
    ] {
        let program = format!(
            "import {{ stdout }} from std.io;\nfn main(): Void {{ inmut value = {expression}; stdout.println(value); }}\nmain();"
        );
        assert_failure_parity(name, &program, "CastError");
    }

    assert_success_parity(
        "float_nan_generation",
        "import { stdout } from std.io;\nfn main(): Void { try { inmut nan = 0.0f / 0.0f; stdout.println(nan); } catch FloatNanError(e) { stdout.println(\"FloatNanError:\" + e.message()); } }\nmain();",
        "FloatNanError:Float operation produced an indeterminate result (NaN)\n",
    );

    assert_success_parity(
        "decimal_exponents_and_float_durations",
        r#"import { stdout } from std.io;
fn main(): Void {
    stdout.println((match Decimal.parse("1e999999") { Error(_) => true; Ok(_) => false; }).to_string());
    stdout.println((match Decimal.parse("1e-999999") { Error(_) => true; Ok(_) => false; }).to_string());
    stdout.println((match Decimal.parse("6.25e-2") { Ok(value) => value; Error(_) => Decimal(0); }).to_string());
    stdout.println((1.5f * 1s).to_string());
    stdout.println((-0.25f * 2s).to_string());
}
main();"#,
        "true\nfalse\n0.0625\n1.5s\n-500ms\n",
    );

    assert_failure_parity(
        "decimal_arithmetic_overflow",
        r#"import { stdout } from std.io;
fn main(): Void {
    inmut max: Decimal = match Decimal.parse("99999999999999999999999999999999999999") { Ok(value) => value; Error(_) => Decimal(0); };
    inmut result = max + max;
    stdout.println(result);
}
main();"#,
        "ArithmeticOverflowError",
    );

    assert_success_parity(
        "time_duration_wraparound",
        r#"import { stdout } from std.io;
fn main(): Void {
    stdout.println((Time(1, 0) - 2h).to_string());
    stdout.println((Time(23, 30) + 2h).to_string());
    stdout.println((Time(0, 0) - 1ns).to_string());
    stdout.println((Time(23, 59, 59, 999999999) + 1ns).to_string());
}
main();"#,
        "23:00:00\n01:30:00\n23:59:59.999999999\n00:00:00\n",
    );

    for (name, expression) in [
        ("date_upper_i32_bound", "Date(2147483647, 12, 31) + 1"),
        ("date_lower_i32_bound", "Date(-2147483648, 1, 1) - 1"),
    ] {
        let program = format!(
            "import {{ stdout }} from std.io;\nfn main(): Void {{ inmut value = {expression}; stdout.println(value); }}\nmain();"
        );
        assert_failure_parity(name, &program, "ArithmeticOverflowError");
    }
}
