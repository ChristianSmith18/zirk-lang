//! End-to-end GC regression for generated loop back-edges. This compiles and
//! runs Zirk through the real CLI and LLVM backend with a deliberately low
//! collection threshold.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn generated_allocation_loop_preserves_a_reachable_object_graph_across_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_generated_loop");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;

class Leaf {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}

class Root {
    leaf: Leaf;
    construct(leaf: Leaf) { this.leaf = leaf; }
}

class Churn {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}

fn main(): Void {
    mut kept = Root(Leaf(7391));
    mut i = 0;
    while i < 20000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(kept.leaf.value);
}
main();
"#,
    )
    .expect("write generated-code fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .output()
        .expect("compile and run Zirk fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "7391\n",
        "the live object reachable through Root.leaf must survive collections triggered by allocations in the generated loop"
    );
}

#[test]
fn generic_outcome_with_nested_managed_payload_survives_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_generic_outcome");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        include_str!("corpus/valid/concurrency_outcome_nested_payload.zrk"),
    )
    .expect("write generic Outcome fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .output()
        .expect("compile and run generic Outcome fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "kept through GC\n",
        "the payload reachable through Outcome<T> must survive collections triggered while the outcome is live"
    );
}

#[test]
fn tagged_union_representation_prototype_traces_only_active_enum_payload_under_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_tagged_union_prototype");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
class Churn { value: Int32; construct(value: Int32) { this.value = value; } }
enum Value { Number(value: Int64), Text(value: String) }
fn identity(value: Value): Value { return value; }
fn render(value: Value): String {
    return match value {
        Value.Number(number) => number.to_string();
        Value.Text(text) => text;
    };
}

fn main(): Void {
    mut live_text: Value = identity(Value.Text("live across collection"));
    mut scalar: Value = identity(Value.Number(68719476736));
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(render(live_text));
    stdout.println(render(scalar));
}
main();
"#,
    )
    .expect("write tagged payload fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("run tagged payload under forced GC");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "live across collection\n68719476736\n"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"),
        "the test must observe an actual collection:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn int32_string_union_preserves_managed_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_int32_string_union");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
class Churn { value: Int32; construct(value: Int32) { this.value = value; } }
type Value = Int32 | String;
fn identity(value: Value): Value { return value; }
fn classify(value: Value): String {
    return match value {
        42 => "number";
        "live across collection" => "text";
        _ => "other";
    };
}

fn hold(text: Value, number: Value): Void {
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(classify(text));
    stdout.println(classify(number));
}
fn main(): Void {
    hold(identity("live across collection"), identity(42));
}
main();
"#,
    )
    .expect("write union payload fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("run source union under forced GC");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "text\nnumber\n"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"),
        "the test must observe an actual collection:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn union_nested_in_enum_payload_preserves_managed_string_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_union_nested_in_enum_payload");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
class Churn { value: Int32; construct(value: Int32) { this.value = value; } }
type Inner = Int32 | String;
enum Envelope { Empty, Nested(value: Inner) }
type Outer = Int32 | Envelope;
fn identity(value: Outer): Outer { return value; }
fn render(value: Outer): String {
    return match value {
        42 => "outer number";
        Envelope.Empty => "empty";
        Envelope.Nested(inner) => match inner {
            7 => "inner number";
            "live nested enum payload" => "nested string";
            _ => "other inner";
        };
        _ => "other outer";
    };
}
fn hold(value: Outer): Void {
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(render(value));
}
fn main(): Void { hold(identity(Envelope.Nested("live nested enum payload"))); }
main();
"#,
    )
    .expect("write nested enum union fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("run nested enum union under forced GC");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "nested string\n"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"),
        "the test must observe an actual collection:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn union_in_object_fields_preserves_managed_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_union_in_object_fields");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
type Value = Int32 | String;
class Holder { value: Value; construct(value: Value) { this.value = value; } read(): Value { return this.value; } }
class Churn { value: Int32; construct(value: Int32) { this.value = value; } }
fn classify(value: Value): String {
    return match value {
        42 => "number";
        "live in object field" => "text";
        _ => "other";
    };
}

fn main(): Void {
    mut text = Holder("live in object field");
    mut number = Holder(42);
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(classify(text.read()));
    stdout.println(classify(number.read()));
}
main();
"#,
    )
    .expect("write union object-field fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("run object-field union under forced GC");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "text\nnumber\n"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"),
        "the test must observe an actual collection:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn managed_atomic_remains_rooted_across_real_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_managed_atomic");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;

class Churn { value: Int32; construct(value: Int32) { this.value = value; } }

fn main(): Void {
    mut kept = Atomic(7391);
    mut i = 0;
    while i < 20000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(kept.load().to_string());
}
main();
"#,
    )
    .expect("write Atomic GC fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .output()
        .expect("compile and run Atomic GC fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "7391\n",
        "a live managed Atomic object must survive repeated collections"
    );
}

