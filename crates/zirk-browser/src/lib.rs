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
use zirk_ast::{Ident, ModuleDecl};
use zirk_diagnostics::{DiagnosticSink, SourceFile, SourceMap};

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
    let standard_package = bundled_standard_package(&mut sources, &mut sink);
    let standard_modules = bundled_standard_modules(&mut sources, &mut sink);
    // The browser has no filesystem-backed loader, so mirror the CLI's merge:
    // modules carry their published-member metadata and their declarations are
    // also part of the checked executable program.
    program.enums.extend(standard_package.enums);
    program.classes.extend(standard_package.classes);
    program.contracts.extend(standard_package.contracts);
    program.type_aliases.extend(standard_package.type_aliases);
    for module in &standard_modules {
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

fn bundled_standard_package(
    sources: &mut SourceMap,
    sink: &mut DiagnosticSink,
) -> zirk_ast::Program {
    let file = sources.add(SourceFile::new(
        "std.io/package.zrk",
        include_str!("../../../stdlib/std/io/package.zrk"),
    ));
    let tokens = zirk_lexer::tokenize(sources.file(file), sink);
    zirk_parser::parse(sources.file(file), &tokens, sink)
}

/// Parses the compiler-bundled standard modules into the same `SourceMap` as
/// the caller's buffer. This keeps browser resolution source-based while the
/// browser remains unable to read host files or fetch packages.
fn bundled_standard_modules(sources: &mut SourceMap, sink: &mut DiagnosticSink) -> Vec<ModuleDecl> {
    [
        ("stdin", include_str!("../../../stdlib/std/io/stdin.zrk")),
        ("stdout", include_str!("../../../stdlib/std/io/stdout.zrk")),
        ("stderr", include_str!("../../../stdlib/std/io/stderr.zrk")),
    ]
    .into_iter()
    .map(|(name, source)| {
        let file = sources.add(SourceFile::new(format!("std.io/{name}.zrk"), source));
        let tokens = zirk_lexer::tokenize(sources.file(file), sink);
        let body = zirk_parser::parse(sources.file(file), &tokens, sink);
        ModuleDecl {
            name: Ident::new(name, body.span),
            shared: true,
            span: body.span,
            body: Box::new(body),
        }
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
    let mut host = zirk_interpreter::CapturedHost::default();
    let result = interpreter.run_main_with_host(&mut host);
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

fn execution_failure(
    result: zirk_interpreter::ExecutionResult,
    completion: &mut Option<contract::Completion>,
) -> Option<contract::Failure> {
    use contract::{Completion, Failure, FailureKind};
    match result {
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
    use super::{API_VERSION, BrowserApi, check, check_source, format, run};
    use crate::contract::{FailureKind, Limits, Request};

    #[test]
    fn exposes_the_initial_contract_revision() {
        assert_eq!(API_VERSION, 1);
    }

    #[test]
    fn format_is_additive_v1_and_returns_source_or_diagnostics() {
        let valid = format(Request {
            api_version: API_VERSION,
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
                    diagnostic.location.clone(),
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
                    diagnostic.location.clone(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(memory, file);
    }

    #[test]
    fn rejects_oversized_source_before_the_lexer_runs() {
        let response = check(Request {
            api_version: API_VERSION,
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
    fn run_lowers_verifies_and_interprets_a_compatible_program() {
        let response = run(Request {
            api_version: API_VERSION,
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
    fn run_contract_covers_completion_runtime_capability_and_execution_limits() {
        let normal = run(Request {
            api_version: API_VERSION,
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
        for fixture_json in [
            include_str!("../fixtures/hello-world.json"),
            include_str!("../fixtures/direct-call.json"),
            include_str!("../fixtures/fatal-error.json"),
            include_str!("../fixtures/scalar-widths.json"),
            include_str!("../fixtures/temporal-range.json"),
            include_str!("../fixtures/closure.json"),
            include_str!("../fixtures/regex.json"),
            include_str!("../fixtures/class-construction-sugar.json"),
            include_str!("../fixtures/void-call.json"),
            include_str!("../fixtures/void-method-call.json"),
            include_str!("../fixtures/list-add.json"),
            include_str!("../fixtures/map-set.json"),
            include_str!("../fixtures/set-add.json"),
            include_str!("../fixtures/typed-catch.json"),
            include_str!("../fixtures/downcast.json"),
            include_str!("../fixtures/decimal-ops.json"),
            include_str!("../fixtures/decimal-compare.json"),
            include_str!("../fixtures/int-division.json"),
            include_str!("../fixtures/length-loop.json"),
            include_str!("../fixtures/uncaught-throwable-message.json"),
        ] {
            let fixture: crate::fixtures::Fixture = serde_json::from_str(fixture_json).unwrap();
            let response = run(Request {
                api_version: API_VERSION,
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
        for fixture_json in [
            include_str!("../fixtures/hello-world.json"),
            include_str!("../fixtures/direct-call.json"),
            include_str!("../fixtures/fatal-error.json"),
            include_str!("../fixtures/scalar-widths.json"),
            include_str!("../fixtures/temporal-range.json"),
            include_str!("../fixtures/closure.json"),
            include_str!("../fixtures/regex.json"),
            include_str!("../fixtures/class-construction-sugar.json"),
            include_str!("../fixtures/void-call.json"),
            include_str!("../fixtures/void-method-call.json"),
            include_str!("../fixtures/list-add.json"),
            include_str!("../fixtures/map-set.json"),
            include_str!("../fixtures/set-add.json"),
            include_str!("../fixtures/typed-catch.json"),
            include_str!("../fixtures/downcast.json"),
            include_str!("../fixtures/decimal-ops.json"),
            include_str!("../fixtures/decimal-compare.json"),
            include_str!("../fixtures/int-division.json"),
            include_str!("../fixtures/length-loop.json"),
            include_str!("../fixtures/uncaught-throwable-message.json"),
        ] {
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
            let mut host = zirk_interpreter::CapturedHost::default();
            let host_result = interpreter.run_main_with_host(&mut host);
            let browser = run(Request {
                api_version: API_VERSION,
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
