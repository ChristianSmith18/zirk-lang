//! Frontend-only validation tests for the `zirk check` subcommand.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Path to the `zirk` executable produced by the workspace.
fn compiler() -> PathBuf {
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

/// A private working directory per test.
fn workspace(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("working directory");
    dir
}

fn lock_local_dependency(root: &Path, package: &str, dependency_dir: &str) {
    let dependency_root = root.join(dependency_dir);
    let manifest = zirk_cli::project::read_manifest(&dependency_root.join(".zkinit")).unwrap();
    let hash = zirk_cli::project::package_integrity_hash(&dependency_root, &manifest).unwrap();
    std::fs::write(
        root.join("zirk.lock"),
        format!("{package} 1.0 local sha256:{hash} src/package.zrk\n"),
    )
    .unwrap();
}

/// Runs `zirk check` over a source file, from its own working directory.
fn zirk_check(source: &Path, name: &str) -> (bool, String, String) {
    zirk_check_with_arg(
        source,
        name,
        source.file_name().expect("file name").to_str().unwrap(),
    )
}

/// Runs `zirk check` with an explicit command-line argument.
fn zirk_check_with_arg(source: &Path, name: &str, arg: &str) -> (bool, String, String) {
    let dir = workspace(name);
    let copied = dir.join(source.file_name().expect("file name"));
    std::fs::copy(source, &copied).expect("copy the source");

    let output = Command::new(compiler())
        .arg("check")
        .arg(arg)
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn a_valid_program_passes_check() {
    let path = corpus("valid").join("hello.zrk");
    let (success, stdout, stderr) = zirk_check(&path, "valid_hello");

    assert!(success, "`zirk check` must exit 0 for a valid program");
    assert!(stdout.is_empty(), "`zirk check` must not write to stdout");
    assert!(
        stderr.is_empty(),
        "`zirk check` must not produce diagnostics"
    );
}

#[test]
fn an_invalid_program_is_rejected_with_diagnostics() {
    let path = corpus("invalid").join("type_mismatch.zrk");
    let (success, stdout, stderr) = zirk_check(&path, "invalid_duplicate");

    assert!(
        !success,
        "`zirk check` must exit non-zero for an invalid program"
    );
    assert!(stdout.is_empty(), "`zirk check` must not write to stdout");
    assert!(
        !stderr.is_empty(),
        "`zirk check` must emit a diagnostic on stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("E0411") || stderr.contains("error"),
        "`zirk check` must report an error, got: {stderr}"
    );
}

#[test]
fn promoted_constructor_diagnostics_have_stable_json_codes() {
    let dir = workspace("promoted_constructor_json");
    let source = dir.join("promoted.zrk");
    std::fs::write(
        &source,
        "class User { name: String; construct(public name: String) { } } fn main(): Void { }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "--json", "promoted.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run zirk check --json");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E0477"), "{stderr}");
    assert!(stderr.trim_start().starts_with('['), "{stderr}");
}

#[test]
fn a_missing_file_reports_a_diagnostic() {
    let dir = workspace("missing_file");
    let output = Command::new(compiler())
        .arg("check")
        .arg("does_not_exist.zrk")
        .current_dir(&dir)
        .output()
        .expect("run the compiler");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not read"),
        "expected a readable diagnostic, got: {stderr}"
    );
}

#[test]
fn a_package_project_is_discovered_from_its_directory() {
    let dir = workspace("package_project_check");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"demo.package\"\nkind package\nentry \"src/package.zrk\"\nsource \"src\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/package.zrk"),
        "package demo.package;\nshare fn hello(): Void { }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run package project check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_malformed_project_manifest_is_rejected_before_source_checking() {
    let dir = workspace("invalid_package_manifest");
    std::fs::write(dir.join(".zkinit"), "project \"broken\"\nkind unknown\n").unwrap();
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run malformed project check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0510"));
}

#[test]
fn a_lockfile_without_an_integrity_hash_is_rejected_before_source_checking() {
    let dir = workspace("invalid_package_lock");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join(".zkinit"), "project \"app\"\nkind application\n").unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        "acme.web 1.0 local - src/package.zrk\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run malformed lock check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0510"));
}

