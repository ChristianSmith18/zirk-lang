//! # zirk-browser
//!
//! **Responsibility:** host the in-memory Zirk frontend and the versioned,
//! worker-compatible browser contract.
//!
//! **Boundary:** this crate coordinates lexer, parser, semantic analysis, IR
//! lowering, preflight, and `zirk-interpreter`. It does not define language
//! semantics, access ambient browser APIs, invoke LLVM, link native code, or
//! depend on `zirk-runtime`.

#![forbid(unsafe_code)]

use wasm_bindgen::prelude::wasm_bindgen;
use zirk_ast::{Ident, ImportSource, ModuleDecl};
use zirk_diagnostics::{DiagnosticSink, SourceFile, SourceIdentity, SourceMap};

pub mod contract;

/// Shared fixture support used by host and browser-package conformance
/// runners. Fixture parsing is deliberately data-only: source stays in the
/// caller-provided JSON buffer and virtual names are never opened as paths.
pub mod fixtures {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    pub struct Fixture {
        pub name: String,
        pub source: String,
        pub virtual_name: String,
        pub expected_output: Vec<String>,
        pub completion: String,
        pub exclusions: Vec<String>,
    }
}

/// The first stable browser-contract revision. Request and response shapes
/// will be added alongside `check` and `run`; keeping the version here makes
/// provenance explicit from the crate's first buildable WASM artifact.
pub const API_VERSION: u32 = 1;
const COMPILER_REVISION: &str = "uncommitted";

/// Worker-safe contract object exposed by `wasm-bindgen`.
///
/// Constructing the object does not grant execution immediately. Consumers
/// must call [`BrowserApi::initialize`] after their asynchronous WASM module
/// initialization has completed; this makes accidental calls from a worker's
/// module-loading phase observable and deterministic. The implementation
/// accesses no `window`, `document`, DOM, clock, storage, network, or host
/// process APIs.
#[wasm_bindgen]
pub struct BrowserApi {
    initialized: bool,
}

impl Default for BrowserApi {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl BrowserApi {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Marks the API ready after the caller's WASM initialization promise has
    /// resolved. It is deliberately explicit rather than inferred from DOM
    /// state so module workers remain supported.
    pub fn initialize(&mut self) {
        self.initialized = true;
    }

    /// Runs the versioned `check` operation over a JSON request and returns a
    /// JSON response. JSON keeps the ABI stable while `wasm-bindgen` emits the
    /// strict TypeScript declaration for this method.
    #[wasm_bindgen(js_name = check)]
    pub fn check_json(&self, request_json: &str) -> String {
        self.dispatch(request_json, false)
    }

    /// Runs the versioned `run` operation over a JSON request and returns a
    /// JSON response. No ambient browser authority is passed to the
    /// interpreter.
    #[wasm_bindgen(js_name = run)]
    pub fn run_json(&self, request_json: &str) -> String {
        self.dispatch(request_json, true)
    }

    /// Runs bounded HTTP requests through one explicitly injected browser
    /// fetch function. The origin list, body ceilings, and abort signal live
    /// for this invocation only; `run` remains hermetic and synchronous.
    #[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
    #[wasm_bindgen(js_name = runWithFetch)]
    pub async fn run_with_fetch_json(
        &self,
        request_json: &str,
        fetch_function: js_sys::Function,
        allowed_origins_json: &str,
        max_request_bytes: u32,
        max_response_bytes: u32,
        abort_signal: wasm_bindgen::JsValue,
    ) -> String {
        let response = if !self.initialized {
            request_failure("ZB103", "browser API is not initialized")
        } else {
            match serde_json::from_str::<contract::Request>(request_json) {
                Err(_) => request_failure("ZB104", "malformed browser request"),
                Ok(request) => match serde_json::from_str::<Vec<String>>(allowed_origins_json) {
                    Err(_) => request_failure("ZB107", "malformed HTTP origin allowlist"),
                    Ok(origins) => {
                        run_with_fetch_async(
                            request,
                            fetch_function,
                            origins,
                            max_request_bytes as usize,
                            max_response_bytes as usize,
                            abort_signal,
                        )
                        .await
                    }
                },
            }
        };
        serde_json::to_string(&response).unwrap_or_else(|_| {
            request_failure("ZB199", "browser response serialization failed").operation
        })
    }

    /// Runs the additive versioned `format` operation without granting host
    /// I/O to the formatter.
    #[wasm_bindgen(js_name = format)]
    pub fn format_json(&self, request_json: &str) -> String {
        self.dispatch_format(request_json)
    }

    fn dispatch(&self, request_json: &str, execute: bool) -> String {
        let response = if !self.initialized {
            request_failure("ZB103", "browser API is not initialized")
        } else {
            match serde_json::from_str::<contract::Request>(request_json) {
                Ok(request) if execute => run(request),
                Ok(request) => check(request),
                Err(_) => request_failure("ZB104", "malformed browser request"),
            }
        };
        // All contract values are serializable by construction. Avoid leaking
        // an internal serializer detail if this invariant is ever broken.
        serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{\"apiVersion\":1,\"operation\":\"contract\",\"compilerVersion\":\"unknown\",\"compilerRevision\":\"unknown\",\"limits\":{\"sourceBytes\":0,\"diagnostics\":0,\"instructions\":0,\"callDepth\":0,\"heapBytes\":0,\"outputBytes\":0,\"outputEvents\":0,\"stackFrames\":0},\"diagnostics\":[],\"diagnosticsTruncated\":false,\"output\":[],\"completion\":null,\"failure\":{\"kind\":\"internal\",\"code\":\"ZB199\",\"message\":\"browser response serialization failed\",\"limit\":null}}"#.into()
        })
    }

    fn dispatch_format(&self, request_json: &str) -> String {
        let response = if !self.initialized {
            request_failure("ZB103", "browser API is not initialized")
        } else {
            match serde_json::from_str::<contract::Request>(request_json) {
                Ok(request) => format(request),
                Err(_) => request_failure("ZB104", "malformed browser request"),
            }
        };
        serde_json::to_string(&response).unwrap_or_else(|_| {
            request_failure("ZB199", "browser response serialization failed").operation
        })
    }
}

fn request_failure(code: &str, message: &str) -> contract::Response {
    contract::Response {
        api_version: API_VERSION,
        operation: "contract".into(),
        compiler_version: env!("CARGO_PKG_VERSION").into(),
        compiler_revision: COMPILER_REVISION.into(),
        limits: contract::Limits::DEFAULT,
        diagnostics: Vec::new(),
        diagnostics_truncated: false,
        output: Vec::new(),
        completion: None,
        failure: Some(contract::Failure {
            kind: contract::FailureKind::Request,
            code: code.into(),
            message: message.into(),
            limit: None,
        }),
        formatted_source: None,
        trace: None,
    }
}

/// Successful result of the browser's single-source frontend pass.
///
/// Browser execution deliberately accepts one caller-owned source buffer.
/// Local-file imports stay outside the initial browser compatibility baseline:
/// resolving them would require ambient filesystem or network authority.
pub struct FrontendResult {
    pub program: zirk_ast::Program,
    pub checked: zirk_sema::CheckedProgram,
    pub sources: SourceMap,
}

/// Runs the authoritative lexer, parser, and semantic checker over source
/// already held in memory. No path is opened, normalized, or resolved.
///
/// `virtual_name` is diagnostic metadata only; it never denotes a host path.
/// The returned sink preserves the frontend's native diagnostic order.
pub fn check_source(
    source: impl Into<String>,
    virtual_name: impl Into<String>,
) -> (Option<FrontendResult>, DiagnosticSink) {
    let mut sources = SourceMap::new();
    let file = sources.add(SourceFile::new(virtual_name, source));
    let mut sink = DiagnosticSink::new();
    let tokens = zirk_lexer::tokenize(sources.file(file), &mut sink);
    let mut program = zirk_parser::parse(sources.file(file), &tokens, &mut sink);
    if sink.has_errors() {
        return (None, sink);
    }
    let (standard_packages, standard_modules) =
        bundled_standard_sources(&mut sources, &mut sink, &program.imports);
    // The browser has no filesystem-backed loader, so mirror the CLI's merge:
    // modules carry their published-member metadata and their declarations are
    // also part of the checked executable program.
    for standard_package in standard_packages {
        program
            .imports
            .extend(standard_package.imports.iter().cloned());
        program.enums.extend(standard_package.enums);
        program.classes.extend(standard_package.classes);
        program.contracts.extend(standard_package.contracts);
        program.type_aliases.extend(standard_package.type_aliases);
    }
    for module in &standard_modules {
        program.imports.extend(module.body.imports.iter().cloned());
        program.enums.extend(module.body.enums.iter().cloned());
        program.classes.extend(module.body.classes.iter().cloned());
        program
            .contracts
            .extend(module.body.contracts.iter().cloned());
        program
            .functions
            .extend(module.body.functions.iter().cloned());
        program
            .type_aliases
            .extend(module.body.type_aliases.iter().cloned());
    }
    program.modules.extend(standard_modules);
    if sink.has_errors() {
        return (None, sink);
    }
    let checked = zirk_sema::check(&sources, &program, &mut sink);
    if sink.has_errors() {
        return (None, sink);
    }
    (
        Some(FrontendResult {
            program,
            checked,
            sources,
        }),
        sink,
    )
}

