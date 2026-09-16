//! Black-box coverage for the intentionally narrow `zirk format` command.

use std::path::PathBuf;
use std::process::Command;

fn compiler() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable");
    let profile = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("Cargo layout");
    profile.join(if cfg!(windows) { "zirk.exe" } else { "zirk" })
}

fn workspace(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("workspace");
    dir
}

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(compiler())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run zirk format")
}

const UNFORMATTED: &str =
    "fn main(): Void {\n    inmut text = '''\n                 hello\n                 ''';\n}\n";
const CANONICAL: &str =
    "fn main(): Void {\n    inmut text =\n        '''\n        hello\n        ''';\n}\n";

#[test]
fn format_rewrites_triple_strings_and_is_idempotent() {
    let dir = workspace("format_rewrites");
    let file = dir.join("sample.zrk");
    std::fs::write(&file, UNFORMATTED).expect("fixture");

    assert!(run(&dir, &["format", "sample"]).status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), CANONICAL);

    assert!(run(&dir, &["format", "sample.zrk"]).status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), CANONICAL);
}

#[test]
fn check_reports_without_writing() {
    let dir = workspace("format_check");
    let file = dir.join("sample.zrk");
    std::fs::write(&file, UNFORMATTED).expect("fixture");

    let output = run(&dir, &["format", "--check", "sample.zrk"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs formatting"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), UNFORMATTED);
}

#[test]
fn syntax_errors_are_reported_without_writing() {
    let dir = workspace("format_invalid");
    let file = dir.join("broken.zrk");
    let input = "fn main(: Void {\n";
    std::fs::write(&file, input).expect("fixture");

    let output = run(&dir, &["format", "broken.zrk"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error["));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), input);
}

#[test]
fn format_canonicalizes_general_source_layout() {
    let dir = workspace("format_other_style");
    let file = dir.join("plain.zrk");
    let input = "fn main(): Void {  stdout.println(  1 ); }\n";
    std::fs::write(&file, input).expect("fixture");

    assert!(run(&dir, &["format", "plain.zrk"]).status.success());
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "fn main(): Void { stdout.println(1); }\n"
    );
}

#[test]
fn format_preserves_class_construction_sugar() {
    let dir = workspace("format_class_construction_sugar");
    let file = dir.join("sugar.zrk");
    let input = "class User { construct(public name: String) { } }\nclass Marker { }\nfn main(): Void { mut user = User(\"Ana\"); mut marker = Marker(); }\n";
    std::fs::write(&file, input).expect("fixture");

    assert!(run(&dir, &["format", "sugar.zrk"]).status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), input);
}

#[test]
fn directory_targets_are_recursive_and_an_invalid_sibling_does_not_block_them() {
    let dir = workspace("format_directory");
    let nested = dir.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let valid = nested.join("sample.zrk");
    let invalid = dir.join("broken.zrk");
    std::fs::write(&valid, UNFORMATTED).unwrap();
    std::fs::write(&invalid, "fn main(: Void {\n").unwrap();

    let output = run(&dir, &["format", "."]);
    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(valid).unwrap(), CANONICAL);
    assert!(String::from_utf8_lossy(&output.stderr).contains("error["));
}

#[test]
fn diff_and_check_never_write_the_target() {
    let dir = workspace("format_diff");
    let file = dir.join("sample.zrk");
    std::fs::write(&file, UNFORMATTED).unwrap();

    let diff = run(&dir, &["format", "--diff", "sample.zrk"]);
    assert!(diff.status.success());
    assert!(String::from_utf8_lossy(&diff.stdout).contains("---"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), UNFORMATTED);

    let check = run(&dir, &["format", "--check", "sample.zrk"]);
    assert!(!check.status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), UNFORMATTED);
}
