//! The `zirk-lsp` library: a Language Server Protocol endpoint backed by the
//! Zirk frontend.
//!
//! The server analyzes the text the editor reports — never the file on disk —
//! by feeding it to the module loader's overlay map, so hover, symbols and
//! diagnostics describe what the user is typing. Analysis is deliberately
//! whole-crate and serialized: files are small, the frontend is fast, and the
//! incremental machinery belongs to Phase 9.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use tower_lsp::Client;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{LanguageServer, LspService};

use zirk_cli::{format, frontend};
use zirk_diagnostics::{
    Diagnostic as ZirkDiagnostic, DiagnosticSink, FileId, Severity, SourceMap, Span,
};
use zirk_sema::{TypeMapEntry, TypeMapKind};

/// The result of one analysis pass over a document's text.
///
/// `frontend` is absent when the text could not be loaded or parsed: the
/// diagnostics still publish, and every interactive request answers `null`
/// rather than reporting types from an older snapshot.
pub struct Analysis {
    /// The diagnostics the run emitted.
    pub diagnostics: Vec<ZirkDiagnostic>,
    /// The checked program, when the frontend got far enough to produce one.
    pub frontend: Option<frontend::FrontendResult>,
}

/// Analyzes `text` as the crate rooted at `path`: the document itself comes
/// from memory, its imports resolve from disk relative to `path`'s directory.
pub fn analyze(path: &Path, text: &str) -> Analysis {
    let mut sink = DiagnosticSink::new();
    let mut overlay = HashMap::new();
    overlay.insert(path.to_path_buf(), text.to_string());
    let result = frontend::run_frontend_lenient_with_overlay(path, &overlay, &mut sink);
    Analysis {
        diagnostics: sink.diagnostics().to_vec(),
        frontend: result,
    }
}

/// The span-picking rule for hover: the smallest entry whose span contains
/// `offset` wins; on a tie a declaration kind beats `Expression` — the same
/// rule the CLI-bridge hover implements in `extension.js`, mirrored so both
/// surfaces report the same type.
pub fn best_hover_entry(entries: &[TypeMapEntry], offset: u32) -> Option<&TypeMapEntry> {
    let mut best: Option<&TypeMapEntry> = None;
    for entry in entries {
        if entry.span.file != FileId::ENTRY {
            continue;
        }
        if entry.span.start > offset || offset > entry.span.end {
            continue;
        }
        let better = match best {
            None => true,
            Some(current) => {
                entry.span.len() < current.span.len()
                    || (entry.span.len() == current.span.len()
                        && current.kind == TypeMapKind::Expression)
            }
        };
        if better {
            best = Some(entry);
        }
    }
    best
}

/// The hover's display line: `fn name(params): Return`, `name: Type`, or the
/// bare expression type.
pub fn hover_text(entry: &TypeMapEntry) -> String {
    match (entry.kind, entry.name.as_deref()) {
        (TypeMapKind::Function, Some(name)) => {
            format!("fn {name}{}", &entry.text["fn".len()..])
        }
        (TypeMapKind::Method, Some(name)) => {
            format!("{name}{}", &entry.text["fn".len()..])
        }
        (TypeMapKind::Variable | TypeMapKind::Parameter | TypeMapKind::Field, Some(name)) => {
            format!("{name}: {}", entry.text)
        }
        _ => entry.text.clone(),
    }
}

