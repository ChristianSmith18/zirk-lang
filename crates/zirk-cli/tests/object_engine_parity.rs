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
main();

"#;
    assert_parity("object_rest", program, "7\nAda\ntrue\n7\nAda\ntrue\n");
}

#[test]
fn scalar_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type Value = Int32 | String;
fn identity(value: Value): Value { return value; }
fn apply(transform: Fn(Value) => Value, value: Value): Value { return transform(value); }
fn choose(flag: Boolean): Value {
    if flag { return "ready"; }
    return 42;
}
fn checked(flag: Boolean): Value throws InvalidArgumentError {
    if flag { return "ready"; }
    throw InvalidArgumentError("missing value");
}
fn classify(value: Value): String {
    return match value { 42 => "number"; "ready" => "text"; _ => "other"; };
}
fn main(): Void {
    stdout.println(classify(identity(42)));
    stdout.println(classify(identity("ready")));
    stdout.println(classify(apply(identity, 42)));
    stdout.println(classify(apply(identity, "ready")));
    stdout.println(classify(choose(true)));
    stdout.println(classify(choose(false)));
    try {
        stdout.println(classify(checked(false)));
    } catch InvalidArgumentError(_) {
        stdout.println("caught");
    } finally {
        stdout.println("cleanup");
    }
}
main();
"#;
    assert_parity(
        "scalar_union",
        program,
        "number\ntext\nnumber\ntext\ntext\nnumber\ncaught\ncleanup\n",
    );
}

#[test]
fn class_reference_union_narrowing_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
class Dog {
    name: String;
    construct(name: String) { this.name = name; }
}
class Cat {
    name: String;
    construct(name: String) { this.name = name; }
}
type Animal = Dog | Cat;
fn describe(animal: Animal): String {
    if animal is Dog { return "dog " + animal.name; }
    if animal is Cat { return "cat " + animal.name; }
    return "unknown";
}
fn main(): Void {
    stdout.println(describe(Dog("Rex")));
    stdout.println(describe(Cat("Miso")));
}
main();
"#;
    assert_parity("class_reference_union", program, "dog Rex\ncat Miso\n");
}

#[test]
fn mixed_literal_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type State = "ready" | "failed" | 0 | 1 | true | false;
fn describe(value: State): String {
    return match value {
        "ready" => "ready";
        "failed" => "failed";
        0 => "zero";
        1 => "one";
        true => "true";
        false => "false";
    };
}
fn main(): Void {
    stdout.println(describe("ready"));
    stdout.println(describe(1));
    stdout.println(describe(false));
}
main();
"#;
    assert_parity("literal_union", program, "ready\none\nfalse\n");
}

#[test]
fn nullable_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
type Value = Int32 | String?;
fn describe(value: Value): String {
    return match value {
        null => "empty";
        9 => "number";
        "zirk" => "text";
        _ => "other";
    };
}
fn main(): Void {
    stdout.println(describe(null));
    stdout.println(describe(9));
    stdout.println(describe("zirk"));
}
main();
"#;
    assert_parity("nullable_union", program, "empty\nnumber\ntext\n");
}

#[test]
fn enum_singleton_union_matching_has_native_interpreter_parity() {
    let program = r#"import { stdout } from std.io;
enum Shade { Red, Blue }
type Color = const(Shade.Red) | const(Shade.Blue);
fn describe(value: Color): String {
    return match value {
        Shade.Red => "warm";
        Shade.Blue => "cool";
    };
}
fn main(): Void {
    stdout.println(describe(Shade.Red));
    stdout.println(describe(Shade.Blue));
}
main();
"#;
    assert_parity("enum_singleton_union", program, "warm\ncool\n");
}

#[test]
fn numeric_and_temporal_singleton_unions_match_across_engines() {
    let program = r#"import { stdout } from std.io;
type Wait = 250ms | 1s;
type Day = const(Date(2026, 12, 25)) | const(Date(2027, 1, 1));
type Ratio = const(1.5f64) | const(2.5f64);
type Money = const(1.25) | const(2.5);
type Letter = const('a') | const('b');
fn wait(value: Wait): String { return match value { 250ms => "short"; 1s => "long"; }; }
fn day(value: Day): String { return match value { Date(2026, 12, 25) => "holiday"; Date(2027, 1, 1) => "new year"; }; }
fn ratio(value: Ratio): String { return match value { 1.5f64 => "first"; 2.5f64 => "second"; }; }
fn money(value: Money): String { return match value { 1.25 => "first"; 2.5 => "second"; }; }
fn letter(value: Letter): String { return match value { 'a' => "first"; 'b' => "second"; }; }
fn main(): Void {
    stdout.println(wait(1s));
    stdout.println(day(Date(2027, 1, 1)));
    stdout.println(ratio(2.5f64));
    stdout.println(money(2.5));
    stdout.println(letter('b'));
}
main();
"#;
    assert_parity(
        "numeric_temporal_singleton_unions",
        program,
        "long\nnew year\nsecond\nsecond\nsecond\n",
    );
}

#[test]
fn structural_union_discriminants_and_narrowing_match_across_engines() {
    let program = r#"import { stdout } from std.io;
type NumberEntry = { kind: "number"; value: Int32 };
type TextEntry = { kind: "text"; value: String };
type Entry = NumberEntry | TextEntry;
fn show(entry: Entry): Void {
    if entry.kind == "number" { stdout.println("number {entry.value}"); }
    else if entry.kind == "text" { stdout.println("text {entry.value}"); }
}
fn main(): Void {
    inmut first: Entry = { kind: "number", value: 42 };
    inmut second: Entry = { kind: "text", value: "ready" };
    show(first);
    show(second);
}
main();
"#;
    assert_parity(
        "structural_union_discriminants",
        program,
        "number 42\ntext ready\n",
    );
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