/// Parses compiler-bundled standard packages into the browser's in-memory
/// source map. This mirrors the native package graph without filesystem reads
/// and gives bundled sources the same identities used by trusted stdlib
/// binding checks in semantic analysis.
fn bundled_standard_sources(
    sources: &mut SourceMap,
    sink: &mut DiagnosticSink,
    imports: &[zirk_ast::ImportDecl],
) -> (Vec<zirk_ast::Program>, Vec<ModuleDecl>) {
    let mut required_packages = std::collections::HashSet::from(["io".to_string()]);
    for import in imports {
        if let ImportSource::Package { path, .. } = &import.source
            && let Some(package) = path.strip_prefix("std.")
        {
            let package = package.split("::").next().unwrap_or(package);
            if matches!(
                package,
                "environment" | "fs" | "http" | "json" | "net" | "path"
            ) {
                required_packages.insert(package.to_string());
            }
        }
    }
    // HTTP's source API is built on the ordinary network and environment
    // packages. Include that dependency closure when HTTP is imported.
    if required_packages.contains("http") {
        required_packages.insert("environment".into());
        required_packages.insert("net".into());
        required_packages.insert("json".into());
    }
    let packages = [
        (
            "io",
            include_str!("../../../stdlib/std/io/package.zrk"),
            vec![
                ("stdin", include_str!("../../../stdlib/std/io/stdin.zrk")),
                ("stdout", include_str!("../../../stdlib/std/io/stdout.zrk")),
                ("stderr", include_str!("../../../stdlib/std/io/stderr.zrk")),
            ],
        ),
        (
            "environment",
            include_str!("../../../stdlib/std/environment/package.zrk"),
            vec![(
                "environment",
                include_str!("../../../stdlib/std/environment/environment.zrk"),
            )],
        ),
        (
            "fs",
            include_str!("../../../stdlib/std/fs/package.zrk"),
            vec![("fs", include_str!("../../../stdlib/std/fs/fs.zrk"))],
        ),
        (
            "http",
            include_str!("../../../stdlib/std/http/package.zrk"),
            vec![("http", include_str!("../../../stdlib/std/http/http.zrk"))],
        ),
        (
            "json",
            include_str!("../../../stdlib/std/json/package.zrk"),
            vec![("json", include_str!("../../../stdlib/std/json/json.zrk"))],
        ),
        (
            "net",
            include_str!("../../../stdlib/std/net/package.zrk"),
            vec![
                ("tcp", include_str!("../../../stdlib/std/net/tcp.zrk")),
                ("dns", include_str!("../../../stdlib/std/net/dns.zrk")),
                (
                    "address",
                    include_str!("../../../stdlib/std/net/address.zrk"),
                ),
                ("tls", include_str!("../../../stdlib/std/net/tls.zrk")),
                ("udp", include_str!("../../../stdlib/std/net/udp.zrk")),
                ("local", include_str!("../../../stdlib/std/net/local.zrk")),
                (
                    "interfaces",
                    include_str!("../../../stdlib/std/net/interfaces.zrk"),
                ),
            ],
        ),
        (
            "path",
            include_str!("../../../stdlib/std/path/package.zrk"),
            vec![("path", include_str!("../../../stdlib/std/path/path.zrk"))],
        ),
    ];

    let mut package_programs = Vec::with_capacity(packages.len());
    let mut modules = Vec::new();
    for (package, package_source, package_modules) in packages {
        if !required_packages.contains(package) {
            continue;
        }
        let package_path = format!("/stdlib/std/{package}/package.zrk");
        let package_file = add_bundled_source(
            sources,
            package_path,
            format!("std.{package}"),
            "package",
            &format!("stdlib/std/{package}"),
            package_source,
        );
        let tokens = zirk_lexer::tokenize(sources.file(package_file), sink);
        let package_program = zirk_parser::parse(sources.file(package_file), &tokens, sink);
        package_programs.push(package_program);

        for (name, source) in package_modules {
            let module_path = format!("/stdlib/std/{package}/{name}.zrk");
            let module_file = add_bundled_source(
                sources,
                module_path,
                format!("std.{package}"),
                name,
                &format!("stdlib/std/{package}"),
                source,
            );
            let tokens = zirk_lexer::tokenize(sources.file(module_file), sink);
            let body = zirk_parser::parse(sources.file(module_file), &tokens, sink);
            modules.push(ModuleDecl {
                name: Ident::new(name, body.span),
                shared: true,
                span: body.span,
                body: Box::new(body),
            });
        }
    }
    (package_programs, modules)
}

fn add_bundled_source(
    sources: &mut SourceMap,
    path: String,
    package: String,
    canonical_module_path: &str,
    package_root: &str,
    source: &str,
) -> zirk_diagnostics::FileId {
    let mut file = SourceFile::new(path.clone(), source);
    file.set_identity(SourceIdentity {
        package,
        canonical_module_path: canonical_module_path.into(),
        written_path: path,
        package_root: package_root.into(),
    });
    sources.add(file)
}

fn bundled_standard_source_ids(sources: &SourceMap) -> Vec<zirk_diagnostics::FileId> {
    sources
        .files()
        .filter_map(|(id, source)| {
            source
                .identity()
                .is_some_and(|identity| identity.package.starts_with("std."))
                .then_some(id)
        })
        .collect()
}

/// Executes the browser `check` operation. It deliberately stops at the
/// frontend boundary: invalid source cannot reach lowering or interpretation.
pub fn check(request: contract::Request) -> contract::Response {
    use contract::{Completion, Diagnostic, Failure, FailureKind, Limits, Response};

    let limits = request
        .limits
        .map(|requested| Limits::MAX.tightened_by(requested))
        .unwrap_or(Limits::DEFAULT);
    let mut response = Response {
        api_version: API_VERSION,
        operation: "check".into(),
        compiler_version: env!("CARGO_PKG_VERSION").into(),
        compiler_revision: COMPILER_REVISION.into(),
        limits,
        diagnostics: Vec::new(),
        diagnostics_truncated: false,
        output: Vec::new(),
        completion: None,
        failure: None,
        formatted_source: None,
        trace: None,
    };
    if request.api_version != API_VERSION {
        response.failure = Some(Failure {
            kind: FailureKind::Version,
            code: "ZB100".into(),
            message: format!("unsupported browser API version {}", request.api_version),
            limit: None,
        });
        return response;
    }
    if let Some(limit) = invalid_limit(limits) {
        response.failure = Some(Failure {
            kind: FailureKind::Limit,
            code: "ZB102".into(),
            message: format!("configured {limit} limit must be greater than zero"),
            limit: Some(limit.into()),
        });
        return response;
    }
    if request.source.len() > limits.source_bytes {
        response.failure = Some(Failure {
            kind: FailureKind::Limit,
            code: "ZB101".into(),
            message: "source exceeds the configured byte limit".into(),
            limit: Some("source".into()),
        });
        return response;
    }

    let source = request.source;
    let (_result, diagnostics) = check_source(source.clone(), request.virtual_name);
    response.diagnostics = diagnostics
        .diagnostics()
        .iter()
        .take(limits.diagnostics)
        .map(|diagnostic| Diagnostic {
            code: diagnostic.code.as_str().into(),
            severity: diagnostic.severity.as_str().into(),
            message: diagnostic.message.clone(),
            // The frontend currently exposes one sink, so this names the
            // stable pipeline boundary without fabricating a lexer/parser
            // distinction that the source diagnostics do not retain.
            stage: "frontend".into(),
            file: diagnostic
                .location
                .as_ref()
                .map(|location| location.file.clone()),
            start: diagnostic
                .location
                .as_ref()
                .map(|location| diagnostic_start(&source, location)),
            end: diagnostic.location.as_ref().map(|location| {
                let start = diagnostic_start(&source, location);
                start.saturating_add(
                    source[start as usize..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(0) as u32,
                )
            }),
        })
        .collect();
    response.diagnostics_truncated =
        diagnostics.len() > limits.diagnostics || diagnostics.suppressed() > 0;
    if !diagnostics.has_errors() {
        response.completion = Some(Completion::Completed);
    }
    response
}

/// Executes the browser `format` operation through the portable shared
/// formatter. Limits and invalid-request behavior match `check`.
pub fn format(request: contract::Request) -> contract::Response {
    let source = request.source.clone();
    let name = request.virtual_name.clone();
    let mut response = check(request);
    response.operation = "format".into();
    if response.failure.is_some() || response.completion.is_none() {
        response.completion = None;
        return response;
    }
    match zirk_format::format(&name, &source) {
        Ok(formatted) => response.formatted_source = Some(formatted),
        Err(_) => {
            response.failure = Some(contract::Failure {
                kind: contract::FailureKind::Internal,
                code: "ZB199".into(),
                message: "formatter rejected checked source".into(),
                limit: None,
            });
            response.completion = None;
        }
    }
    response
}

fn invalid_limit(limits: contract::Limits) -> Option<&'static str> {
    (limits.source_bytes == 0)
        .then_some("source")
        .or_else(|| (limits.diagnostics == 0).then_some("diagnostics"))
        .or_else(|| (limits.instructions == 0).then_some("instructions"))
        .or_else(|| (limits.call_depth == 0).then_some("call-depth"))
        .or_else(|| (limits.heap_bytes == 0).then_some("heap"))
        .or_else(|| (limits.output_bytes == 0).then_some("output-bytes"))
        .or_else(|| (limits.output_events == 0).then_some("output-events"))
        .or_else(|| (limits.stack_frames == 0).then_some("stack-frames"))
}

