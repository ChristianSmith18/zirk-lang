//! Generated-code GC coverage for collection combinator results and nested
//! managed payloads. The low threshold forces collection while results are
//! held in task result boxes and after they are staged into output lists.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn each_and_each_settled_keep_nested_payloads_alive_across_real_gc() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gc_concurrent_each");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create isolated test workspace");
    std::fs::write(
        dir.join("main.zrk"),
        r#"import { stdout } from std.io;

record Leaf { text: String; }
record Payload { leaf: Leaf; }

class Churn {
    value: Int32;
    construct(value: Int32) { this.value = value; }
}

fn make_payload(value: Int32): Payload {
    if value == 2 {
        mut i = 0;
        while i < 12000 {
            mut discarded = Churn(i);
            i = i + 1;
        }
    }
    return Payload(leaf: Leaf(text: if value == 1 { "first payload" } else { "second payload" }));
}

fn churn_after_results(): Void {
    mut i = 0;
    while i < 12000 {
        mut discarded = Churn(i);
        i = i + 1;
    }
}

fn main(): Void {
    inmut::strict values: List<Int32> = [1, 2];
    mut results = Concurrent.each(values, (value: Int32): Payload => make_payload(value), 2);
    churn_after_results();
    for payload in results {
        stdout.println(payload.leaf.text);
    }

    mut outcomes = Concurrent.each_settled(values, (value: Int32): Payload => make_payload(value), 2);
    churn_after_results();
    for outcome in outcomes {
        match outcome {
            Outcome.Fulfilled(payload) => stdout.println(payload.leaf.text);
            Outcome.Rejected(error) => stdout.println("rejected: " + error.message());
            Outcome.Cancelled(_) => stdout.println("cancelled");
        }
    }
}
"#,
    )
    .expect("write collection-combinator GC fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_zirk"))
        .arg("run")
        .arg("main.zrk")
        .current_dir(&dir)
        .env("ZIRK_GC_THRESHOLD", "1")
        .output()
        .expect("compile and run collection-combinator GC fixture");

    assert!(
        output.status.success(),
        "Zirk process failed (status {:?})\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "first payload\nsecond payload\nfirst payload\nsecond payload\n",
        "nested references must remain live in each result boxes and in List<Outcome<Payload>> through actual collections"
    );
}