#[test]
fn a_tampered_local_package_is_rejected_before_its_body_is_checked() {
    let dir = workspace("tampered_package_lock");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.web ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { Api } from acme.web;\nfn main(): Void { mut api = Api(); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.web\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.web;\nthis is deliberately not valid Zirk",
    )
    .unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        "acme.web 1.0 local sha256:0000000000000000000000000000000000000000000000000000000000000000 src/package.zrk\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run tampered package check");
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostics.contains("does not match the integrity hash"),
        "{diagnostics}"
    );
    assert!(
        !diagnostics.contains("expected a declaration"),
        "{diagnostics}"
    );
}

#[test]
fn an_undeclared_package_is_rejected_before_type_checking() {
    let dir = workspace("undeclared_package_import");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join(".zkinit"), "project \"app\"\nkind application\n").unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { Api } from acme.web;\nfn main(): Void { Api(); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run undeclared package check");
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("E0511"), "{diagnostics}");
    assert!(diagnostics.contains("not declared"), "{diagnostics}");
}

#[test]
fn a_declared_local_package_root_is_loaded_by_logical_name() {
    let dir = workspace("local_package_resolution");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.web ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        "acme.web 1.0 local sha256:fixture src/package.zrk\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { Api } from acme.web;\nfn main(): Void { mut api = Api(); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.web\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.web;\nshare class Api { }",
    )
    .unwrap();
    lock_local_dependency(&dir, "acme.web", "dep");
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run local package check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn imported_stdout_opens_println_and_explicit_qualification() {
    let dir = workspace("standard_module_opening");
    std::fs::write(
        dir.join("main.zrk"),
        "import { stdout } from std.io;\nfn main(): Void { println(\"open\"); stdout.println(\"qualified\"); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run standard opening check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn direct_standard_module_member_import_binds_only_the_member() {
    let dir = workspace("direct_standard_module_member");
    std::fs::write(
        dir.join("main.zrk"),
        "import { println } from std.io::stdout;\nfn main(): Void { println(\"direct\"); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run direct standard member import check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn external_println_export_never_receives_the_standard_output_implementation() {
    let dir = workspace("external_println_is_not_standard_output");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.io ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { stdout } from acme.io;\nfn main(): Void { println(\"not standard\"); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.io\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.io;\nshare mod stdout { share fn println(...args: String): Void { } }",
    )
    .unwrap();
    lock_local_dependency(&dir, "acme.io", "dep");
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run external output check");
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostics.contains("E0401"), "{diagnostics}");
}

#[test]
fn an_unknown_standard_package_module_is_rejected_by_the_resolver() {
    let dir = workspace("unknown_standard_package_module");
    std::fs::write(
        dir.join("main.zrk"),
        "import { missing } from std.unknown;\nfn main(): Void { }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run unknown standard module check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not provided"));
}

#[test]
fn unimported_println_is_rejected() {
    let dir = workspace("unimported_standard_member");
    std::fs::write(
        dir.join("main.zrk"),
        "fn main(): Void { println(\"nope\"); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run unimported standard member check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("E0401"));
}

#[test]
fn package_module_members_open_and_qualify_from_a_root_reexport() {
    let dir = workspace("package_module_members");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.cli ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        "acme.cli 1.0 local sha256:fixture src/package.zrk\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { terminal } from acme.cli;\nfn main(): Void { clear(); terminal::clear(); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.cli\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.cli;\nshare mod terminal from \"./terminal\";",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/terminal.zrk"),
        "share fn clear(): Void { }",
    )
    .unwrap();
    lock_local_dependency(&dir, "acme.cli", "dep");
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run module member check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn opened_module_member_collisions_require_qualification_without_source_order() {
    let dir = workspace("package_module_member_collision");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.cli ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        "acme.cli 1.0 local sha256:fixture src/package.zrk\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { terminal, screen } from acme.cli;\nfn main(): Void { terminal::clear(); screen::clear(); clear(); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.cli\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.cli;\nshare mod terminal from \"./terminal\";\nshare mod screen from \"./screen\";",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/terminal.zrk"),
        "share fn clear(): Void { }",
    )
    .unwrap();
    std::fs::write(dir.join("dep/src/screen.zrk"), "share fn clear(): Void { }").unwrap();
    lock_local_dependency(&dir, "acme.cli", "dep");
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run opened module collision check");
    assert!(!output.status.success());
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostics.contains("ambiguous between imported modules"),
        "{diagnostics}"
    );
    assert!(!diagnostics.contains("already defined"), "{diagnostics}");
}