/// A compiler diagnostic translated for LSP: the file it belongs to (by the
/// name its `SourceFile` was registered under) plus the editor-facing
/// diagnostic itself.
fn to_lsp_diagnostic(diag: &ZirkDiagnostic) -> (Option<String>, Diagnostic) {
    let (range, file) = match (&diag.location, &diag.snippet) {
        (Some(location), Some(snippet)) => {
            let line = location.line.saturating_sub(1);
            // `Location::column` counts characters; LSP wants UTF-16 units.
            let start: usize = snippet
                .line_text
                .chars()
                .take(location.column.saturating_sub(1) as usize)
                .map(char::len_utf16)
                .sum();
            let width: usize = snippet
                .line_text
                .chars()
                .skip(location.column.saturating_sub(1) as usize)
                .take(snippet.width as usize)
                .map(char::len_utf16)
                .sum();
            (
                Range::new(
                    Position::new(line, start as u32),
                    Position::new(line, (start + width.max(1)) as u32),
                ),
                Some(location.file.clone()),
            )
        }
        _ => (Range::new(Position::new(0, 0), Position::new(0, 0)), None),
    };

    let mut message = diag.message.clone();
    if let Some(cause) = &diag.cause {
        message.push_str(&format!("\n\ncause: {cause}"));
    }
    if let Some(help) = &diag.help {
        message.push_str(&format!("\n\nhelp: {help}"));
    }

    let severity = match diag.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        _ => DiagnosticSeverity::WARNING,
    };

    let mut lsp = Diagnostic::new_simple(range, message);
    lsp.severity = Some(severity);
    lsp.source = Some("zirk".to_string());
    lsp.code = Some(NumberOrString::String(diag.code.as_str().to_string()));
    (file, lsp)
}

/// One symbol of `textDocument/documentSymbol`, built from the merged AST.
/// Only declarations that live in the entry file become symbols: an imported
/// declaration belongs to its own document's outline.
pub fn document_symbols(program: &zirk_ast::Program, sources: &SourceMap) -> Vec<DocumentSymbol> {
    fn range(sources: &SourceMap, span: Span) -> Range {
        let file = sources.file(span.file);
        let (sl, sc) = file.line_column_utf16(span.start);
        let (el, ec) = file.line_column_utf16(span.end);
        Range::new(Position::new(sl, sc), Position::new(el, ec))
    }

    #[allow(deprecated)]
    fn symbol(
        sources: &SourceMap,
        name: &str,
        detail: Option<String>,
        kind: SymbolKind,
        span: Span,
        selection: Span,
        children: Vec<DocumentSymbol>,
    ) -> DocumentSymbol {
        DocumentSymbol {
            name: name.to_string(),
            detail,
            kind,
            tags: None,
            deprecated: None,
            range: range(sources, span),
            selection_range: range(sources, selection),
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
        }
    }

    fn class_symbols(sources: &SourceMap, class: &zirk_ast::ClassDecl) -> DocumentSymbol {
        let mut children = Vec::new();
        for field in &class.fields {
            children.push(symbol(
                sources,
                &field.name.name,
                None,
                SymbolKind::FIELD,
                field.span,
                field.name.span,
                Vec::new(),
            ));
        }
        for construct in &class.constructors {
            children.push(symbol(
                sources,
                "construct",
                None,
                SymbolKind::CONSTRUCTOR,
                construct.span,
                construct.span,
                Vec::new(),
            ));
        }
        for method in &class.methods {
            children.push(symbol(
                sources,
                &method.name.name,
                None,
                SymbolKind::METHOD,
                method.span,
                method.name.span,
                Vec::new(),
            ));
        }
        for nested in &class.nested {
            children.push(class_symbols(sources, nested));
        }
        let kind = match class.kind {
            zirk_ast::ClassKind::Record => SymbolKind::STRUCT,
            _ => SymbolKind::CLASS,
        };
        symbol(
            sources,
            &class.name.name,
            Some(class.kind.as_str().to_string()),
            kind,
            class.span,
            class.name.span,
            children,
        )
    }

    fn in_entry(sources: &SourceMap, span: Span) -> bool {
        span.file == FileId::ENTRY && span.start as usize <= sources.entry().text().len()
    }

    let mut symbols = Vec::new();

    for function in &program.functions {
        if !in_entry(sources, function.span) {
            continue;
        }
        symbols.push(symbol(
            sources,
            &function.name.name,
            None,
            SymbolKind::FUNCTION,
            function.span,
            function.name.span,
            Vec::new(),
        ));
    }

    for class in &program.classes {
        if !in_entry(sources, class.span) {
            continue;
        }
        symbols.push(class_symbols(sources, class));
    }

    for enumeration in &program.enums {
        if !in_entry(sources, enumeration.span) {
            continue;
        }
        let children = enumeration
            .variants
            .iter()
            .map(|variant| {
                symbol(
                    sources,
                    &variant.name.name,
                    None,
                    SymbolKind::ENUM_MEMBER,
                    variant.span,
                    variant.name.span,
                    Vec::new(),
                )
            })
            .collect();
        symbols.push(symbol(
            sources,
            &enumeration.name.name,
            None,
            SymbolKind::ENUM,
            enumeration.span,
            enumeration.name.span,
            children,
        ));
    }

    for contract in &program.contracts {
        if !in_entry(sources, contract.span) {
            continue;
        }
        let children = contract
            .methods
            .iter()
            .map(|method| {
                symbol(
                    sources,
                    &method.name.name,
                    None,
                    SymbolKind::METHOD,
                    method.span,
                    method.name.span,
                    Vec::new(),
                )
            })
            .collect();
        symbols.push(symbol(
            sources,
            &contract.name.name,
            Some(contract.kind.as_str().to_string()),
            SymbolKind::INTERFACE,
            contract.span,
            contract.name.span,
            children,
        ));
    }

    for alias in &program.type_aliases {
        if !in_entry(sources, alias.span) {
            continue;
        }
        symbols.push(symbol(
            sources,
            &alias.name.name,
            None,
            SymbolKind::TYPE_PARAMETER,
            alias.span,
            alias.name.span,
            Vec::new(),
        ));
    }

    for external in &program.externs {
        if !in_entry(sources, external.span) {
            continue;
        }
        symbols.push(symbol(
            sources,
            &external.name.name,
            None,
            SymbolKind::FUNCTION,
            external.span,
            external.name.span,
            Vec::new(),
        ));
    }

    symbols
}