#[test]
fn settled_outcome_preserves_nested_payloads_across_real_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_concurrent_settled_nested");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        include_str!("corpus/valid/concurrent_settled_nested_payload.zrk"),
    )
    .expect("write settled payload fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .output()
        .expect("compile and run settled payload fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "nested settled one\nnested settled two\n",
        "nested references reachable through settled generic outcomes must remain live through collections"
    );
}

#[test]
fn first_winner_nested_payload_stays_rooted_while_loser_is_cancelled_under_real_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_concurrent_first_nested");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;

class Leaf {
    label: String;
    construct(label: String) { this.label = label; }
}

class Payload {
    leaf: Leaf;
    construct(leaf: Leaf) { this.leaf = leaf; }
}

class Churn {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}

fn wait_for_loser(): Void {
    Timer.sleep(500ms);
}

fn main(): Void {
    mut winner: Payload = Concurrent.of(
        (): Payload => {
            Timer.sleep(150ms);
            return Payload(Leaf("winner survived cancellation GC"));
        },
        (): Payload => {
            try {
                stdout.println("loser started");
                wait_for_loser();
            } finally {
                mut i = 0;
                while i < 10000 {
                    mut discarded = Churn(i);
                    i = i + 1;
                }
                stdout.println("loser cleanup completed");
            }
            return Payload(Leaf("loser"));
        },
    ).first();
    stdout.println(winner.leaf.label);
}
main();
"#,
    )
    .expect("write first nested-payload fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .output()
        .expect("compile and run first nested-payload fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "loser started\nloser cleanup completed\nwinner survived cancellation GC\n",
        "the first() winner's nested managed payload must remain rooted through loser GC and cancellation"
    );
}

#[test]
fn captured_union_preserves_managed_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_union_in_closure_capture");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
type Value = Int32 | String;
class Churn { value: Int32; construct(value: Int32) { this.value = value; } }
fn main(): Void {
    inmut captured: Value? = "captured through collections";
    inmut read: Fn() => Value? = (): Value? => captured;
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(match read() { "captured through collections" => "kept"; null => "null"; _ => "lost"; });
}

main();
"#,
    )
    .expect("write captured union fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("run captured union under forced GC");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "kept\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"));
}

#[test]
fn collection_stored_union_preserves_managed_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_union_in_list");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
type Value = Int32 | String;
class Churn { value: Int32; construct(value: Int32) { this.value = value; } }
fn main(): Void {
    mut values: List<Value> = List();
    values.add("stored in list across collections");
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    stdout.println(match values[0] {
        "stored in list across collections" => "kept";
        _ => "lost";
    });
}
main();
"#,
    )
    .expect("write list-stored union fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("run list-stored union under forced GC");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "kept\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"));
}

#[test]
fn channel_stored_union_preserves_managed_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_channel_union");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
type Value = Int32 | String;
class Churn {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}
fn fallback(): Value { return 0; }
fn main(): Void {
    mut channel: Channel<Value> = Channel(1);
    channel.send("kept in channel");
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
    mut received: Value = channel.receive() ?? fallback();
    stdout.println(match received {
        "kept in channel" => "kept in channel";
        _ => "lost";
    });
}
main();
"#,
    )
    .expect("write channel union fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("compile and run Zirk fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "kept in channel\n",
        "the channel queue must retain the active managed union payload through forced collections"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"));
}

#[test]
fn suspended_task_capture_preserves_union_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_suspended_task_union");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
type Value = Int32 | String;
class Churn {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}
fn churn(): Void {
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
}
fn main(): Void {
    inmut payload: Value = "kept in suspended task";
    concurrent {
        spawn {
            Timer.sleep(2ms);
            stdout.println(match payload {
                "kept in suspended task" => "kept in suspended task";
                _ => "lost";
            });
        };
        spawn churn();
    }
}
main();
"#,
    )
    .expect("write suspended-task union fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("compile and run suspended-task fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "kept in suspended task\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"));
}

#[test]
fn exception_field_union_preserves_managed_payload_across_forced_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_exception_union");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;
type Value = Int32 | String;
class Churn {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}
class UnionError implements RuntimeError {
    inmut value: Value;
    construct(value: Value) { this.value = value; }
    #override message(): String { return "union error"; }
    #override code(): String { return "UNION_ERROR"; }
    #override cause(): Error? { return null; }
    #override stack_trace(): StackTrace { return StackTrace(); }
}
fn main(): Void {
    try {
        throw UnionError("kept in exception");
    } catch UnionError(error) {
        mut i = 0;
        while i < 12000 {
            mut discarded = Churn(i);
            i = i + 1;
        }
        stdout.println(match error.value {
            "kept in exception" => "kept in exception";
            _ => "lost";
        });
    }
}
main();
"#,
    )
    .expect("write exception union fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .env("ZIRK_GC_TRACE", "1")
        .output()
        .expect("compile and run exception fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "kept in exception\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("[zirk-gc] collection"));
}
