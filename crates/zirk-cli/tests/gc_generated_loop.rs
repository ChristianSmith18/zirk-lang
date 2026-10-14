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