struct Document {
    /// The text the editor last reported — the only truth the server reads.
    text: String,
    /// The document version `text` belongs to.
    version: i32,
    /// Analysis of `text` at `version`, absent until the first pass lands.
    analysis: Option<Analysis>,
    /// URIs diagnostics were published to, so a clean run can clear them —
    /// an absent publish would leave last run's squiggles behind forever.
    published_uris: Vec<Url>,
}

#[derive(Default)]
struct State {
    documents: HashMap<Url, Document>,
}

/// The LSP endpoint. One global analysis lock serializes runs so a burst of
/// edits never stacks compiler invocations on top of each other.
pub struct Backend {
    client: Client,
    state: Mutex<State>,
    analysis_lock: tokio::sync::Mutex<()>,
}

impl Backend {
    /// Runs the analysis for `uri`'s current text and publishes the outcome.
    ///
    /// Results are applied only when the document's version still matches the
    /// text that was analyzed — a superseded run is dropped, and the run for
    /// the newer text is already scheduled or in flight.
    async fn run_analysis(&self, uri: Url) {
        let (version, path, text) = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(&uri) else {
                return;
            };
            let Ok(path) = uri.to_file_path() else {
                return;
            };
            (document.version, path, document.text.clone())
        };

        // Only one analysis in flight at a time: the frontend is fast enough
        // that queueing beats racing.
        let _permit = self.analysis_lock.lock().await;
        let mut analysis = match tokio::task::spawn_blocking(move || analyze(&path, &text)).await {
            Ok(analysis) => analysis,
            Err(_) => return,
        };
        drop(_permit);

        let outcome = {
            let mut state = self.state.lock().unwrap();
            match state.documents.get_mut(&uri) {
                Some(document) if document.version == version => {
                    let diagnostics = std::mem::take(&mut analysis.diagnostics);
                    document.analysis = Some(analysis);
                    let previously = std::mem::take(&mut document.published_uris);
                    Some((diagnostics, previously))
                }
                _ => None,
            }
        };