/// Executes the browser `run` operation through the same frontend result as
/// `check`, then lowering, verification, compatibility preflight, and the IR
/// interpreter. Compiler diagnostics always win over execution: no partial
/// program is ever lowered after a frontend error.
pub fn run(request: contract::Request) -> contract::Response {
    use contract::{Failure, FailureKind, OutputEvent};

    let source = request.source.clone();
    let virtual_name = request.virtual_name.clone();
    let trace_options = request.trace.clone();
    if let Some(kinds) = trace_options
        .as_ref()
        .and_then(|options| options.kinds.as_ref())
        && kinds
            .iter()
            .any(|kind| !contract::TRACE_KINDS.contains(&kind.as_str()))
    {
        let mut failure = request_failure("ZB105", "unknown execution trace event kind");
        failure.operation = "run".into();
        return failure;
    }
    let mut response = check(request);
    response.operation = "run".into();
    if response.failure.is_some() || response.completion.is_none() {
        response.completion = None;
        return response;
    }
    let (frontend, diagnostics) = check_source(source, virtual_name);
    if diagnostics.has_errors() {
        // This cannot normally differ from `check`; retain a sanitized
        // internal failure rather than attempting to execute an uncertain IR.
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB199".into(),
            message: "frontend result changed before execution".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    }
    let Some(frontend) = frontend else {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB199".into(),
            message: "frontend produced no executable result".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    };
    let module = zirk_ir::lower(&frontend.program, &frontend.checked);
    if zirk_ir::verify(&module).is_err() {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB198".into(),
            message: "compiler produced invalid IR".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    }
    if module.function("main").is_none() {
        response.failure = Some(Failure {
            kind: FailureKind::Capability,
            code: "ZB004".into(),
            message: "file-body entrypoints are unavailable in the browser runtime".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    }
    let limits = zirk_interpreter::InterpreterLimits {
        instructions: response.limits.instructions,
        call_depth: response.limits.call_depth,
        heap_bytes: response.limits.heap_bytes,
        output_bytes: response.limits.output_bytes,
        output_events: response.limits.output_events,
        stack_frames: response.limits.stack_frames,
    };
    let Ok(mut interpreter) = zirk_interpreter::Interpreter::new(&module, limits) else {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB198".into(),
            message: "interpreter rejected compiler IR".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    };
    interpreter.set_bundled_standard_sources(bundled_standard_source_ids(&frontend.sources));
    let trace_limits = trace_options.as_ref().map(|options| {
        let defaults = zirk_interpreter::TraceLimits::DEFAULT;
        zirk_interpreter::TraceLimits {
            max_events: options.events.unwrap_or(defaults.max_events),
            max_bytes: options.bytes.unwrap_or(defaults.max_bytes),
            max_value_bytes: options.value_bytes.unwrap_or(defaults.max_value_bytes),
        }
        .clamped()
    });
    if let Some(limits) = trace_limits {
        interpreter.enable_trace(limits);
    }
    let mut host = zirk_interpreter::CapturedHost::default();
    let result = interpreter.run_main_with_host(&mut host);
    if let (Some(limits), Some(options)) = (trace_limits, trace_options.as_ref()) {
        response.trace = Some(convert_trace(interpreter.take_trace(), limits, options));
    }
    response.output = host
        .events()
        .iter()
        .map(|event| OutputEvent {
            text: event.text.clone(),
            destination: match event.destination {
                zirk_interpreter::OutputDestination::Stdout => "stdout".into(),
                zirk_interpreter::OutputDestination::Stderr => "stderr".into(),
            },
            truncated: event.truncated,
        })
        .collect();
    response.completion = None;
    response.failure = execution_failure(result, &mut response.completion);
    response
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
struct FetchHost {
    events: Vec<zirk_interpreter::OutputEvent>,
    allowed_origins: std::collections::HashSet<String>,
    abort_signal: wasm_bindgen::JsValue,
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
impl zirk_interpreter::InterpreterHost for FetchHost {
    fn emit(
        &mut self,
        event: zirk_interpreter::OutputEvent,
    ) -> Result<(), zirk_interpreter::HostOutputError> {
        if let Some(last) = self.events.last_mut()
            && !event.truncated
            && !last.truncated
            && last.destination == event.destination
            && !last.text.ends_with('\n')
        {
            last.text.push_str(&event.text);
            return Ok(());
        }
        self.events.push(event);
        Ok(())
    }

    fn is_cancelled(&self) -> bool {
        js_sys::Reflect::get(&self.abort_signal, &"aborted".into())
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    fn begin_http_request(&mut self, request: &zirk_interpreter::HttpHostRequest) -> bool {
        if self.is_cancelled() {
            return false;
        }
        request_origin(&request.url).is_some_and(|origin| self.allowed_origins.contains(&origin))
    }
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
fn canonical_origin(value: &str) -> Option<String> {
    let url = zirk_runtime_core::http_values::Url::parse(value).ok()?;
    let normalized = url.as_str();
    let authority_and_suffix = normalized.split_once("://")?.1;
    let suffix_start = authority_and_suffix
        .find(['/', '?', '#'])
        .unwrap_or(authority_and_suffix.len());
    if !matches!(&authority_and_suffix[suffix_start..], "" | "/") {
        return None;
    }
    Some(url.origin())
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
fn request_origin(value: &str) -> Option<String> {
    zirk_runtime_core::http_values::Url::parse(value)
        .ok()
        .map(|url| url.origin())
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
fn parse_origin_allowlist(
    origins: Vec<String>,
) -> Result<std::collections::HashSet<String>, contract::Response> {
    if origins.is_empty() || origins.len() > 256 {
        return Err(request_failure(
            "ZB107",
            "HTTP origin allowlist must contain between 1 and 256 origins",
        ));
    }
    let mut normalized = std::collections::HashSet::with_capacity(origins.len());
    for origin in origins {
        let Some(origin) = canonical_origin(&origin) else {
            return Err(request_failure(
                "ZB107",
                "HTTP allowlist entries must be credential-free HTTP origins without a path",
            ));
        };
        normalized.insert(origin);
    }
    Ok(normalized)
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
async fn run_with_fetch_async(
    request: contract::Request,
    fetch_function: js_sys::Function,
    origins: Vec<String>,
    max_request_bytes: usize,
    max_response_bytes: usize,
    abort_signal: wasm_bindgen::JsValue,
) -> contract::Response {
    use contract::{Failure, FailureKind, OutputEvent};

    let mut response = check(request.clone());
    response.operation = "run".into();
    if response.failure.is_some() || response.completion.is_none() {
        response.completion = None;
        return response;
    }
    let default_body_limit = zirk_runtime_core::http_values::MAX_HTTP_BUFFERED_BODY_BYTES;
    if max_request_bytes == 0
        || max_request_bytes > default_body_limit
        || max_response_bytes == 0
        || max_response_bytes > default_body_limit
    {
        response.failure = Some(Failure {
            kind: FailureKind::Request,
            code: "ZB107".into(),
            message: "HTTP request and response byte limits must be between 1 and 16777216".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    }
    let allowed_origins = match parse_origin_allowlist(origins) {
        Ok(origins) => origins,
        Err(failure) => return failure,
    };

    let source = request.source.clone();
    let virtual_name = request.virtual_name.clone();
    let trace_options = request.trace.clone();
    let (frontend, diagnostics) = check_source(source, virtual_name);
    if diagnostics.has_errors() {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB199".into(),
            message: "frontend result changed before asynchronous execution".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    }
    let Some(frontend) = frontend else {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB199".into(),
            message: "frontend produced no executable result".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    };
    let standard_sources = bundled_standard_source_ids(&frontend.sources);
    let module = zirk_ir::lower(&frontend.program, &frontend.checked);
    if zirk_ir::verify(&module).is_err() {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB198".into(),
            message: "compiler produced invalid IR".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    }
    let limits = zirk_interpreter::InterpreterLimits {
        instructions: response.limits.instructions,
        call_depth: response.limits.call_depth,
        heap_bytes: response.limits.heap_bytes,
        output_bytes: response.limits.output_bytes,
        output_events: response.limits.output_events,
        stack_frames: response.limits.stack_frames,
    };
    let Ok(mut interpreter) = zirk_interpreter::Interpreter::new(&module, limits) else {
        response.failure = Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB198".into(),
            message: "interpreter rejected compiler IR".into(),
            limit: None,
        });
        response.completion = None;
        return response;
    };
    interpreter.set_bundled_standard_sources(standard_sources);
    let trace_limits = trace_options.as_ref().map(|options| {
        let defaults = zirk_interpreter::TraceLimits::DEFAULT;
        zirk_interpreter::TraceLimits {
            max_events: options.events.unwrap_or(defaults.max_events),
            max_bytes: options.bytes.unwrap_or(defaults.max_bytes),
            max_value_bytes: options.value_bytes.unwrap_or(defaults.max_value_bytes),
        }
        .clamped()
    });
    if let Some(limits) = trace_limits {
        interpreter.enable_trace(limits);
    }
    let mut host = FetchHost {
        events: Vec::new(),
        allowed_origins,
        abort_signal,
    };
    let terminal_result = loop {
        match interpreter.run_main_with_host_slice(&mut host, 20_000) {
            zirk_interpreter::ExecutionResult::Yielded => {
                let tick = js_sys::Promise::resolve(&wasm_bindgen::JsValue::UNDEFINED);
                let _ = wasm_bindgen_futures::JsFuture::from(tick).await;
            }
            zirk_interpreter::ExecutionResult::HttpSuspended { request } => {
                if request.body.len() > max_request_bytes {
                    let _ = interpreter.resume_http_error(request.id, 4);
                    continue;
                }
                match fetch_http_request(
                    &fetch_function,
                    &request,
                    &host.allowed_origins,
                    max_response_bytes,
                    &host.abort_signal,
                )
                .await
                {
                    Ok(response) => {
                        if interpreter
                            .resume_http_response(request.id, response)
                            .is_err()
                        {
                            break zirk_interpreter::ExecutionResult::InternalFailure(
                                zirk_interpreter::InternalFailure {
                                    message:
                                        "browser HTTP response could not resume the interpreter"
                                            .into(),
                                    span: None,
                                },
                            );
                        }
                    }
                    Err(error_code) => {
                        if interpreter
                            .resume_http_error(request.id, error_code)
                            .is_err()
                        {
                            break zirk_interpreter::ExecutionResult::InternalFailure(
                                zirk_interpreter::InternalFailure {
                                    message:
                                        "browser HTTP failure could not resume the interpreter"
                                            .into(),
                                    span: None,
                                },
                            );
                        }
                    }
                }
            }
            terminal => break terminal,
        }
    };
    if let (Some(limits), Some(options)) = (trace_limits, trace_options.as_ref()) {
        response.trace = Some(convert_trace(interpreter.take_trace(), limits, options));
    }
    response.output = host
        .events
        .into_iter()
        .map(|event| OutputEvent {
            text: event.text,
            destination: match event.destination {
                zirk_interpreter::OutputDestination::Stdout => "stdout".into(),
                zirk_interpreter::OutputDestination::Stderr => "stderr".into(),
            },
            truncated: event.truncated,
        })
        .collect();
    response.completion = None;
    response.failure = execution_failure(terminal_result, &mut response.completion);
    response
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
async fn fetch_http_request(
    fetch_function: &js_sys::Function,
    request: &zirk_interpreter::HttpHostRequest,
    allowed_origins: &std::collections::HashSet<String>,
    max_response_bytes: usize,
    abort_signal: &wasm_bindgen::JsValue,
) -> Result<zirk_interpreter::HttpHostResponse, i32> {
    use js_sys::{Array, Function, Promise, Reflect, Uint8Array};
    use wasm_bindgen::{JsCast, JsValue};
    use zirk_runtime_core::http_values::{Headers, HttpMethod, HttpRequestValues, Url};

    if abort_signal_aborted(abort_signal) {
        return Err(7);
    }
    let origin = request_origin(&request.url).ok_or(0)?;
    if !allowed_origins.contains(&origin) {
        return Err(16);
    }
    if !valid_http_method_token(&request.method) {
        return Err(15);
    }
    let method = match request.method.as_str() {
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "OPTIONS" => HttpMethod::Options,
        "CONNECT" => HttpMethod::Connect,
        "TRACE" => HttpMethod::Trace,
        "QUERY" => HttpMethod::Query,
        _ => HttpMethod::Other,
    };
    let url = Url::parse(&request.url).map_err(|_| 0)?;
    let mut validated_headers = Headers::new();
    for (name, value) in &request.headers {
        validated_headers.append(name, value).map_err(|_| 3)?;
    }
    let mut values = HttpRequestValues::new(method, url);
    values.method_token.clone_from(&request.method);
    values.headers = validated_headers;
    values.set_body(request.body.clone()).map_err(|_| 4)?;
    values
        .validate_client_request()
        .map_err(|error| match error {
            zirk_runtime_core::http_values::HttpValueError::UnsupportedMethod => 14,
            _ => 3,
        })?;
    if request.validate_json && serde_json::from_slice::<serde_json::Value>(&request.body).is_err()
    {
        return Err(9);
    }
    let global = js_sys::global();
    let headers_constructor = Reflect::get(&global, &"Headers".into())
        .map_err(|_| 16)?
        .dyn_into::<Function>()
        .map_err(|_| 16)?;
    let headers = Reflect::construct(&headers_constructor, &Array::new()).map_err(|_| 16)?;
    let append_header = Reflect::get(&headers, &"append".into())
        .map_err(|_| 16)?
        .dyn_into::<Function>()
        .map_err(|_| 16)?;
    for (name, bytes) in &request.headers {
        let value = String::from_utf8(bytes.clone()).map_err(|_| 3)?;
        append_header
            .call2(
                &headers,
                &JsValue::from_str(name),
                &JsValue::from_str(&value),
            )
            .map_err(|_| 3)?;
    }

    let options = js_sys::Object::new();
    set_js_property(&options, "method", &JsValue::from_str(&request.method)).map_err(|_| 3)?;
    set_js_property(&options, "headers", &headers).map_err(|_| 3)?;
    set_js_property(&options, "mode", &JsValue::from_str("cors")).map_err(|_| 3)?;
    set_js_property(&options, "credentials", &JsValue::from_str("omit")).map_err(|_| 3)?;
    set_js_property(&options, "redirect", &JsValue::from_str("error")).map_err(|_| 3)?;
    let cache = if request.cached {
        "default"
    } else {
        "no-store"
    };
    set_js_property(&options, "cache", &JsValue::from_str(cache)).map_err(|_| 3)?;
    if !abort_signal.is_null() && !abort_signal.is_undefined() {
        set_js_property(&options, "signal", abort_signal).map_err(|_| 3)?;
    }
    if !request.body.is_empty() {
        let body = Uint8Array::from(request.body.as_slice());
        set_js_property(&options, "body", &body.into()).map_err(|_| 3)?;
    }
    let fetch_promise = fetch_function
        .call2(&global, &JsValue::from_str(&request.url), &options.into())
        .map_err(|_| {
            if abort_signal_aborted(abort_signal) {
                7
            } else {
                16
            }
        })?;
    let response = wasm_bindgen_futures::JsFuture::from(Promise::resolve(&fetch_promise))
        .await
        .map_err(|_| {
            if abort_signal_aborted(abort_signal) {
                7
            } else {
                16
            }
        })?;
    let status = Reflect::get(&response, &"status".into())
        .map_err(|_| 16)?
        .as_f64()
        .filter(|status| status.fract() == 0.0 && (100.0..=999.0).contains(status))
        .ok_or(16)? as u16;

    let response_headers = Reflect::get(&response, &"headers".into()).map_err(|_| 16)?;
    let entries_function = Reflect::get(&response_headers, &"entries".into())
        .map_err(|_| 16)?
        .dyn_into::<Function>()
        .map_err(|_| 16)?;
    let entries = entries_function.call0(&response_headers).map_err(|_| 16)?;
    let iterator = js_sys::try_iter(&entries).map_err(|_| 16)?.ok_or(16)?;
    let mut headers = Vec::new();
    let mut header_bytes = 0usize;
    for entry in iterator {
        let pair = Array::from(&entry.map_err(|_| 16)?);
        if pair.length() != 2 || headers.len() >= 256 {
            return Err(3);
        }
        let name = pair.get(0).as_string().ok_or(3)?;
        let value = pair.get(1).as_string().ok_or(3)?;
        header_bytes = header_bytes
            .checked_add(name.len() + value.len())
            .ok_or(3)?;
        if header_bytes > 64 * 1024 {
            return Err(3);
        }
        headers.push((name, value.into_bytes()));
    }

    let body_stream = Reflect::get(&response, &"body".into()).map_err(|_| 16)?;
    let mut body = Vec::new();
    if !body_stream.is_null() && !body_stream.is_undefined() {
        let reader = Reflect::get(&body_stream, &"getReader".into())
            .map_err(|_| 16)?
            .dyn_into::<Function>()
            .map_err(|_| 16)?
            .call0(&body_stream)
            .map_err(|_| 16)?;
        let read = Reflect::get(&reader, &"read".into())
            .map_err(|_| 16)?
            .dyn_into::<Function>()
            .map_err(|_| 16)?;
        loop {
            if abort_signal_aborted(abort_signal) {
                if let Ok(cancel) = Reflect::get(&reader, &"cancel".into())
                    && let Ok(cancel) = cancel.dyn_into::<Function>()
                {
                    let _ = cancel.call0(&reader);
                }
                return Err(7);
            }
            let read_promise = read.call0(&reader).map_err(|_| 16)?;
            let result = wasm_bindgen_futures::JsFuture::from(Promise::resolve(&read_promise))
                .await
                .map_err(|_| {
                    if abort_signal_aborted(abort_signal) {
                        7
                    } else {
                        16
                    }
                })?;
            if Reflect::get(&result, &"done".into())
                .map_err(|_| 16)?
                .as_bool()
                .unwrap_or(false)
            {
                break;
            }
            let chunk = Uint8Array::new(&Reflect::get(&result, &"value".into()).map_err(|_| 16)?);
            let next_len = body.len().checked_add(chunk.length() as usize).ok_or(4)?;
            if next_len > max_response_bytes {
                if let Ok(cancel) = Reflect::get(&reader, &"cancel".into())
                    && let Ok(cancel) = cancel.dyn_into::<Function>()
                {
                    let _ = cancel.call0(&reader);
                }
                return Err(4);
            }
            body.extend_from_slice(&chunk.to_vec());
        }
    }
    Ok(zirk_interpreter::HttpHostResponse {
        status,
        headers,
        body,
    })
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
fn valid_http_method_token(method: &str) -> bool {
    !method.is_empty()
        && method.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
fn set_js_property(
    target: &js_sys::Object,
    name: &str,
    value: &wasm_bindgen::JsValue,
) -> Result<(), wasm_bindgen::JsValue> {
    js_sys::Reflect::set(target, &name.into(), value).map(|_| ())
}

#[cfg(all(target_arch = "wasm32", feature = "http-fetch"))]
fn abort_signal_aborted(signal: &wasm_bindgen::JsValue) -> bool {
    !signal.is_null()
        && !signal.is_undefined()
        && js_sys::Reflect::get(signal, &"aborted".into())
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
}

fn convert_trace(
    trace: zirk_interpreter::Trace,
    limits: zirk_interpreter::TraceLimits,
    options: &contract::TraceOptions,
) -> contract::Trace {
    use zirk_interpreter::TraceEventKind as Kind;
    let wanted = |name: &str| {
        options
            .kinds
            .as_ref()
            .is_none_or(|kinds| kinds.iter().any(|kind| kind == name))
    };
    let events = trace
        .events
        .into_iter()
        .filter_map(|event| {
            let mut dto = contract::TraceEvent {
                seq: event.seq,
                ..Default::default()
            };
            let name = match event.kind {
                Kind::Alloc { object, class } => {
                    dto.object = Some(object);
                    dto.class = Some(class);
                    "alloc"
                }
                Kind::FieldWrite {
                    object,
                    field,
                    ty,
                    value,
                    truncated,
                } => {
                    dto.object = Some(object);
                    dto.field = Some(field);
                    dto.ty = Some(ty);
                    dto.value = Some(value);
                    dto.value_truncated = truncated.then_some(true);
                    "fieldWrite"
                }
                Kind::Call { callee, receiver } => {
                    dto.callee = Some(callee);
                    dto.receiver = receiver;
                    "call"
                }
                Kind::Return { callee } => {
                    dto.callee = Some(callee);
                    "return"
                }
                Kind::Throw { object } => {
                    dto.object = object;
                    "throw"
                }
                Kind::Output { index } => {
                    dto.output_index = Some(index);
                    "output"
                }
            };
            dto.kind = name.into();
            wanted(name).then_some(dto)
        })
        .collect();
    contract::Trace {
        events,
        truncated: trace.truncated,
        limits: contract::TraceLimits {
            events: limits.max_events,
            bytes: limits.max_bytes,
            value_bytes: limits.max_value_bytes,
        },
    }
}

fn execution_failure(
    result: zirk_interpreter::ExecutionResult,
    completion: &mut Option<contract::Completion>,
) -> Option<contract::Failure> {
    use contract::{Completion, Failure, FailureKind};
    match result {
        zirk_interpreter::ExecutionResult::Yielded => Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB198".into(),
            message: "synchronous browser run cannot resume a suspended interpreter".into(),
            limit: None,
        }),
        #[cfg(feature = "http-fetch")]
        zirk_interpreter::ExecutionResult::HttpSuspended { .. } => Some(Failure {
            kind: FailureKind::Capability,
            code: "ZB006".into(),
            message: "HTTP requires the asynchronous browser execution API and an explicit fetch capability".into(),
            limit: None,
        }),
        zirk_interpreter::ExecutionResult::Completed { .. } => {
            *completion = Some(Completion::Completed);
            None
        }
        zirk_interpreter::ExecutionResult::RuntimeFailure(error) => Some(Failure {
            kind: FailureKind::Runtime,
            code: error.category,
            message: error.message,
            limit: None,
        }),
        zirk_interpreter::ExecutionResult::CapabilityFailure(error) => Some(Failure {
            kind: FailureKind::Capability,
            code: error.code,
            message: error.message,
            limit: None,
        }),
        zirk_interpreter::ExecutionResult::LimitFailure(error) => Some(Failure {
            kind: FailureKind::Limit,
            code: "ZB102".into(),
            message: format!("execution limit exceeded: {error:?}"),
            limit: Some(format!("{error:?}").to_lowercase()),
        }),
        zirk_interpreter::ExecutionResult::InternalFailure(error) => Some(Failure {
            kind: FailureKind::Internal,
            code: "ZB198".into(),
            // The browser contract intentionally never exposes Rust error
            // chains, paths, or debug output to the caller.
            message: if error.message.is_empty() {
                "internal interpreter failure".into()
            } else {
                error.message
            },
            limit: None,
        }),
    }
}

/// Converts the diagnostics crate's one-based Unicode line/column location
/// back into a byte offset in the caller-owned source. Diagnostics currently
/// retain a location rather than their originating `Span`; this conversion is
/// bounded by the already accepted source and never consults a host path.
fn diagnostic_start(source: &str, location: &zirk_diagnostics::Location) -> u32 {
    let line_start = source
        .split_inclusive('\n')
        .take(location.line.saturating_sub(1) as usize)
        .map(str::len)
        .sum::<usize>();
    let column_bytes = source[line_start..]
        .chars()
        .take(location.column.saturating_sub(1) as usize)
        .map(char::len_utf8)
        .sum::<usize>();
    (line_start + column_bytes).min(source.len()) as u32
}

#[cfg(test)]
mod tests {
    use super::{
        API_VERSION, BrowserApi, bundled_standard_source_ids, check, check_source, format, run,
    };
    use crate::contract::{FailureKind, Limits, Request};

    #[test]
    fn exposes_the_initial_contract_revision() {
        assert_eq!(API_VERSION, 1);
    }

    #[test]
    fn format_is_additive_v1_and_returns_source_or_diagnostics() {
        let valid = format(Request {
            api_version: API_VERSION,
            trace: None,
            source: "fn main(): Void { }\n".into(),
            virtual_name: "editor.zrk".into(),
            limits: None,
        });
        assert_eq!(valid.operation, "format");
        assert_eq!(
            valid.formatted_source.as_deref(),
            Some("fn main(): Void { }\n")
        );
        assert!(valid.failure.is_none());

        let invalid = format(Request {
            api_version: API_VERSION,
            trace: None,
            source: "fn main(: Void {".into(),
            virtual_name: "editor.zrk".into(),
            limits: None,
        });
        assert!(invalid.formatted_source.is_none());
        assert!(!invalid.diagnostics.is_empty());

        let api = BrowserApi::new();
        let before: crate::contract::Response = serde_json::from_str(&api.format_json(
            r#"{"apiVersion":1,"source":"fn main(): Void {}","virtualName":"editor.zrk"}"#,
        ))
        .unwrap();
        assert!(matches!(
            before.failure,
            Some(crate::contract::Failure {
                kind: FailureKind::Request,
                ..
            })
        ));
    }

    #[test]
    fn checks_a_virtual_source_without_reading_a_path() {
        let (result, diagnostics) = check_source("fn main(): Void {}", "playground.zrk");
        assert!(diagnostics.is_empty());
        assert!(result.is_some());
    }

    #[test]
    fn interpreter_executes_scalar_singleton_union_matches() {
        let response = run(Request {
            api_version: API_VERSION,
            source: "import { stdout } from std.io;\n\
                type Day = Date(2026, 12, 25) | Date(2027, 1, 1);\n\
                type Wait = 250ms | 1s;\n\
                type ClockTime = Time(9, 30) | Time(17, 0);\n\
                type Moment = DateTime(Date(2026, 12, 25), Time(9, 30)) | DateTime(Date(2027, 1, 1), Time(0, 0));\n\
                type CivilOrClock = Date(2026, 12, 25) | Time(9, 30);\n\
                type Number = 1 | 2;\n\
                type Word = \"one\" | \"two\";\n\
                type Letter = 'a' | 'b';\n\
                type Ratio = 1.5f64 | 2.5f64;\n\
                type Money = 1.25 | 2.5;\n\
                type Flag = true | false;\n\
                fn day(value: Day): String { return match value { Date(2026, 12, 25) => \"holiday\"; Date(2027, 1, 1) => \"new year\"; }; }\n\
                fn wait(value: Wait): String { return match value { 250ms => \"short\"; 1s => \"long\"; }; }\n\
                fn clock_time(value: ClockTime): String { return match value { Time(9, 30) => \"morning\"; Time(17, 0) => \"evening\"; }; }\n\
                fn moment(value: Moment): String { return match value { DateTime(Date(2026, 12, 25), Time(9, 30)) => \"holiday\"; DateTime(Date(2027, 1, 1), Time(0, 0)) => \"new year\"; }; }\n\
                fn civil_or_clock(value: CivilOrClock): String { return match value { Date(2026, 12, 25) => \"date\"; Time(9, 30) => \"time\"; }; }\n\
                fn number(value: Number): String { return match value { 1 => \"one\"; 2 => \"two\"; }; }\n\
                fn word(value: Word): String { return match value { \"one\" => \"first\"; \"two\" => \"second\"; }; }\n\
                fn letter(value: Letter): String { return match value { 'a' => \"first\"; 'b' => \"second\"; }; }\n\
                fn ratio(value: Ratio): String { return match value { 1.5f64 => \"first\"; 2.5f64 => \"second\"; }; }\n\
                fn money(value: Money): String { return match value { 1.25 => \"first\"; 2.5 => \"second\"; }; }\n\
                fn flag(value: Flag): String { return match value { true => \"yes\"; false => \"no\"; }; }\n\
                fn main(): Void { stdout.println(day(Date(2026, 12, 25))); stdout.println(wait(1s)); stdout.println(clock_time(Time(17, 0))); stdout.println(moment(DateTime(Date(2027, 1, 1), Time(0, 0)))); stdout.println(civil_or_clock(Time(9, 30))); stdout.println(number(2)); stdout.println(word(\"two\")); stdout.println(letter('b')); stdout.println(ratio(2.5f64)); stdout.println(money(2.5)); stdout.println(flag(true)); }\n\
                main();\n".into(),
            virtual_name: "singleton-unions.zrk".into(),
            limits: None,
            trace: None,
        });
        assert!(response.failure.is_none(), "{:?}", response.failure);
        assert!(
            response.diagnostics.is_empty(),
            "{:?}",
            response.diagnostics
        );
        assert_eq!(
            response
                .output
                .iter()
                .map(|event| event.text.as_str())
                .collect::<String>(),
            "holiday\nlong\nevening\nnew year\ntime\ntwo\nsecond\nsecond\nsecond\nsecond\nyes\n"
        );
    }

    #[test]
    fn preserves_frontend_diagnostics_for_in_memory_source() {
        let (result, diagnostics) = check_source("fn main(): Void { unknown; }", "editor.zrk");
        assert!(result.is_none());
        assert_eq!(diagnostics.diagnostics()[0].code.as_str(), "E0401");
        assert_eq!(
            diagnostics.diagnostics()[0]
                .location
                .as_ref()
                .map(|location| location.file.as_str()),
            Some("editor.zrk")
        );
    }

    #[test]
    fn in_memory_frontend_matches_file_backed_cli_diagnostic_order() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/invalid-unknown.zrk"
        ));
        let mut file_sink = zirk_diagnostics::DiagnosticSink::new();
        let file_result = zirk_cli::frontend::run_frontend(path, &mut file_sink);
        let (memory_result, memory_sink) = check_source(
            include_str!("../fixtures/invalid-unknown.zrk"),
            path.to_string_lossy(),
        );
        assert!(file_result.is_none());
        assert!(memory_result.is_none());
        let file = file_sink
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.code.as_str().to_owned(),
                    diagnostic.severity.as_str().to_owned(),
                    diagnostic.message.clone(),
                    diagnostic.location.clone().map(|mut location| {
                        location.file = location.file.replace('\\', "/");
                        location
                    }),
                )
            })
            .collect::<Vec<_>>();
        let memory = memory_sink
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.code.as_str().to_owned(),
                    diagnostic.severity.as_str().to_owned(),
                    diagnostic.message.clone(),
                    diagnostic.location.clone().map(|mut location| {
                        location.file = location.file.replace('\\', "/");
                        location
                    }),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(memory, file);
    }

    #[test]
    fn rejects_oversized_source_before_the_lexer_runs() {
        let response = check(Request {
            api_version: API_VERSION,
            trace: None,
            source: "x".repeat(5),
            virtual_name: "editor.zrk".into(),
            limits: Some(Limits {
                source_bytes: 4,
                ..Limits::MAX
            }),
        });
        assert!(response.diagnostics.is_empty());
        assert!(matches!(
            response.failure,
            Some(crate::contract::Failure { kind: FailureKind::Limit, ref limit, .. })
                if limit.as_deref() == Some("source")
        ));
    }

    #[test]
    fn check_keeps_compiler_errors_out_of_execution() {
        let response = check(Request {
            api_version: API_VERSION,
            trace: None,
            source: "fn main(): Void { unknown; }".into(),
            virtual_name: "editor.zrk".into(),
            limits: None,
        });
        assert_eq!(response.operation, "check");
        assert!(response.completion.is_none());
        assert!(response.failure.is_none());
        assert_eq!(response.diagnostics[0].code, "E0401");
        assert!(response.diagnostics[0].start < response.diagnostics[0].end);
        assert!(response.output.is_empty());
    }

    #[test]
    fn browser_frontend_checks_http_and_environment_from_bundled_sources() {
        for source in [
            "import { HTTP, HTTPClient, HTTPHeaders, URL } from std.http;\nfn main(): Void {}",
            "import { Environment } from std.environment;\nfn main(): Void {}",
        ] {
            let (frontend, diagnostics) = check_source(source, "stdlib-browser.zrk");
            assert!(
                frontend.is_some(),
                "bundled standard source did not check: {:?}",
                diagnostics.diagnostics()
            );
            assert!(!diagnostics.has_errors());
        }
    }

    #[test]
    fn browser_can_parse_and_resolve_http_urls_without_network_authority() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { stdout } from std.io;\nimport { URL } from std.http;\nfn main(): Void { match URL.parse(\"HTTPS://Example.COM:443/a/b\") { Ok(base) => { match URL.resolve(base, \"../c?q=1\") { Ok(url) => stdout.println(url.as_string()); Error(_) => stdout.println(\"resolve-error\"); } } Error(_) => stdout.println(\"parse-error\"); } match URL.parse(\"ftp://example.com/\") { Ok(_) => stdout.println(\"unexpected-ok\"); Error(_) => stdout.println(\"invalid-url\"); } }".into(),
            virtual_name: "browser-http-url.zrk".into(),
            limits: None,
        });
        assert!(
            response.failure.is_none(),
            "URL operations are portable: {response:?}"
        );
        assert_eq!(response.output.len(), 2);
        assert_eq!(response.output[0].text, "https://example.com/c?q=1\n");
        assert_eq!(response.output[1].text, "invalid-url\n");
    }

    #[test]
    fn browser_run_rejects_http_until_a_fetch_capability_is_injected() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { HTTP, URL } from std.http;\nfn main(): Void { match URL.parse(\"https://example.com/\") { Ok(url) => { _ = HTTP.get(url); } Error(_) => {} } }".into(),
            virtual_name: "browser-http.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(
                response.failure,
                Some(crate::contract::Failure {
                    kind: FailureKind::Capability,
                    ..
                })
            ),
            "browser HTTP must remain fail-closed without a host capability: {response:?}"
        );
        assert!(response.output.is_empty());

        let request_response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { HTTP, HTTPHeaders, HTTPMethod, URL } from std.http;\nfn main(): Void { mut body: Array<Byte> = [1 as Byte]; match URL.parse(\"https://example.com/submit\") { Ok(url) => { _ = HTTP.request(HTTPMethod.Post, url, body, HTTPHeaders.create()); } Error(_) => {} } }".into(),
            virtual_name: "browser-http-request.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(
                request_response.failure,
                Some(crate::contract::Failure {
                    kind: FailureKind::Capability,
                    ..
                })
            ),
            "general browser requests must stay default-deny without an injected fetch capability: {request_response:?}"
        );
    }

    #[test]
    fn interpreter_suspends_and_resumes_http_get_without_restarting_main() {
        let source = "import { stdout } from std.io;\nimport { HTTP, URL } from std.http;\nfn main(): Void { stdout.println(\"before\"); match URL.parse(\"https://example.com/path\") { Ok(url) => { match HTTP.get(url) { Ok(response) => stdout.println(response.status_code()); Error(_) => stdout.println(\"http-error\"); } } Error(_) => stdout.println(\"url-error\"); } }";
        let (frontend, diagnostics) = check_source(source, "browser-http-resume.zrk");
        assert!(
            !diagnostics.has_errors(),
            "source diagnostics: {:?}",
            diagnostics.diagnostics()
        );
        let frontend = frontend.expect("valid HTTP fixture");
        let standard_sources = bundled_standard_source_ids(&frontend.sources);
        let module = zirk_ir::lower(&frontend.program, &frontend.checked);
        let mut interpreter = zirk_interpreter::Interpreter::new(
            &module,
            zirk_interpreter::InterpreterLimits::default(),
        )
        .expect("verified browser fixture");
        interpreter.set_bundled_standard_sources(standard_sources.clone());
        interpreter.enable_trace(zirk_interpreter::TraceLimits::DEFAULT);
        let mut host = zirk_interpreter::CapturedHost::with_http_capability();

        let request = match interpreter.run_main_with_host(&mut host) {
            zirk_interpreter::ExecutionResult::HttpSuspended { request } => request,
            other => panic!("expected one suspended HTTP request, got {other:?}"),
        };
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "https://example.com/path");
        assert_eq!(
            host.events().len(),
            1,
            "main ran up to the suspension point"
        );
        assert_eq!(host.events()[0].text, "before\n");
        let trace_before_resume = interpreter.take_trace();
        assert!(trace_before_resume.events.iter().any(|event| {
            matches!(
                event.kind,
                zirk_interpreter::TraceEventKind::Output { index: 0 }
            )
        }));
        assert!(
            interpreter
                .resume_http_response(
                    request.id + 1,
                    zirk_interpreter::HttpHostResponse {
                        status: 201,
                        headers: vec![("x-test".into(), b"value".to_vec())],
                        body: b"payload".to_vec(),
                    },
                )
                .is_err(),
            "stale responses must be rejected"
        );
        interpreter
            .resume_http_response(
                request.id,
                zirk_interpreter::HttpHostResponse {
                    status: 201,
                    headers: vec![("x-test".into(), b"value".to_vec())],
                    body: b"payload".to_vec(),
                },
            )
            .expect("matching response resumes the request");
        assert!(
            interpreter
                .resume_http_response(
                    request.id,
                    zirk_interpreter::HttpHostResponse {
                        status: 200,
                        headers: Vec::new(),
                        body: Vec::new(),
                    },
                )
                .is_err(),
            "duplicate responses must be rejected"
        );
        assert!(matches!(
            interpreter.run_main_with_host(&mut host),
            zirk_interpreter::ExecutionResult::Completed { value: None }
        ));
        assert_eq!(
            host.events()
                .iter()
                .map(|event| event.text.as_str())
                .collect::<Vec<_>>(),
            ["before\n", "201\n"],
            "resuming must continue after the suspended instruction"
        );
        let trace_after_resume = interpreter.take_trace();
        assert!(trace_after_resume.events.iter().any(|event| {
            matches!(
                event.kind,
                zirk_interpreter::TraceEventKind::Output { index: 1 }
            )
        }));
        assert!(
            trace_after_resume.events.first().unwrap().seq
                > trace_before_resume.events.last().unwrap().seq,
            "trace sequence must continue across the HTTP suspension"
        );
        assert_eq!(host.http_requests().len(), 1);

        let mut cancelled_interpreter = zirk_interpreter::Interpreter::new(
            &module,
            zirk_interpreter::InterpreterLimits::default(),
        )
        .expect("verified browser fixture");
        cancelled_interpreter.set_bundled_standard_sources(standard_sources);
        let mut cancelled_host = zirk_interpreter::CapturedHost::with_http_capability();
        let cancelled_request = match cancelled_interpreter.run_main_with_host(&mut cancelled_host)
        {
            zirk_interpreter::ExecutionResult::HttpSuspended { request } => request,
            other => panic!("expected suspended request before cancellation, got {other:?}"),
        };
        cancelled_host.cancel();
        assert_eq!(
            cancelled_interpreter.run_main_with_host_slice(&mut cancelled_host, 1),
            zirk_interpreter::ExecutionResult::LimitFailure(
                zirk_interpreter::LimitFailure::Cancelled
            )
        );
        assert!(
            cancelled_interpreter
                .resume_http_response(
                    cancelled_request.id,
                    zirk_interpreter::HttpHostResponse {
                        status: 200,
                        headers: Vec::new(),
                        body: Vec::new(),
                    },
                )
                .is_err(),
            "cancelled requests must release their continuation"
        );
    }

    #[test]
    fn interpreter_suspends_and_resumes_http_request_with_body_and_headers() {
        let source = "import { stdout } from std.io;\nimport { HTTP, HTTPHeaders, HTTPMethod, URL } from std.http;\nfn main(): Void { mut payload: Array<Byte> = [112 as Byte, 97 as Byte, 121 as Byte, 108 as Byte, 111 as Byte, 97 as Byte, 100 as Byte]; mut headers = HTTPHeaders.create(); _ = headers.add(\"x-test\", \"yes\"); match URL.parse(\"https://example.com/submit\") { Ok(url) => { match HTTP.request(HTTPMethod.Post, url, payload, headers) { Ok(response) => { stdout.println(response.status_code()); match response.text() { Ok(text) => stdout.println(text); Error(_) => stdout.println(\"text-error\"); } } Error(_) => stdout.println(\"request-error\"); } } Error(_) => stdout.println(\"url-error\"); } }";
        let (frontend, diagnostics) = check_source(source, "browser-http-request-resume.zrk");
        assert!(
            !diagnostics.has_errors(),
            "source diagnostics: {:?}",
            diagnostics.diagnostics()
        );
        let frontend = frontend.expect("valid HTTP request fixture");
        let standard_sources = bundled_standard_source_ids(&frontend.sources);
        let module = zirk_ir::lower(&frontend.program, &frontend.checked);
        let mut interpreter = zirk_interpreter::Interpreter::new(
            &module,
            zirk_interpreter::InterpreterLimits::default(),
        )
        .expect("verified browser HTTP request fixture");
        interpreter.set_bundled_standard_sources(standard_sources);
        let mut host = zirk_interpreter::CapturedHost::with_http_capability();
        let request = match interpreter.run_main_with_host(&mut host) {
            zirk_interpreter::ExecutionResult::HttpSuspended { request } => request,
            other => panic!("expected suspended HTTP request, got {other:?}"),
        };
        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://example.com/submit");
        assert_eq!(request.body, b"payload");
        assert_eq!(request.headers, [("x-test".into(), b"yes".to_vec())]);
        assert!(!request.validate_json);

        interpreter
            .resume_http_response(
                request.id,
                zirk_interpreter::HttpHostResponse {
                    status: 202,
                    headers: Vec::new(),
                    body: b"response body".to_vec(),
                },
            )
            .expect("matching HTTP request response resumes execution");
        assert!(matches!(
            interpreter.run_main_with_host(&mut host),
            zirk_interpreter::ExecutionResult::Completed { value: None }
        ));
        assert_eq!(host.events()[0].text, "202\n");
        assert_eq!(host.events()[1].text, "response body\n");
        assert_eq!(host.http_requests(), &[request]);
    }

    #[test]
    fn interpreter_maps_http_convenience_calls_to_bounded_host_requests() {
        let source = r#"
import { HTTP, URL } from std.http;
fn main(): Void {
    match URL.parse("https://example.com/api") {
        Ok(url) => {
            _ = HTTP.head(url);
            _ = HTTP.query(url, "true");
            _ = HTTP.query_with_content_type(url, "select 1", "application/sql");
            _ = HTTP.post(url, "plain text");
            _ = HTTP.post_json(url, "true");
            mut bytes: Array<Byte> = [1 as Byte, 255 as Byte];
            _ = HTTP.post_bytes(url, bytes);
        }
        Error(_) => {}
    }
}
"#;
        let (frontend, diagnostics) = check_source(source, "browser-http-convenience.zrk");
        assert!(
            !diagnostics.has_errors(),
            "source diagnostics: {:?}",
            diagnostics.diagnostics()
        );
        let frontend = frontend.expect("valid HTTP convenience fixture");
        let standard_sources = bundled_standard_source_ids(&frontend.sources);
        let module = zirk_ir::lower(&frontend.program, &frontend.checked);
        let mut interpreter = zirk_interpreter::Interpreter::new(
            &module,
            zirk_interpreter::InterpreterLimits::default(),
        )
        .expect("verified HTTP convenience fixture");
        interpreter.set_bundled_standard_sources(standard_sources);
        let mut host = zirk_interpreter::CapturedHost::with_http_capability();
        loop {
            match interpreter.run_main_with_host(&mut host) {
                zirk_interpreter::ExecutionResult::HttpSuspended { request } => interpreter
                    .resume_http_response(
                        request.id,
                        zirk_interpreter::HttpHostResponse {
                            status: 204,
                            headers: Vec::new(),
                            body: Vec::new(),
                        },
                    )
                    .expect("each convenience request resumes the same interpreter"),
                zirk_interpreter::ExecutionResult::Completed { value: None } => break,
                other => panic!("expected HTTP request suspension or completion, got {other:?}"),
            }
        }
        let requests = host.http_requests();
        assert_eq!(requests.len(), 6);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.method.as_str())
                .collect::<Vec<_>>(),
            ["HEAD", "QUERY", "QUERY", "POST", "POST", "POST"]
        );
        assert_eq!(requests[0].body, b"");
        assert_eq!(requests[1].body, b"true");
        assert_eq!(
            requests[1].headers,
            [("content-type".into(), b"application/json".to_vec())]
        );
        assert!(requests[1].validate_json);
        assert_eq!(requests[2].body, b"select 1");
        assert_eq!(
            requests[2].headers,
            [("content-type".into(), b"application/sql".to_vec())]
        );
        assert!(!requests[2].validate_json);
        assert_eq!(requests[3].body, b"plain text");
        assert_eq!(
            requests[3].headers,
            [("content-type".into(), b"text/plain; charset=utf-8".to_vec())]
        );
        assert_eq!(requests[4].body, b"true");
        assert!(requests[4].validate_json);
        assert_eq!(
            requests[4].headers,
            [("content-type".into(), b"application/json".to_vec())]
        );
        assert_eq!(requests[5].body, [1, 255]);
        assert_eq!(
            requests[5].headers,
            [("content-type".into(), b"application/octet-stream".to_vec())]
        );
    }

    #[test]
    fn run_lowers_verifies_and_interprets_a_compatible_program() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source:
                "import { stdout } from std.io;\nfn main(): Void { stdout.println(\"hello\"); }"
                    .into(),
            virtual_name: "editor.zrk".into(),
            limits: None,
        });
        assert!(response.failure.is_none(), "{response:?}");
        assert!(response.completion.is_some());
        assert_eq!(response.output[0].text, "hello\n");
    }

    #[test]
    fn bundled_standard_package_sources_run_without_browser_filesystem_access() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { stdout } from std.io;\nfn main(): Void { println(\"package\"); }"
                .into(),
            virtual_name: "package-editor.zrk".into(),
            limits: None,
        });
        assert!(response.failure.is_none(), "{response:?}");
        assert_eq!(
            response.output.first().map(|event| event.text.as_str()),
            Some("package\n"),
            "{response:?}"
        );
    }

    #[test]
    fn browser_captures_standard_error_with_its_destination() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source:
                "import { stderr } from std.io;\nfn main(): Void { stderr.println(\"warning\"); }"
                    .into(),
            virtual_name: "stderr-editor.zrk".into(),
            limits: None,
        });
        assert!(response.failure.is_none(), "{response:?}");
        assert_eq!(response.output[0].text, "warning\n");
        assert_eq!(response.output[0].destination, "stderr");
    }

    #[test]
    fn browser_rejects_reachable_standard_input_before_execution() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { stdin } from std.io;\nfn main(): Void { stdin.read_line(); }".into(),
            virtual_name: "stdin-editor.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(
                response.failure,
                Some(crate::contract::Failure {
                    kind: FailureKind::Capability,
                    ..
                })
            ),
            "{response:?}"
        );
        assert!(response.output.is_empty());
    }

    #[test]
    fn browser_rejects_reachable_filesystem_access_before_execution() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { stdout } from std.io;\nimport { File } from std.fs::fs;\nimport { Path } from std.path::path;\nfn main(): Void { stdout.println(\"before\"); File.read_text(Path.of(\"secret.txt\"), 64); stdout.println(\"after\"); }".into(),
            virtual_name: "filesystem-editor.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(
                response.failure,
                Some(crate::contract::Failure {
                    kind: FailureKind::Capability,
                    ..
                })
            ),
            "{response:?}"
        );
        assert!(response.output.is_empty());
    }

    #[test]
    fn browser_rejects_async_filesystem_access_before_execution() {
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { stdout } from std.io;\nimport { File } from std.fs::fs;\nimport { Path } from std.path::path;\nfn main(): Void { stdout.println(\"before\"); _ = File.read_text_async(Path.of(\"secret.txt\"), 64); stdout.println(\"after\"); }".into(),
            virtual_name: "filesystem-async-editor.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(
                response.failure,
                Some(crate::contract::Failure {
                    kind: FailureKind::Capability,
                    ..
                })
            ),
            "{response:?}"
        );
        assert!(response.output.is_empty(), "{response:?}");
    }

    #[test]
    fn run_contract_covers_completion_runtime_capability_and_execution_limits() {
        let normal = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "import { stdout } from std.io;\nfn main(): Void { stdout.println(\"done\"); }"
                .into(),
            virtual_name: "normal.zrk".into(),
            limits: None,
        });
        assert_eq!(
            normal.completion,
            Some(crate::contract::Completion::Completed)
        );

        let fatal = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "fn main(): Void { fatalError(\"stop\"); }".into(),
            virtual_name: "fatal.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(fatal.failure, Some(crate::contract::Failure {
            kind: FailureKind::Runtime, ref code, ref message, ..
        }) if code == "FatalError" && message == "stop"),
            "{fatal:?}"
        );

        let capability = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: "extern \"C\" fn abs(n: Int32): Int32; fn main(): Void { unsafe { commit { abs(1); } } }".into(),
            virtual_name: "extern.zrk".into(),
            limits: None,
        });
        assert!(
            matches!(capability.failure, Some(crate::contract::Failure {
            kind: FailureKind::Capability, ref code, ..
        }) if code == "ZB002"),
            "{capability:?}"
        );

        let limited = run(Request {
            api_version: API_VERSION,
            trace: None,
            source:
                "import { stdout } from std.io;\nfn main(): Void { stdout.println(\"limit\"); }"
                    .into(),
            virtual_name: "limit.zrk".into(),
            limits: Some(Limits {
                instructions: 1,
                ..Limits::DEFAULT
            }),
        });
        assert!(
            matches!(limited.failure, Some(crate::contract::Failure {
            kind: FailureKind::Limit, ref limit, ..
        }) if limit.as_deref() == Some("instructions")),
            "{limited:?}"
        );
    }

    #[test]
    fn unexpected_internal_execution_failure_is_sanitized_at_the_contract_boundary() {
        let mut completion = None;
        let failure = super::execution_failure(
            zirk_interpreter::ExecutionResult::InternalFailure(zirk_interpreter::InternalFailure {
                message: String::new(),
                span: None,
            }),
            &mut completion,
        )
        .expect("internal failure response");
        assert_eq!(failure.kind, FailureKind::Internal);
        assert_eq!(failure.code, "ZB198");
        assert_eq!(failure.message, "internal interpreter failure");
        assert!(completion.is_none());
    }

    #[test]
    fn check_and_run_return_the_same_compiler_errors_without_output() {
        let request = Request {
            api_version: API_VERSION,
            trace: None,
            source: "fn main(): Void { unknown; }".into(),
            virtual_name: "editor.zrk".into(),
            limits: None,
        };
        let checked = check(request.clone());
        let ran = run(request);
        assert_eq!(checked.diagnostics, ran.diagnostics);
        assert!(ran.completion.is_none());
        assert!(ran.failure.is_none());
        assert!(ran.output.is_empty());
    }

    #[test]
    fn rejects_every_zero_resource_limit_before_frontend_or_execution() {
        let cases: &[(&str, Limits)] = &[
            (
                "source",
                Limits {
                    source_bytes: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "diagnostics",
                Limits {
                    diagnostics: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "instructions",
                Limits {
                    instructions: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "call-depth",
                Limits {
                    call_depth: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "heap",
                Limits {
                    heap_bytes: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "output-bytes",
                Limits {
                    output_bytes: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "output-events",
                Limits {
                    output_events: 0,
                    ..Limits::DEFAULT
                },
            ),
            (
                "stack-frames",
                Limits {
                    stack_frames: 0,
                    ..Limits::DEFAULT
                },
            ),
        ];
        for (expected, limits) in cases {
            let response = check(Request {
                api_version: API_VERSION,
                trace: None,
                source: "fn main(): Void { unknown; }".into(),
                virtual_name: "editor.zrk".into(),
                limits: Some(*limits),
            });
            assert!(response.diagnostics.is_empty(), "{expected}");
            assert!(
                matches!(response.failure,
                Some(crate::contract::Failure { kind: FailureKind::Limit, ref limit, .. })
                    if limit.as_deref() == Some(*expected)),
                "{response:?}"
            );
        }
    }

    #[test]
    fn wasm_api_requires_explicit_initialization_and_sanitizes_bad_json() {
        let mut api = BrowserApi::new();
        let before: crate::contract::Response =
            serde_json::from_str(&api.check_json("{}")).expect("response JSON");
        assert!(matches!(before.failure, Some(crate::contract::Failure {
            kind: FailureKind::Request, ref code, ..
        }) if code == "ZB103"));

        api.initialize();
        let malformed: crate::contract::Response =
            serde_json::from_str(&api.run_json("{")).expect("response JSON");
        assert!(matches!(malformed.failure, Some(crate::contract::Failure {
            kind: FailureKind::Request, ref code, ..
        }) if code == "ZB104"));
    }

    #[test]
    fn wasm_api_preserves_unknown_field_and_version_boundaries() {
        let mut api = BrowserApi::new();
        api.initialize();
        let unknown: crate::contract::Response =
            serde_json::from_str(&api.check_json(
                r#"{"apiVersion":1,"source":"","virtualName":"editor.zrk","extra":true}"#,
            ))
            .expect("response JSON");
        assert!(matches!(unknown.failure, Some(crate::contract::Failure {
            kind: FailureKind::Request, ref code, ..
        }) if code == "ZB104"));

        let version: crate::contract::Response = serde_json::from_str(
            &api.check_json(r#"{"apiVersion":2,"source":"","virtualName":"editor.zrk"}"#),
        )
        .expect("response JSON");
        assert!(matches!(version.failure, Some(crate::contract::Failure {
            kind: FailureKind::Version, ref code, ..
        }) if code == "ZB100"));
    }

    /// Every conformance fixture, in one place. The integration test
    /// `fixture_list_covers_every_fixture_file` fails when a file is added to
    /// `fixtures/` without being listed here.
    const FIXTURES: &[&str] = &[
        include_str!("../fixtures/class-construction-sugar.json"),
        include_str!("../fixtures/closure.json"),
        include_str!("../fixtures/std-io-mixed-values.json"),
        include_str!("../fixtures/clone-tree.json"),
        include_str!("../fixtures/clone-shared-child.json"),
        include_str!("../fixtures/clone-cycle.json"),
        include_str!("../fixtures/clone-polymorphic.json"),
        include_str!("../fixtures/clone-callable.json"),
        include_str!("../fixtures/decimal-compare.json"),
        include_str!("../fixtures/decimal-ops.json"),
        include_str!("../fixtures/direct-call.json"),
        include_str!("../fixtures/downcast.json"),
        include_str!("../fixtures/fatal-error.json"),
        include_str!("../fixtures/hello-world.json"),
        include_str!("../fixtures/int-division.json"),
        include_str!("../fixtures/length-loop.json"),
        include_str!("../fixtures/list-add.json"),
        include_str!("../fixtures/map-set.json"),
        include_str!("../fixtures/oop-constructors.json"),
        include_str!("../fixtures/oop-equality-contract.json"),
        include_str!("../fixtures/oop-exceptions-in-methods.json"),
        include_str!("../fixtures/oop-final-class.json"),
        include_str!("../fixtures/oop-generics-two-params.json"),
        include_str!("../fixtures/oop-identity-aliasing.json"),
        include_str!("../fixtures/oop-inheritance-chain.json"),
        include_str!("../fixtures/oop-contract-trait-default.json"),
        include_str!("../fixtures/oop-nested-inner-local.json"),
        include_str!("../fixtures/oop-polymorphic-collection.json"),
        include_str!("../fixtures/oop-objects-enums-in-classes.json"),
        include_str!("../fixtures/oop-static-mutable.json"),
        include_str!("../fixtures/oop-subclass-adds-method.json"),
        include_str!("../fixtures/oop-visibility-inheritance.json"),
        include_str!("../fixtures/regex.json"),
        include_str!("../fixtures/scalar-widths.json"),
        include_str!("../fixtures/set-add.json"),
        include_str!("../fixtures/temporal-range.json"),
        include_str!("../fixtures/typed-catch.json"),
        include_str!("../fixtures/uncaught-throwable-message.json"),
        include_str!("../fixtures/void-call.json"),
        include_str!("../fixtures/void-method-call.json"),
    ];

    #[test]
    fn deep_clone_work_is_charged_against_the_instruction_budget() {
        let program = |clone: bool| {
            format!(
                "import {{ stdout }} from std.io;\nclass Node {{ v: Int32; next: Node?; construct(v: Int32) {{ this.v = v; this.next = null; }} }}\nfn main(): Void {{\n    mut head = Node(0);\n    mut i = 1;\n    while i < 200 {{ mut n = Node(i); n.next = head; head = n; i = i + 1; }}\n    {}\n    stdout.println(\"done\");\n}}\n",
                if clone {
                    "mut copy = head.clone();"
                } else {
                    ""
                }
            )
        };
        let run_with = |source: String, instructions: u64| {
            run(Request {
                api_version: API_VERSION,
                trace: None,
                source,
                virtual_name: "clone-budget.zrk".into(),
                limits: Some(crate::contract::Limits {
                    instructions,
                    ..crate::contract::Limits::DEFAULT
                }),
            })
        };
        // Smallest budget at which the program without a clone completes.
        let (mut low, mut high) = (1_u64, 2_000_000_u64);
        while low < high {
            let mid = (low + high) / 2;
            if run_with(program(false), mid).completion.is_some() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        let with_clone = run_with(program(true), low);
        assert!(with_clone.completion.is_none(), "{with_clone:?}");
        assert!(matches!(
            with_clone.failure,
            Some(crate::contract::Failure {
                kind: FailureKind::Limit,
                ..
            })
        ));
        assert!(run_with(program(true), 2_000_000).completion.is_some());
    }

    const TRACE_SOURCE: &str = "import { stdout } from std.io;\nclass Leaf { v: Int32; construct(v: Int32) { this.v = v; } }\nclass Tree { name: String; leaf: Leaf; construct(name: String, leaf: Leaf) { this.name = name; this.leaf = leaf; } }\nfn main(): Void {\n    mut t = Tree(\"oak\", Leaf(1));\n    stdout.println(\"built\");\n    t.leaf.v = 2;\n}\n";

    fn traced(
        source: &str,
        trace: Option<crate::contract::TraceOptions>,
    ) -> crate::contract::Response {
        run(Request {
            api_version: API_VERSION,
            trace,
            source: source.into(),
            virtual_name: "trace.zrk".into(),
            limits: None,
        })
    }

    #[test]
    fn trace_is_absent_unless_requested() {
        let response = traced(TRACE_SOURCE, None);
        assert!(response.trace.is_none());
        assert!(
            !serde_json::to_string(&response)
                .unwrap()
                .contains("\"trace\"")
        );
    }

    #[test]
    fn trace_records_object_lifecycle_and_interleaves_output() {
        let response = traced(TRACE_SOURCE, Some(Default::default()));
        let trace = response.trace.expect("trace");
        assert!(!trace.truncated);
        let kinds: Vec<&str> = trace.events.iter().map(|e| e.kind.as_str()).collect();
        assert!(kinds.contains(&"alloc") && kinds.contains(&"fieldWrite"));
        assert!(
            trace
                .events
                .windows(2)
                .all(|pair| pair[0].seq < pair[1].seq)
        );
        let alloc_tree = trace
            .events
            .iter()
            .position(|e| e.class.as_deref() == Some("Tree"))
            .expect("Tree alloc");
        let output = trace
            .events
            .iter()
            .position(|e| e.kind == "output")
            .expect("output marker");
        let last_write = trace
            .events
            .iter()
            .rposition(|e| e.field.as_deref() == Some("v"))
            .expect("write");
        assert!(alloc_tree < output && output < last_write);
        // An object-valued field renders as a reference, never as contents.
        let leaf_write = trace
            .events
            .iter()
            .find(|e| e.field.as_deref() == Some("leaf"))
            .expect("leaf write");
        assert!(leaf_write.value.as_deref().unwrap().starts_with('#'));
        let name_write = trace
            .events
            .iter()
            .rev()
            .find(|e| e.field.as_deref() == Some("name"))
            .expect("name write");
        assert_eq!(name_write.value.as_deref(), Some("\"oak\""));
    }

    #[test]
    fn trace_is_deterministic_and_does_not_change_behavior() {
        let first = traced(TRACE_SOURCE, Some(Default::default())).trace;
        assert_eq!(first, traced(TRACE_SOURCE, Some(Default::default())).trace);
        for fixture_json in FIXTURES.iter().copied() {
            let fixture: crate::fixtures::Fixture = serde_json::from_str(fixture_json).unwrap();
            let plain = traced(&fixture.source, None);
            let mut with_trace = traced(&fixture.source, Some(Default::default()));
            assert!(with_trace.trace.is_some(), "{}", fixture.name);
            with_trace.trace = None;
            assert_eq!(plain, with_trace, "{}", fixture.name);
        }
    }

    #[test]
    fn trace_budget_truncates_without_failing_the_program() {
        let options = crate::contract::TraceOptions {
            events: Some(2),
            ..Default::default()
        };
        let response = traced(TRACE_SOURCE, Some(options));
        let trace = response.trace.expect("trace");
        assert_eq!(trace.events.len(), 2);
        assert!(trace.truncated);
        assert_eq!(
            response.completion,
            Some(crate::contract::Completion::Completed)
        );
    }

    #[test]
    fn long_loops_keep_the_trace_bounded() {
        let source = "class C { v: Int32; construct() { this.v = 0; } }\nfn main(): Void {\n    mut c = C();\n    mut i = 0;\n    while i < 20000 { c.v = i; i = i + 1; }\n}\n";
        let response = traced(source, Some(Default::default()));
        let trace = response.trace.expect("trace");
        assert!(trace.truncated);
        assert!(trace.events.len() <= trace.limits.events);
        assert_eq!(
            response.completion,
            Some(crate::contract::Completion::Completed)
        );
    }

    #[test]
    fn trace_selects_event_kinds_and_rejects_unknown_ones() {
        let only_alloc = crate::contract::TraceOptions {
            kinds: Some(vec!["alloc".into()]),
            ..Default::default()
        };
        let trace = traced(TRACE_SOURCE, Some(only_alloc)).trace.expect("trace");
        assert!(!trace.events.is_empty() && trace.events.iter().all(|e| e.kind == "alloc"));
        let unknown = crate::contract::TraceOptions {
            kinds: Some(vec!["nope".into()]),
            ..Default::default()
        };
        let response = traced(TRACE_SOURCE, Some(unknown));
        assert!(response.output.is_empty());
        assert!(matches!(
            response.failure,
            Some(crate::contract::Failure { kind: FailureKind::Request, ref code, .. }) if code == "ZB105"
        ));
        assert!(
            serde_json::from_str::<Request>(
                r#"{"apiVersion":1,"source":"","virtualName":"a.zrk","trace":{"bogus":1}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn deep_clone_of_a_wide_graph_ends_in_a_heap_limit_without_touching_the_source() {
        let source = "class Node { v: Int32; a: Node?; b: Node?; construct(v: Int32) { this.v = v; this.a = null; this.b = null; } }\nfn main(): Void {\n    mut root = Node(0);\n    mut i = 1;\n    while i < 400 { mut n = Node(i); n.a = root.a; n.b = root; root.a = n; i = i + 1; }\n    mut copy = root.clone();\n}\n";
        let run_with_heap = |heap_bytes: usize| {
            run(Request {
                api_version: API_VERSION,
                trace: None,
                source: source.into(),
                virtual_name: "clone-heap.zrk".into(),
                limits: Some(crate::contract::Limits {
                    heap_bytes,
                    ..crate::contract::Limits::DEFAULT
                }),
            })
        };
        let roomy = run_with_heap(16 * 1024 * 1024);
        assert_eq!(
            roomy.completion,
            Some(crate::contract::Completion::Completed),
            "{roomy:?}"
        );
        // Smallest heap in which the graph is built but the clone does not fit.
        let (mut low, mut high) = (1_usize, 16 * 1024 * 1024);
        while low < high {
            let mid = (low + high) / 2;
            if run_with_heap(mid).completion.is_some() {
                high = mid;
            } else {
                low = mid + 1;
            }
        }
        let tight = run_with_heap(low - 1);
        assert!(
            matches!(
                tight.failure,
                Some(crate::contract::Failure { kind: FailureKind::Limit, ref limit, .. }) if limit.as_deref() == Some("heap")
            ),
            "{tight:?}"
        );
        assert!(tight.output.is_empty());
    }

    #[test]
    fn pieces_of_one_line_are_one_event_and_streams_stay_separate() {
        let source = "import { stdout, stderr } from std.io;\nfn main(): Void {\n    stdout.println(\"a\", 2);\n    stdout.print(\"x\");\n    stderr.print(\"E\");\n    stdout.println(\"y\");\n}\n";
        let response = run(Request {
            api_version: API_VERSION,
            trace: None,
            source: source.into(),
            virtual_name: "lines.zrk".into(),
            limits: None,
        });
        let events: Vec<(String, String)> = response
            .output
            .iter()
            .map(|event| (event.destination.clone(), event.text.clone()))
            .collect();
        assert_eq!(
            events,
            [
                ("stdout".to_string(), "a 2\n".to_string()),
                ("stdout".to_string(), "x".to_string()),
                ("stderr".to_string(), "E".to_string()),
                ("stdout".to_string(), "y\n".to_string()),
            ]
        );
    }

    #[test]
    fn host_interpreter_fixture_runner_compares_observable_results() {
        let fixture: crate::fixtures::Fixture =
            serde_json::from_str(include_str!("../fixtures/hello-world.json"))
                .expect("valid fixture");
        assert_eq!(fixture.name, "hello-world");
        assert!(fixture.exclusions.is_empty());

        let (frontend, diagnostics) = check_source(&fixture.source, &fixture.virtual_name);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let frontend = frontend.expect("fixture frontend result");
        let module = zirk_ir::lower(&frontend.program, &frontend.checked);
        zirk_ir::verify(&module).expect("fixture IR");
        let mut interpreter = zirk_interpreter::Interpreter::new(
            &module,
            zirk_interpreter::InterpreterLimits::default(),
        )
        .expect("compatible fixture");
        interpreter.set_bundled_standard_sources(bundled_standard_source_ids(&frontend.sources));
        let mut host = zirk_interpreter::CapturedHost::default();
        let result = interpreter.run_main_with_host(&mut host);
        assert!(matches!(
            result,
            zirk_interpreter::ExecutionResult::Completed { .. }
        ));
        assert_eq!(fixture.completion, "completed");
        assert_eq!(
            host.events()
                .iter()
                .map(|event| &event.text)
                .collect::<Vec<_>>(),
            fixture.expected_output.iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn fixture_contract_runner_reports_the_same_observables_as_the_fixture_metadata() {
        for fixture_json in FIXTURES.iter().copied() {
            let fixture: crate::fixtures::Fixture = serde_json::from_str(fixture_json).unwrap();
            let response = run(Request {
                api_version: API_VERSION,
                trace: None,
                source: fixture.source,
                virtual_name: fixture.virtual_name,
                limits: None,
            });
            assert_eq!(
                response
                    .output
                    .iter()
                    .map(|event| event.text.as_str())
                    .collect::<Vec<_>>(),
                fixture
                    .expected_output
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                "{}: {response:?}",
                fixture.name
            );
            match fixture.completion.as_str() {
                "completed" => assert_eq!(
                    response.completion,
                    Some(crate::contract::Completion::Completed)
                ),
                "runtime" => assert!(matches!(
                    response.failure,
                    Some(crate::contract::Failure {
                        kind: FailureKind::Runtime,
                        ..
                    })
                )),
                other => panic!("unsupported fixture completion {other}"),
            }
        }
    }

    #[test]
    fn differential_fixture_runner_compares_host_interpreter_and_browser_contract() {
        for fixture_json in FIXTURES.iter().copied() {
            let fixture: crate::fixtures::Fixture = serde_json::from_str(fixture_json).unwrap();
            let (frontend, diagnostics) = check_source(&fixture.source, &fixture.virtual_name);
            assert!(!diagnostics.has_errors(), "{}", fixture.name);
            let frontend = frontend.unwrap();
            let module = zirk_ir::lower(&frontend.program, &frontend.checked);
            let mut interpreter = zirk_interpreter::Interpreter::new(
                &module,
                zirk_interpreter::InterpreterLimits::default(),
            )
            .unwrap();
            interpreter
                .set_bundled_standard_sources(bundled_standard_source_ids(&frontend.sources));
            let mut host = zirk_interpreter::CapturedHost::default();
            let host_result = interpreter.run_main_with_host(&mut host);
            let browser = run(Request {
                api_version: API_VERSION,
                trace: None,
                source: fixture.source,
                virtual_name: fixture.virtual_name,
                limits: None,
            });
            assert_eq!(
                host.events()
                    .iter()
                    .map(|event| event.text.as_str())
                    .collect::<Vec<_>>(),
                browser
                    .output
                    .iter()
                    .map(|event| event.text.as_str())
                    .collect::<Vec<_>>(),
                "{}",
                fixture.name
            );
            assert_eq!(
                matches!(
                    host_result,
                    zirk_interpreter::ExecutionResult::Completed { .. }
                ),
                browser.completion == Some(crate::contract::Completion::Completed),
                "{}",
                fixture.name
            );
        }
    }

    #[test]
    fn repeated_browser_runs_do_not_leak_output_or_execution_state() {
        let request = Request {
            api_version: API_VERSION,
            trace: None,
            source:
                "import { stdout } from std.io;\nfn main(): Void { stdout.println(\"fresh\"); }"
                    .into(),
            virtual_name: "isolation.zrk".into(),
            limits: None,
        };
        let first = run(request.clone());
        let second = run(request);
        assert_eq!(first.output, second.output);
        assert_eq!(first.diagnostics, second.diagnostics);
        assert_eq!(first.completion, second.completion);
        assert!(first.failure.is_none());
        assert!(second.failure.is_none());
    }
}