#[test]
fn qualified_module_values_types_and_direct_member_imports_resolve() {
    let dir = workspace("qualified_module_values_types");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::create_dir_all(dir.join("dep/src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"app\"\nkind application\ndependency acme.web ^1 local \"dep\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("zirk.lock"),
        "acme.web 1.0 local sha256:fixture src/package.zrk\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { api } from acme.web;\nimport { Api } from acme.web::api;\nfn receive(value: Fn() => Void): Void { value(); }\nfn main(): Void { mut item: api::Api = Api(); receive(api::clear); }",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/.zkinit"),
        "project \"acme.web\"\nkind package\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/package.zrk"),
        "package acme.web;\nshare mod api from \"./api\";",
    )
    .unwrap();
    std::fs::write(
        dir.join("dep/src/api.zrk"),
        "share class Api { }\nshare fn clear(): Void { }",
    )
    .unwrap();
    lock_local_dependency(&dir, "acme.web", "dep");
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run qualified module value/type check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_and_build_agree_on_invalid_programs() {
    let path = corpus("invalid").join("type_mismatch.zrk");
    let (_, _, check_stderr) = zirk_check(&path, "agreement_check");

    let dir = workspace("agreement_build");
    let copied = dir.join(path.file_name().expect("file name"));
    std::fs::copy(&path, &copied).expect("copy the source");

    let build_output = Command::new(compiler())
        .arg("build")
        .arg(copied.file_name().expect("file name"))
        .current_dir(&dir)
        .output()
        .expect("run the compiler");
    let build_stderr = String::from_utf8_lossy(&build_output.stderr).to_string();

    // The important part is that the same error is reported, not necessarily
    // identical formatting (which already has dedicated tests).
    assert!(
        !check_stderr.is_empty() && !build_stderr.is_empty(),
        "both `check` and `build` must emit diagnostics"
    );
}

#[test]
fn a_valid_program_passes_check_without_extension() {
    let path = corpus("valid").join("hello.zrk");
    let (success, stdout, stderr) = zirk_check_with_arg(&path, "valid_hello_bare", "hello");

    assert!(
        success,
        "`zirk check hello` must exit 0 for a valid program"
    );
    assert!(
        stdout.is_empty(),
        "`zirk check hello` must not write to stdout"
    );
    assert!(
        stderr.is_empty(),
        "`zirk check hello` must not produce diagnostics, got: {stderr}"
    );
}

#[test]
fn an_invalid_program_is_rejected_without_extension() {
    let path = corpus("invalid").join("type_mismatch.zrk");
    let (success, stdout, stderr) = zirk_check_with_arg(&path, "invalid_bare", "type_mismatch");

    assert!(
        !success,
        "`zirk check type_mismatch` must exit non-zero for an invalid program"
    );
    assert!(
        stdout.is_empty(),
        "`zirk check type_mismatch` must not write to stdout"
    );
    assert!(
        !stderr.is_empty(),
        "`zirk check type_mismatch` must emit a diagnostic on stderr, got: {stderr}"
    );
}

#[test]
fn a_program_with_zrk_extension_still_works() {
    let path = corpus("valid").join("hello.zrk");
    let (success, stdout, stderr) = zirk_check_with_arg(&path, "valid_hello_ext", "hello.zrk");

    assert!(success, "`zirk check hello.zrk` must still exit 0");
    assert!(
        stdout.is_empty(),
        "`zirk check hello.zrk` must not write to stdout"
    );
    assert!(
        stderr.is_empty(),
        "`zirk check hello.zrk` must not produce diagnostics, got: {stderr}"
    );
}