        let Some((diagnostics, previously)) = outcome else {
            return;
        };

        // Group by target URI so diagnostics on imported files reach their
        // own documents; a diagnostic with no location lands on the entry.
        let mut by_uri: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
        for diagnostic in &diagnostics {
            let (file, lsp) = to_lsp_diagnostic(diagnostic);
            let target = file
                .and_then(|file| Url::from_file_path(&file).ok())
                .unwrap_or_else(|| uri.clone());
            by_uri.entry(target).or_default().push(lsp);
        }

        let mut published = Vec::new();
        for (target, diags) in by_uri {
            published.push(target.clone());
            self.client.publish_diagnostics(target, diags, None).await;
        }

        // The entry always gets a publish — even an empty one — so fixing the
        // last error clears the squiggles instead of leaving them behind.
        if !published.contains(&uri) {
            published.push(uri.clone());
            self.client
                .publish_diagnostics(uri.clone(), Vec::new(), Some(version))
                .await;
        }
        for target in previously {
            if !published.contains(&target) {
                self.client
                    .publish_diagnostics(target, Vec::new(), None)
                    .await;
            }
        }

        {
            let mut state = self.state.lock().unwrap();
            if let Some(document) = state.documents.get_mut(&uri) {
                document.published_uris = published;
            }
        }
    }

    /// Replaces a document's text and schedules a fresh analysis for it.
    async fn update_document(&self, uri: Url, text: String, version: i32) {
        {
            let mut state = self.state.lock().unwrap();
            let published_uris = state
                .documents
                .get(&uri)
                .map(|document| document.published_uris.clone())
                .unwrap_or_default();
            state.documents.insert(
                uri.clone(),
                Document {
                    text,
                    version,
                    analysis: None,
                    published_uris,
                },
            );
        }
        self.run_analysis(uri).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> RpcResult<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {}

    async fn shutdown(&self) -> RpcResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        if document.language_id != "zirk" {
            return;
        }
        self.update_document(document.uri, document.text, document.version)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let Some(change) = params.content_changes.into_iter().last() else {
            return;
        };
        self.update_document(uri, change.text, params.text_document.version)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut state = self.state.lock().unwrap();
            state.documents.remove(&uri);
        }
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        let state = self.state.lock().unwrap();
        let Some(document) = state.documents.get(uri) else {
            return Ok(None);
        };
        let Some(analysis) = &document.analysis else {
            return Ok(None);
        };
        let Some(result) = &analysis.frontend else {
            return Ok(None);
        };

        let file = result.sources.file(FileId::ENTRY);
        let offset = file.offset_at_utf16(position.line, position.character);
        let Some(entry) = best_hover_entry(&result.checked.type_map, offset) else {
            return Ok(None);
        };

        let mut text = format!("```zirk\n{}\n```", hover_text(entry));
        if entry.inferred {
            text.push_str("\n\n*(inferred)*");
        }
        let (sl, sc) = file.line_column_utf16(entry.span.start);
        let (el, ec) = file.line_column_utf16(entry.span.end);
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: text,
            }),
            range: Some(Range::new(Position::new(sl, sc), Position::new(el, ec))),
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        let state = self.state.lock().unwrap();
        let Some(document) = state.documents.get(uri) else {
            return Ok(None);
        };
        let Some(analysis) = &document.analysis else {
            return Ok(None);
        };
        let Some(result) = &analysis.frontend else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(document_symbols(
            &result.program,
            &result.sources,
        ))))
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> RpcResult<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        let (name, text) = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(uri) else {
                return Ok(None);
            };
            let name = uri
                .to_file_path()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| uri.to_string());
            (name, document.text.clone())
        };

        let Ok(formatted) = format::format_source(&name, &text) else {
            return Ok(None);
        };
        if formatted == text {
            return Ok(None);
        }

        let file = zirk_diagnostics::SourceFile::new(name, text);
        let (end_line, end_col) = file.line_column_utf16(u32::MAX);
        let edit = TextEdit {
            range: Range::new(Position::new(0, 0), Position::new(end_line, end_col)),
            new_text: formatted,
        };
        Ok(Some(vec![edit]))
    }
}

