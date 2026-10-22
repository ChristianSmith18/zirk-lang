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
fn network_resources_cannot_cross_managed_thread_boundaries() {
    let path = corpus("invalid").join("network_resources_cannot_cross_thread_boundary.zrk");
    let (success, stdout, stderr) = zirk_check(&path, "network_resource_thread_boundary");

    assert!(
        !success,
        "owner-bound TCP/TLS resources must not cross Thread.run"
    );
    assert!(stdout.is_empty(), "zirk check must not write to stdout");
    assert_eq!(
        stderr.matches("error[E0482]").count(),
        2,
        "both network resource types must fail the transfer proof:\n{stderr}"
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
fn exact_environment_requirements_are_accepted_in_a_project_manifest() {
    let dir = workspace("environment_manifest_check");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join(".zkinit"),
        "project \"demo.environment\"\nkind application\npermissions {\nenvironment {\nread: { names: [\"APP_MODE\"]; during: runtime; }\nsecrets: { names: [\"DB_PASSWORD\"]; during: runtime; }\nlist_names: { during: runtime; }\n}\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.zrk"),
        "import { Env } from std.environment;\nfn main(): Void { mut mode = Env.get(\"APP_MODE\"); }\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "."])
        .current_dir(&dir)
        .output()
        .expect("run environment project check");
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
fn stdout_requires_an_explicit_standard_module_import() {
    let dir = workspace("stdout_requires_import");
    std::fs::write(
        dir.join("main.zrk"),
        "fn main(): Void { stdout.println(\"not global\"); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run missing stdout import check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("`stdout` is not available"));
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
fn environment_package_import_and_fail_closed_api_resolve() {
    let dir = workspace("standard_environment_package");
    std::fs::write(
        dir.join("main.zrk"),
        "import { Env, Environment } from std.environment;\n\
         fn main(): Void {\n\
             mut alias: Env? = null;\n\
             mut outcome = Env.get(\"ZIRK_TEST_VALUE\");\n\
             mut parsed = Env.get<Int32>(\"ZIRK_TEST_PORT\");\n\
             mut also: String? = Environment.get(\"ZIRK_TEST_VALUE\");\n\
             mut names = Env.list_names();\n\
             _ = outcome;\n\
             _ = parsed;\n\
             _ = also;\n\
             _ = names;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run bundled environment package check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn contextual_result_projection_and_preservation_resolve() {
    let dir = workspace("contextual_result_projection");
    std::fs::write(
        dir.join("main.zrk"),
        "fn keep(value: Result<Int32, String>): Result<Int32, String> { return value; }\n\
         fn project_return(value: Result<Int32, String>): Int32? { return value; }\n\
         fn accept_nullable(value: Int32?): Int32? { return value; }\n\
         fn project_nested(): Result<Result<Int32, String>, String> { mut inner: Result<Int32, String> = Ok(5); return Ok(inner); }\n\
         class Holder { projected: Int32?; preserved: Result<Int32, String>; construct(value: Result<Int32, String>) { this.projected = value; this.preserved = value; } }\n\
         fn identity<T>(value: T): T { return value; }\n\
         fn main(): Void {\n\
             mut result: Result<Int32, String> = Ok(5);\n\
             mut projected: Int32? = result;\n\
             mut fallback: Int32 = result ?? 0;\n\
             mut argument_projection: Int32? = accept_nullable(result);\n\
             mut return_projection: Int32? = project_return(result);\n\
             mut nested_projection: Result<Int32, String>? = project_nested();\n\
             mut holder = Holder(result);\n\
             mut preserved: Result<Int32, String> = identity<Result<Int32, String>>(result);\n\
             mut inferred: Int32? = identity(result);\n\
             mut matched = match result { Ok(value) => value; Error(_) => -1; };\n\
             _ = projected; _ = fallback; _ = argument_projection; _ = return_projection;\n\
             _ = nested_projection; _ = holder; _ = preserved; _ = inferred; _ = matched;\n\
             _ = keep(result);\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check contextual Result behavior");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn result_projection_rejects_payloads_without_nullable_runtime_layouts() {
    let dir = workspace("unsupported_result_projection");
    std::fs::write(
        dir.join("main.zrk"),
        "fn main(): Void { mut value: Decimal = Decimal.parse(\"1.5\"); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check unsupported nullable Result payload");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot be projected to a nullable value")
    );
}

#[test]
fn removed_result_extraction_methods_report_migration_guidance() {
    for method in [
        "is_ok()",
        "is_error()",
        "ok_or_null()",
        "error_or_null()",
        "get_or(0)",
        "get_or_else(0)",
        "unwrap()",
        "unwrap_error()",
    ] {
        let dir = workspace(&format!("removed_result_helper_{method}"));
        std::fs::write(
            dir.join("main.zrk"),
            format!(
                "fn main(): Void {{ mut result: Result<Int32, String> = Ok(5); _ = result.{method}; }}"
            ),
        )
        .unwrap();
        let output = Command::new(compiler())
            .args(["check", "main.zrk"])
            .current_dir(&dir)
            .output()
            .expect("check removed Result method");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{method} unexpectedly compiled");
        let method_name = method.split('(').next().unwrap();
        assert!(
            stderr.contains(&format!("`Result.{method_name}` is no longer available")),
            "{method} did not get its migration diagnostic: {stderr}"
        );
    }
}

#[test]
fn http_package_publishes_method_status_and_client_types() {
    let dir = workspace("standard_http_values");
    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTP, HTTPClient, HTTPError, HTTPEvent, HTTPEventStream, HTTPHeaders, HTTPMethod, HTTPMiddleware, HTTPMultipartPart, HTTPNext, HTTPProxy, HTTPRequest, HTTPResponse, HTTPRouteHandler, HTTPServer, HTTPStatus, HTTPStreamResponse, URL } from std.http;\n\
         import { TCPListener } from std.net::tcp;\n\
         import { HostPortAddress, SocketAddress } from std.net::address;\n\
         import { SecretString } from std.environment;\n\
         fn verify_url(value: String): Void { mut parsed = URL.parse(value); _ = parsed; }\n\
         fn verify_method_name(method: HTTPMethod): String { return HTTP.method_name(method); }\n\
         fn verify_http(url: URL): Void { mut response = HTTP.get(url); _ = response; mut cached = HTTP.get_cached(url); _ = cached; mut followed = HTTP.get_following(url); _ = followed; mut head = HTTP.head(url); _ = head; mut empty_body: Array<Byte> = []; mut request_cached = HTTP.request_cached(HTTPMethod.Query, url, empty_body, HTTPHeaders.create()); _ = request_cached; }\n\
         fn verify_server(listener: TCPListener, handler: Fn(HTTPRequest) => HTTPResponse): Result<UInt64, HTTPError> { return HTTPServer.serve_many(listener, 2, handler); }\n\
         fn verify_retries(client: HTTPClient, url: URL): Void { mut empty_body: Array<Byte> = []; _ = client.request_with_retries(HTTPMethod.Query, url, empty_body, HTTPHeaders.create(), 2); }\n\
         fn verify_proxy_tls(proxy: HTTPProxy, roots: Array<Array<Byte>>): Void { mut address = proxy.address(); _ = address; _ = HTTPClient.create_with_proxy_and_trust_roots(proxy, roots); }\n\
         fn verify_address_origins(ip: SocketAddress, host: HostPortAddress): String { _ = host.as_tcp_origin(); return ip.as_tcp_origin(); }\n\
         fn verify_authenticated_https_proxy(url: URL, secret: SecretString): Void { _ = HTTPProxy.create_authenticated(url, secret); }\n\
         fn response_text(response: HTTPResponse): Result<String, HTTPError> { return response.text(); }\n\
         fn verify_stream_alias(stream: HTTPStreamResponse): HTTPResponse { return stream; }\n\
         fn verify_multipart_part(part: HTTPMultipartPart): String { return part.name(); }\n\
         fn verify_header_helpers(headers: HTTPHeaders): Array<Byte> { mut copy = headers.redirect_copy(true, true); _ = copy; return headers.encode_for_transport(); }\n\
         fn verify_response_helpers(response: HTTPResponse, patterns: List<String>, methods: List<String>, path: String): Void { mut fields = response.read_sse_event_fields(); mut beats = response.sse_heartbeat_count(); mut rejected = HTTPResponse.method_not_allowed(patterns, methods, path); _ = fields; _ = beats; _ = rejected; }\n\
         fn verify_callback_helpers(route: HTTPRouteHandler, middleware: HTTPMiddleware, request: HTTPRequest, values: List<String>, next: HTTPNext): Void { mut routed = route.invoke(request, values); mut continued = middleware.invoke(request, next); _ = routed; _ = continued; }\n\
         fn verify_sse(url: URL): Void { match HTTPEventStream.connect(url) { Ok(stream) => { mut event: Result<HTTPEvent?, HTTPError> = stream.next_event(); _ = event; _ = stream.close(); } Error(_) => { } } }\n\
         fn main(): Void {\n\
             mut method: HTTPMethod = HTTPMethod.Query;\n\
             mut status: HTTPStatus = HTTPStatus.Ok;\n\
             mut code: Int32 = status.value;\n\
             mut extension: HTTPStatus = HTTPStatus.Unknown(499);\n\
             mut extension_code: Int32 = match extension {\n\
                 HTTPStatus.Unknown(extension_value) => extension_value;\n\
                 _ => 0;\n\
             };\n\
             mut failed: Result<HTTPResponse, HTTPError> = Error(HTTPError.InvalidUrl);\n\
             _ = failed;\n\
             _ = method;\n\
             _ = code;\n\
             _ = extension_code;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run bundled HTTP value type check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn http_response_internal_result_helpers_are_private() {
    let dir = workspace("standard_http_private_result_helpers");
    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPResponse } from std.http;\n\
         fn main(): Void {\n\
             mut response = HTTPResponse.indirect_url_ok(null);\n\
             _ = response;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check private HTTPResponse result constructor");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("indirect_url_ok"));

    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPClient } from std.http;\n\
         fn main(): Void {\n\
             mut redirect = HTTPClient.is_redirect_status(302);\n\
             _ = redirect;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check private HTTP redirect predicate");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("is_redirect_status"));

    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPClient, URL } from std.http;\n\
         fn compare_origins(left: URL, right: URL): Boolean {\n\
             return HTTPClient.same_origin(left, right);\n\
         }\n\
         fn main(): Void { }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check private HTTP origin predicate");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("same_origin"));
}

#[test]
fn websocket_client_types_and_operations_compile_through_bundled_std_http() {
    let dir = workspace("standard_http_websocket_client");
    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPError, HTTPWebSocket, HTTPWebSocketMessage, URL } from std.http;\n\
         fn exercise_client(url: URL): Void {\n\
             mut protocols: List<String> = List();\n\
             mut binary: Array<Byte> = [0 as Byte, 255 as Byte];\n\
             protocols.add(\"zirk.echo.v1\");\n\
             match HTTPWebSocket.connect_with_options(url, \"https://app.example\", protocols) {\n\
                 Ok(socket) => {\n\
                     _ = socket.send(HTTPWebSocketMessage.Text(\"hello\"));\n\
                     mut received: Result<HTTPWebSocketMessage, HTTPError> = socket.receive();\n\
                     _ = received;\n\
                     _ = socket.send(HTTPWebSocketMessage.Binary(binary));\n\
                     _ = socket.close();\n\
                 }\n\
                 Error(_) => { }\n\
             }\n\
         }\n\
         fn main(): Void { }\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled WebSocket client API");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn websocket_server_tcp_and_tls_callbacks_compile_through_bundled_std_http() {
    let dir = workspace("standard_http_websocket_server");
    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPError, HTTPServer, HTTPWebSocket } from std.http;\n\
         import { TCPStream } from std.net::tcp;\n\
         import { TLSStream } from std.net::tls;\n\
         fn handle(socket: HTTPWebSocket): Void {\n\
             _ = socket.receive();\n\
         }\n\
         fn serve_tcp(stream: TCPStream): Result<Void, HTTPError> {\n\
             mut origins: List<String> = List();\n\
             mut protocols: List<String> = List();\n\
             protocols.add(\"zirk.echo.v1\");\n\
             return HTTPServer.serve_websocket_once(stream, \"/socket\", origins, protocols, 8, handle);\n\
         }\n\
         fn serve_tls(stream: TLSStream): Result<Void, HTTPError> {\n\
             mut origins: List<String> = List();\n\
             mut protocols: List<String> = List();\n\
             return HTTPServer.serve_tls_websocket_once(stream, \"/socket\", origins, protocols, 8, handle);\n\
         }\n\
         fn main(): Void { }\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled WebSocket server API");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn websocket_server_http3_api_compiles_through_bundled_std_http() {
    let dir = workspace("standard_http3_websocket_server");
    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPError, HTTPServer, HTTPWebSocket } from std.http;\n\
         fn handle(socket: HTTPWebSocket): Void { _ = socket.receive(); }\n\
         fn serve(endpoint: String, certificates: Array<Array<Byte>>, key: Array<Byte>): Result<Void, HTTPError> {\n\
             mut origins: List<String> = List();\n\
             mut protocols: List<String> = List();\n\
             protocols.add(\"zirk.echo.v1\");\n\
             return HTTPServer.serve_http3_websocket_once(endpoint, certificates, key, \"/socket\", origins, protocols, 8, handle, false, 30000);\n\
         }\n\
         fn main(): Void { }\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled HTTP/3 WebSocket server API");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(compiler())
        .args(["build", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("lower and compile bundled HTTP/3 WebSocket server API");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_user_defined_http_type_does_not_acquire_bundled_native_bindings() {
    let dir = workspace("user_http_name_collision");
    std::fs::write(
        dir.join("main.zrk"),
        "import { stdout } from std.io;\n\
         class HTTP {\n\
             static get_native(value: String): String { return \"user:\" + value; }\n\
         }\n\
         fn main(): Void { stdout.println(HTTP.get_native(\"ordinary\")); }\n",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["run", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run user-defined HTTP name collision fixture");
    assert!(
        output.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    assert_eq!(stdout, "user:ordinary\n");
}

#[test]
fn native_standard_library_io_effects_propagate_through_wrappers() {
    let http = workspace("http_effect_cannot_hide_in_mutex_callback");
    std::fs::write(
        http.join("main.zrk"),
        "import { HTTP, HTTPStreamResponse, URL } from std.http;\n\
         fn request(url: URL, mutex: Mutex<Int32>): Void {\n\
             mutex.with((value: Int32): Void => {\n\
                 mut result = HTTP.get(url);\n\
                 _ = result;\n\
             });\n\
         }\n\
         fn consume(stream: HTTPStreamResponse, mutex: Mutex<Int32>): Void {\n\
             mutex.with((value: Int32): Void => {\n\
                 mut chunk = stream.read_chunk(1);\n\
                 _ = chunk;\n\
             });\n\
         }\n\
         fn main(): Void { }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&http)
        .output()
        .expect("check HTTP suspension effect");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "HTTP must not be hidden in a mutex callback"
    );
    assert!(
        diagnostics.contains("Mutex.with` callback may suspend"),
        "{diagnostics}"
    );

    let environment = workspace("environment_effect_cannot_hide_in_thread_callback");
    std::fs::write(
        environment.join("main.zrk"),
        "import { Env } from std.environment;\n\
         fn read_environment(): Void {\n\
             mut result = Env.get(\"APP_MODE\");\n\
             _ = result;\n\
         }\n\
         fn main(): Void { Thread.run(read_environment); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&environment)
        .output()
        .expect("check environment blocking effect");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "environment lookup must not be hidden in a thread callback"
    );
    assert!(
        diagnostics.contains("Thread.run` callback has effects unsupported"),
        "{diagnostics}"
    );

    let upload = workspace("http_upload_file_read_is_blocking");
    std::fs::write(
        upload.join("main.zrk"),
        "import { HTTPMultipartData, HTTPMultipartPart, HTTPRequest, HTTPStreamChunk } from std.http;\n\
         fn read_upload(part: HTTPMultipartPart): Void {\n\
             mut result: HTTPStreamChunk = part.read_temporary_upload(0, 1);\n\
             _ = result;\n\
         }\n\
         fn parse_upload(request: HTTPRequest): Void { mut result = HTTPMultipartData.parse(request); _ = result; }\n\
         fn main(part: HTTPMultipartPart, request: HTTPRequest): Void {\n\
             Thread.run((): Void => read_upload(part));\n\
             Thread.run((): Void => parse_upload(request));\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&upload)
        .output()
        .expect("check blocking multipart upload file read");
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "temporary upload reads must not be hidden in a thread callback"
    );
    assert!(
        diagnostics.contains("Thread.run` callback has effects unsupported"),
        "{diagnostics}"
    );
}

#[test]
fn net_package_publishes_task_aware_tcp_resource_types() {
    let dir = workspace("standard_net_tcp_package");
    std::fs::write(
        dir.join("main.zrk"),
        "import { TCPError, TCPReadResult, TCPResourceError, TCPStream } from std.net::tcp;\n\
         fn close_stream(stream: TCPStream): Result<Void, TCPResourceError> { return stream.close(); }\n\
         fn main(): Void {\n\
             mut result: Result<TCPStream, TCPError> = Error(TCPError.PermissionDenied);\n\
             mut read: TCPReadResult<Array<Byte>> = TCPReadResult.End;\n\
             _ = result;\n\
             _ = read;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run bundled networking package check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn net_package_publishes_permission_scoped_dns_resolution() {
    let dir = workspace("standard_net_dns_package");
    std::fs::write(
        dir.join("main.zrk"),
        "import { DNS, DNSError } from std.net::dns;\n\
         fn resolve(origin: String): Result<List<String>, DNSError> { return DNS.resolve(origin); }\n\
         fn main(): Void {\n\
             mut result: Result<List<String>, DNSError> = Error(DNSError.PermissionDenied);\n\
             _ = result;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled DNS package");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn net_package_publishes_validated_ip_address_values() {
    let dir = workspace("standard_net_address_package");
    std::fs::write(
        dir.join("main.zrk"),
        "import { AddressError, IPAddress } from std.net::address;\n\
         fn parse_address(value: String): Result<IPAddress, AddressError> { return IPAddress.parse(value); }\n\
         fn main(): Void {\n\
             mut result: Result<IPAddress, AddressError> = Error(AddressError.InvalidAddress);\n\
             _ = result;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled address package");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn net_package_publishes_typed_socket_addresses_for_tcp() {
    let dir = workspace("standard_net_socket_address_package");
    std::fs::write(
        dir.join("main.zrk"),
        "import { AddressError, SocketAddress } from std.net::address;\n\
         import { TCPError, TCPListener, TCPStream } from std.net::tcp;\n\
         fn parse_address(ip: String, port: UInt16): Result<SocketAddress, AddressError> { return SocketAddress.parse(ip, port); }\n\
         fn connect(address: SocketAddress): Result<TCPStream, TCPError> { return TCPStream.connect_to(address); }\n\
         fn bind(address: SocketAddress): Result<TCPListener, TCPError> { return TCPListener.bind_to(address); }\n\
         fn main(): Void {\n\
             mut result: Result<SocketAddress, AddressError> = Error(AddressError.InvalidAddress);\n\
             _ = result;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled typed socket address APIs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn net_package_publishes_authenticated_tls_stream_types() {
    let dir = workspace("standard_net_tls_package");
    std::fs::write(
        dir.join("main.zrk"),
        "import { TLSReadResult, TLSError, TLSResourceError, TLSStream, TLSListener } from std.net::tls;\n\
         fn connect(endpoint: String): Result<TLSStream, TLSError> { return TLSStream.connect(endpoint); }\n\
         fn connect_with_roots(endpoint: String, roots: Array<Array<Byte>>, allow_tls12: Boolean): Result<TLSStream, TLSError> { return TLSStream.connect_with_trust_roots(endpoint, roots, allow_tls12); }\n\
         fn listen(endpoint: String, chain: Array<Array<Byte>>, key: Array<Byte>, allow_tls12: Boolean): Result<TLSListener, TLSError> { return TLSListener.bind(endpoint, chain, key, allow_tls12); }\n\
         fn accept(listener: TLSListener): Result<TLSStream, TLSError> { return listener.accept(); }\n\
         fn close_listener(listener: TLSListener): Result<Void, TLSResourceError> { return listener.close(); }\n\
         fn listener_is_closed(listener: TLSListener): Boolean { return listener.is_closed(); }\n\
         fn peer_address(stream: TLSStream): Result<String, TLSError> { return stream.peer_address(); }\n\
         fn read(stream: TLSStream): TLSReadResult<Array<Byte>> { return stream.read(1024); }\n\
         fn write(stream: TLSStream, bytes: Array<Byte>): Result<Void, TLSError> { return stream.write_all(bytes); }\n\
         fn close(stream: TLSStream): Result<Void, TLSResourceError> { return stream.close(); }\
         fn main(): Void {\n\
             mut result: Result<TLSStream, TLSError> = Error(TLSError.PermissionDenied);\n\
             _ = result;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled TLS package");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn net_package_publishes_permission_checked_listener_and_accepted_stream_types() {
    let dir = workspace("standard_net_tcp_listener_package");
    std::fs::write(
        dir.join("main.zrk"),
        concat!(
            "import { TCPError, TCPListener, TCPResourceError, TCPStream } from std.net::tcp;\n",
            "fn close_listener(listener: TCPListener): Result<Void, TCPResourceError> { return listener.close(); }\n",
            "fn bind(endpoint: String): Result<TCPListener, TCPError> { return TCPListener.bind(endpoint); }\n",
            "fn accept(listener: TCPListener): Result<TCPStream, TCPError> { return listener.accept(); }\n",
            "fn main(): Void {\n",
            "    mut result: Result<TCPListener, TCPError> = Error(TCPError.PermissionDenied);\n",
            "    _ = result;\n",
            "}",
        ),
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("run bundled TCP listener package check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bundled_http_static_helpers_require_a_shared_type() {
    let dir = workspace("standard_http_internal_static_helpers_are_private");
    std::fs::write(
        dir.join("main.zrk"),
        "import { HTTPRouteMatcher } from std.http;\n\
         fn main(): Void { _ = HTTPRouteMatcher.validate_pattern(\"/\"); }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check bundled HTTP private helper boundary");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "private helper was callable");
    assert!(stderr.contains("HTTPRouteMatcher"), "{stderr}");
    assert!(stderr.contains("without `share`"), "{stderr}");
}

#[test]
fn environment_secret_raw_binding_and_diagnostic_sinks_are_restricted() {
    let dir = workspace("standard_environment_secret_private");
    std::fs::write(
        dir.join("main.zrk"),
        "import { Environment } from std.environment;\n\
         fn main(): Void {\n\
             mut raw = Environment.get_secret_value(\"TOKEN\");\n\
             _ = raw;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check private environment secret boundary");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("get_secret_value"));

    std::fs::write(
        dir.join("main.zrk"),
        "import { SecretString } from std.environment;\n\
         fn main(): Void {\n\
             mut secret = SecretString(\"do-not-read\");\n\
             mut raw = secret.value;\n\
             _ = raw;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check private SecretString payload field");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("value"));

    std::fs::write(
        dir.join("main.zrk"),
        "import { Environment } from std.environment;\n\
         fn main(): Void {\n\
             mut result = Environment.string_ok(\"value\");\n\
             _ = result;\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check private Environment result constructor");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("string_ok"));

    std::fs::write(
        dir.join("main.zrk"),
        "import { SecretString } from std.environment;\n\
         fn main(): Void {\n\
             mut secret = SecretString(\"do-not-read\");\n\
             fatalError(secret);\n\
         }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check that fatal diagnostics cannot receive a secret as an ordinary string");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fatalError"));
}

#[test]
fn environment_secret_flows_only_through_explicit_http_sink_api() {
    let dir = workspace("environment_secret_http_sink");
    std::fs::write(
        dir.join("main.zrk"),
        "import { SecretString } from std.environment;\n\
         import { HTTP, HTTPError, HTTPHeaders, HTTPMethod, HTTPResponse, URL } from std.http;\n\
         fn send_secret(url: URL, headers: HTTPHeaders, secret: SecretString): Result<HTTPResponse, HTTPError> {\n\
             mut body: Array<Byte> = [];\n\
             return HTTP.request_with_secret_header(HTTPMethod.Get, url, body, headers, \"Authorization\", secret);\n\
         }\n\
         fn main(): Void { }",
    )
    .unwrap();
    let output = Command::new(compiler())
        .args(["check", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("check explicit secret sink API");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(compiler())
        .args(["build", "main.zrk"])
        .current_dir(&dir)
        .output()
        .expect("compile the explicit secret sink through native code generation");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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
        "import { terminal } from acme.cli;\nfn main(): Void { clear(); terminal.clear(); }",
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
        "import { terminal, screen } from acme.cli;\nfn main(): Void { terminal.clear(); screen.clear(); clear(); }",
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
fn module_values_types_and_direct_member_imports_resolve() {
    let dir = workspace("module_values_types");
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
        "import { api } from acme.web;\nimport { Api } from acme.web::api;\nfn receive(value: Fn() => Void): Void { value(); }\nfn main(): Void { mut item: api.Api = Api(); receive(api.clear); }",
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