/// Builds the `(service, socket)` pair `main` serves over stdio.
pub fn service() -> (LspService<Backend>, tower_lsp::ClientSocket) {
    LspService::new(|client| Backend {
        client,
        state: Mutex::new(State::default()),
        analysis_lock: tokio::sync::Mutex::new(()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zirk-lsp-test-{}-{}", std::process::id(), tag));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const EXAMPLE: &str = "fn square(n: Int32) {\n    return n * n;\n}\n\nfn main() {\n    stdout.println(square(7));\n}\n";

    #[test]
    fn analysis_reports_diagnostics_for_broken_text() {
        let dir = scratch("broken");
        let path = dir.join("broken.zrk");
        let analysis = analyze(&path, "fn broken(x: Int32) {\n    return x + ;\n}\n");
        assert!(analysis.frontend.is_none());
        assert!(!analysis.diagnostics.is_empty());
    }

    #[test]
    fn analysis_produces_a_type_map_for_valid_text() {
        let dir = scratch("valid");
        let path = dir.join("ok.zrk");
        let analysis = analyze(&path, EXAMPLE);
        assert!(analysis.frontend.is_some());
        assert!(analysis.diagnostics.is_empty());
    }

    #[test]
    fn hover_prefers_the_declaration_over_the_expression() {
        let dir = scratch("hover");
        let path = dir.join("ok.zrk");
        let analysis = analyze(&path, EXAMPLE);
        let result = analysis.frontend.as_ref().unwrap();
        let file = result.sources.file(FileId::ENTRY);

        // `square` in `fn square(...)` — the name's offset.
        let offset = EXAMPLE.find("square").unwrap() as u32;
        let entry = best_hover_entry(&result.checked.type_map, offset).unwrap();
        assert_eq!(hover_text(entry), "fn square(n: Int32): Int32");
        assert!(entry.inferred);
        assert_eq!(entry.span.file, file.id());
    }

    #[test]
    fn hover_outside_typed_spans_is_none() {
        let dir = scratch("hover-none");
        let path = dir.join("ok.zrk");
        let analysis = analyze(&path, EXAMPLE);
        let result = analysis.frontend.as_ref().unwrap();
        // A comment column has no typed span.
        assert!(best_hover_entry(&result.checked.type_map, u32::MAX).is_none());
    }

    #[test]
    fn symbols_cover_the_file_declarations() {
        let dir = scratch("symbols");
        let path = dir.join("ok.zrk");
        let text = "class Counter {\n    value: Int32;\n    bump() { return 1; }\n}\nenum Mode { On, Off }\nfn main() {}\n";
        let analysis = analyze(&path, text);
        let result = analysis.frontend.as_ref().unwrap();
        let symbols = document_symbols(&result.program, &result.sources);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Counter"));
        assert!(names.contains(&"Mode"));
        assert!(names.contains(&"main"));
        let counter = symbols.iter().find(|s| s.name == "Counter").unwrap();
        let child_names: Vec<&str> = counter
            .children
            .as_ref()
            .unwrap()
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(child_names.contains(&"value"));
        assert!(child_names.contains(&"bump"));
    }

    #[test]
    fn analysis_uses_overlay_not_disk() {
        let dir = scratch("overlay");
        let path = dir.join("doc.zrk");
        fs::write(&path, "fn on_disk() { return 0; }\n").unwrap();
        let analysis = analyze(&path, EXAMPLE);
        let result = analysis.frontend.as_ref().unwrap();
        assert!(result.sources.entry().text().contains("square"));
    }
}
