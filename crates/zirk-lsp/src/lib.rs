//! The `zirk-lsp` library: a Language Server Protocol endpoint backed by the
//! Zirk frontend.
//!
//! The server analyzes the text the editor reports — never the file on disk —
//! by feeding it to the module loader's overlay map, so hover, symbols and
//! diagnostics describe what the user is typing. Analysis is deliberately
//! whole-crate and serialized: files are small, the frontend is fast, and the
//! incremental machinery belongs to Phase 9.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};

use tower_lsp::Client;
use tower_lsp::jsonrpc::Result as RpcResult;
use tower_lsp::lsp_types::notification::Progress;
use tower_lsp::lsp_types::request::{
    GotoTypeDefinitionParams, GotoTypeDefinitionResponse, WorkDoneProgressCreate,
};
use tower_lsp::lsp_types::*;
use tower_lsp::{LanguageServer, LspService};

use zirk_cli::frontend;
use zirk_diagnostics::{
    Diagnostic as ZirkDiagnostic, DiagnosticSink, FileId, Severity, SourceMap, Span,
};
use zirk_ide::{IdeSnapshot, Position as IdePosition};
use zirk_sema::{ReferenceRole, SemanticIndex, SemanticSymbolKind, TypeMapEntry, TypeMapKind};

/// The result of one analysis pass over a document's text.
///
/// `frontend` is absent when the entry cannot be loaded or its syntax cannot
/// support an analysis. An unresolved import may retain a partial snapshot so
/// import completion can recover; it never reuses symbols from an older text
/// version.
pub struct Analysis {
    /// The diagnostics the run emitted.
    pub diagnostics: Vec<ZirkDiagnostic>,
    /// The checked program, when the frontend got far enough to produce one.
    pub frontend: Option<frontend::FrontendResult>,
}

impl Analysis {
    fn has_unresolved_imports(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == zirk_cli::codes::UNREADABLE_FILE
                || diagnostic.code == zirk_cli::codes::PACKAGE_RESOLUTION
        })
    }
}

/// Analyzes `text` as the crate rooted at `path`: the document itself comes
/// from memory, its imports resolve from disk relative to `path`'s directory.
pub fn analyze(path: &Path, text: &str) -> Analysis {
    let mut overlay = HashMap::new();
    overlay.insert(path.to_path_buf(), text.to_string());
    analyze_with_overlays(path, &overlay)
}

/// Analyzes one project with unsaved text overlays for any open source files.
pub fn analyze_with_overlays(
    path: &Path,
    overlays: &HashMap<std::path::PathBuf, String>,
) -> Analysis {
    let mut sink = DiagnosticSink::new();
    let result = frontend::run_frontend_ide_with_overlay(path, overlays, &mut sink);
    Analysis {
        diagnostics: sink.diagnostics().to_vec(),
        frontend: result,
    }
}

fn workspace_root(path: &Path) -> std::path::PathBuf {
    if path.is_dir() {
        return path.to_path_buf();
    }
    if path.file_name().is_some_and(|name| name == ".zkinit") {
        return path.parent().unwrap_or(path).to_path_buf();
    }
    path.ancestors()
        .find(|candidate| candidate.join(".zkinit").is_file())
        .map(Path::to_path_buf)
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

/// Builds the one full-document edit exposed by `textDocument/formatting`.
/// Invalid text deliberately produces no edit, matching the formatter's
/// no-partial-output contract.
fn formatting_edit(name: &str, text: &str) -> Option<TextEdit> {
    let formatted = zirk_format::format(name, text).ok()?;
    if formatted == text {
        return None;
    }
    let file = zirk_diagnostics::SourceFile::new(name.to_owned(), text.to_owned());
    let (end_line, end_col) = file.line_column_utf16(u32::MAX);
    Some(TextEdit {
        range: Range::new(Position::new(0, 0), Position::new(end_line, end_col)),
        new_text: formatted,
    })
}

#[derive(Clone)]
struct ReferenceExpectation {
    source_path: String,
    source_start: u32,
    target_path: String,
    target_start: u32,
    role: ReferenceRole,
    rename_target: Option<(String, u32)>,
}

fn reference_expectations(
    sources: &SourceMap,
    semantics: &SemanticIndex,
) -> Vec<ReferenceExpectation> {
    semantics
        .references()
        .iter()
        .filter_map(|reference| {
            let source = sources.file(reference.span.file);
            let target = semantics.symbol(reference.target)?;
            let target_source = sources.file(target.selection_span.file);
            let rename_target = reference.rename_target.and_then(|id| {
                let symbol = semantics.symbol(id)?;
                Some((
                    sources.file(symbol.selection_span.file).name().to_owned(),
                    symbol.selection_span.start,
                ))
            });
            Some(ReferenceExpectation {
                source_path: source.name().to_owned(),
                source_start: reference.span.start,
                target_path: target_source.name().to_owned(),
                target_start: target.selection_span.start,
                role: reference.role,
                rename_target,
            })
        })
        .collect()
}

fn shifted_offset(path: &str, offset: u32, edits: &[zirk_ide::RenameEdit]) -> u32 {
    let shift: i64 = edits
        .iter()
        .filter(|edit| edit.location.path == path && edit.location.span.end <= offset)
        .map(|edit| edit.new_text.len() as i64 - edit.location.span.len() as i64)
        .sum();
    (offset as i64 + shift).max(0) as u32
}

fn apply_rename_edits(
    mut overlays: HashMap<std::path::PathBuf, String>,
    source_texts: &HashMap<std::path::PathBuf, String>,
    edits: &[zirk_ide::RenameEdit],
) -> Option<HashMap<std::path::PathBuf, String>> {
    let mut by_path: HashMap<std::path::PathBuf, Vec<&zirk_ide::RenameEdit>> = HashMap::new();
    for edit in edits {
        by_path
            .entry(std::path::PathBuf::from(&edit.location.path))
            .or_default()
            .push(edit);
    }
    for (path, mut file_edits) in by_path {
        let text = overlays
            .remove(&path)
            .or_else(|| source_texts.get(&path).cloned())?;
        let mut result = text;
        file_edits.sort_by_key(|edit| std::cmp::Reverse(edit.location.span.start));
        let mut previous_start = usize::MAX;
        for edit in file_edits {
            let start = edit.location.span.start as usize;
            let end = edit.location.span.end as usize;
            if end > previous_start || end > result.len() || start > end {
                return None;
            }
            result.replace_range(start..end, &edit.new_text);
            previous_start = start;
        }
        overlays.insert(path, result);
    }
    Some(overlays)
}

fn rename_preserves_bindings(
    path: &Path,
    overlays: &HashMap<std::path::PathBuf, String>,
    edits: &[zirk_ide::RenameEdit],
    expectations: &[ReferenceExpectation],
) -> bool {
    let analysis = analyze_with_overlays(path, overlays);
    let Some(frontend) = analysis.frontend else {
        return false;
    };
    expectations.iter().all(|expected| {
        let Some((source_id, _)) = frontend
            .sources
            .files()
            .find(|(_, source)| source.name() == expected.source_path)
        else {
            return false;
        };
        let source_offset = shifted_offset(&expected.source_path, expected.source_start, edits);
        let Some(reference) = frontend
            .checked
            .semantic_index
            .reference_at_position(source_id, source_offset)
        else {
            return false;
        };
        if reference.role != expected.role {
            return false;
        }
        let Some(target) = frontend.checked.semantic_index.symbol(reference.target) else {
            return false;
        };
        let target_path = frontend.sources.file(target.selection_span.file).name();
        if target_path != expected.target_path
            || target.selection_span.start
                != shifted_offset(&expected.target_path, expected.target_start, edits)
        {
            return false;
        }
        match (&expected.rename_target, reference.rename_target) {
            (None, None) => true,
            (Some((path, start)), Some(id)) => frontend
                .checked
                .semantic_index
                .symbol(id)
                .is_some_and(|symbol| {
                    frontend.sources.file(symbol.selection_span.file).name() == path
                        && symbol.selection_span.start == shifted_offset(path, *start, edits)
                }),
            _ => false,
        }
    })
}

fn workspace_edit(
    edits: Vec<zirk_ide::RenameEdit>,
    open_versions: &HashMap<Url, i32>,
    supports_document_changes: bool,
) -> Option<WorkspaceEdit> {
    let mut grouped: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    for edit in edits {
        grouped
            .entry(Url::from_file_path(edit.location.path).ok()?)
            .or_default()
            .push(TextEdit {
                range: lsp_range(edit.location.range),
                new_text: edit.new_text,
            });
    }
    let mut documents = grouped.into_iter().collect::<Vec<_>>();
    documents.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
    if supports_document_changes {
        Some(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Edits(
                documents
                    .into_iter()
                    .map(|(uri, edits)| TextDocumentEdit {
                        text_document: OptionalVersionedTextDocumentIdentifier {
                            version: open_versions.get(&uri).copied(),
                            uri,
                        },
                        edits: edits.into_iter().map(OneOf::Left).collect(),
                    })
                    .collect(),
            )),
            ..WorkspaceEdit::default()
        })
    } else {
        Some(WorkspaceEdit {
            changes: Some(documents.into_iter().collect()),
            ..WorkspaceEdit::default()
        })
    }
}

fn lsp_symbol_kind(kind: SemanticSymbolKind) -> SymbolKind {
    match kind {
        SemanticSymbolKind::Function => SymbolKind::FUNCTION,
        SemanticSymbolKind::Method => SymbolKind::METHOD,
        SemanticSymbolKind::Variable => SymbolKind::VARIABLE,
        SemanticSymbolKind::Parameter => SymbolKind::VARIABLE,
        SemanticSymbolKind::Field => SymbolKind::FIELD,
        SemanticSymbolKind::Type | SemanticSymbolKind::TypeAlias => SymbolKind::TYPE_PARAMETER,
        SemanticSymbolKind::Class => SymbolKind::CLASS,
        SemanticSymbolKind::Contract => SymbolKind::INTERFACE,
        SemanticSymbolKind::Enum => SymbolKind::ENUM,
        SemanticSymbolKind::EnumVariant => SymbolKind::ENUM_MEMBER,
        SemanticSymbolKind::Module | SemanticSymbolKind::Import => SymbolKind::MODULE,
        SemanticSymbolKind::TypeParameter => SymbolKind::TYPE_PARAMETER,
    }
}

fn lsp_completion_kind(kind: SemanticSymbolKind) -> CompletionItemKind {
    match kind {
        SemanticSymbolKind::Function => CompletionItemKind::FUNCTION,
        SemanticSymbolKind::Method => CompletionItemKind::METHOD,
        SemanticSymbolKind::Variable | SemanticSymbolKind::Parameter => {
            CompletionItemKind::VARIABLE
        }
        SemanticSymbolKind::Field => CompletionItemKind::FIELD,
        SemanticSymbolKind::Type | SemanticSymbolKind::TypeAlias => {
            CompletionItemKind::TYPE_PARAMETER
        }
        SemanticSymbolKind::Class => CompletionItemKind::CLASS,
        SemanticSymbolKind::Contract => CompletionItemKind::INTERFACE,
        SemanticSymbolKind::Enum => CompletionItemKind::ENUM,
        SemanticSymbolKind::EnumVariant => CompletionItemKind::ENUM_MEMBER,
        SemanticSymbolKind::TypeParameter => CompletionItemKind::TYPE_PARAMETER,
        SemanticSymbolKind::Import | SemanticSymbolKind::Module => CompletionItemKind::MODULE,
    }
}

fn completion_items(
    semantics: &SemanticIndex,
    file: FileId,
    position: IdePosition,
    sources: &SourceMap,
    origin_uri: &Url,
    document_version: i32,
    module_index: Option<&zirk_ide::ModuleIndex>,
) -> Vec<CompletionItem> {
    let snapshot = IdeSnapshot::new(0, sources, semantics);
    let snapshot = match module_index {
        Some(index) => snapshot.with_module_index(index),
        None => snapshot,
    };
    let candidates = snapshot.completions(file, position);
    candidates
        .into_iter()
        .map(|candidate| {
            let data = candidate.declaration.as_ref().and_then(|declaration| {
                Some(serde_json::json!({
                    "originUri": origin_uri,
                    "originVersion": document_version,
                    "declarationUri": Url::from_file_path(&declaration.path).ok()?,
                    "start": declaration.span.start,
                    "end": declaration.span.end,
                }))
            });
            CompletionItem {
                label: candidate.label,
                kind: Some(lsp_completion_kind(candidate.kind)),
                detail: candidate.detail,
                data,
                ..CompletionItem::default()
            }
        })
        .collect()
}

fn versioned_type_quickfix(
    uri: &Url,
    version: i32,
    source: &zirk_diagnostics::SourceFile,
    diagnostic: &Diagnostic,
) -> Option<CodeAction> {
    if diagnostic.source.as_deref() != Some("zirk") {
        return None;
    }
    let Some(NumberOrString::String(code)) = diagnostic.code.as_ref() else {
        return None;
    };
    let fix = zirk_ide::diagnostic_quick_fix(
        source,
        zirk_ide::TextRange {
            start: IdePosition {
                line: diagnostic.range.start.line,
                character: diagnostic.range.start.character,
            },
            end: IdePosition {
                line: diagnostic.range.end.line,
                character: diagnostic.range.end.character,
            },
        },
        code,
    )?;
    let edit = TextEdit {
        range: lsp_range(fix.range),
        new_text: fix.new_text,
    };
    Some(CodeAction {
        title: fix.title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: Some(version),
                },
                edits: vec![OneOf::Left(edit)],
            }])),
            ..WorkspaceEdit::default()
        }),
        is_preferred: Some(true),
        ..CodeAction::default()
    })
}

fn ranges_intersect(left: Range, right: Range) -> bool {
    let before = |a: Position, b: Position| (a.line, a.character) < (b.line, b.character);
    !before(left.end, right.start) && !before(right.end, left.start)
}

fn generic_substitutions_at(
    frontend: &frontend::FrontendResult,
    file: FileId,
    offset: u32,
    callee: &str,
) -> std::collections::HashMap<String, String> {
    let Some((_, call)) = frontend
        .checked
        .generic_call_substitutions
        .iter()
        .filter(|(span, info)| {
            span.file == file
                && span.start <= offset
                && offset <= span.end
                && info.callable == callee
        })
        .min_by_key(|(span, _)| span.len())
    else {
        return std::collections::HashMap::new();
    };

    call.substitutions
        .iter()
        .filter_map(|(id, ty)| {
            let parameter = frontend.checked.type_params.get(*id as usize)?;
            let concrete = frontend.checked.describe_type(*ty);
            (!concrete.contains("<unknown>")).then(|| (parameter.name.clone(), concrete))
        })
        .collect()
}

#[allow(deprecated)]
fn workspace_symbols(
    sources: &SourceMap,
    semantics: &SemanticIndex,
    query: &str,
) -> Vec<SymbolInformation> {
    let query = query.to_lowercase();
    let mut symbols = semantics
        .symbols()
        .iter()
        .filter(|symbol| {
            symbol.owner_span.is_none()
                && !matches!(
                    symbol.kind,
                    SemanticSymbolKind::Variable
                        | SemanticSymbolKind::Parameter
                        | SemanticSymbolKind::Import
                        | SemanticSymbolKind::TypeParameter
                )
                && (query.is_empty() || symbol.name.to_lowercase().contains(&query))
        })
        .filter_map(|symbol| {
            let source = sources.file(symbol.selection_span.file);
            Some(SymbolInformation {
                name: symbol.name.clone(),
                kind: lsp_symbol_kind(symbol.kind),
                tags: None,
                deprecated: None,
                location: Location {
                    uri: Url::from_file_path(source.name()).ok()?,
                    range: source_range(source, symbol.selection_span),
                },
                container_name: None,
            })
        })
        .collect::<Vec<_>>();
    symbols.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.location.uri.as_str().cmp(right.location.uri.as_str()))
    });
    symbols.dedup_by(|left, right| {
        left.name == right.name
            && left.location.uri == right.location.uri
            && left.location.range == right.location.range
    });
    symbols.truncate(500);
    symbols
}

fn source_range(source: &zirk_diagnostics::SourceFile, span: Span) -> Range {
    let (start_line, start_character) = source.line_column_utf16(span.start);
    let (end_line, end_character) = source.line_column_utf16(span.end);
    Range::new(
        Position::new(start_line, start_character),
        Position::new(end_line, end_character),
    )
}

fn inferred_type_hints(
    entries: &[TypeMapEntry],
    sources: &SourceMap,
    file: FileId,
    range: Range,
) -> Vec<InlayHint> {
    entries
        .iter()
        .filter(|entry| {
            entry.kind == TypeMapKind::Variable
                && entry.inferred
                && entry.span.file == file
                && !entry.text.contains("Unknown")
                && !entry.text.contains("unknown")
        })
        .filter_map(|entry| {
            let source = sources.file(file);
            let (line, character) = source.line_column_utf16(entry.span.end);
            let position = Position::new(line, character);
            if position < range.start || position > range.end {
                return None;
            }
            Some(InlayHint {
                position,
                label: format!(": {}", entry.text).into(),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: Some(false),
                padding_right: Some(false),
                data: None,
            })
        })
        .collect()
}

fn file_id_for_uri(sources: &SourceMap, uri: &Url) -> Option<FileId> {
    sources.files().find_map(|(id, file)| {
        Url::from_file_path(file.name())
            .ok()
            .filter(|candidate| candidate == uri)
            .map(|_| id)
    })
}

fn lsp_range(range: zirk_ide::TextRange) -> Range {
    Range::new(
        Position::new(range.start.line, range.start.character),
        Position::new(range.end.line, range.end.character),
    )
}

/// The span-picking rule for hover: the smallest entry whose span contains
/// `offset` wins; on a tie a declaration kind beats `Expression` — the same
/// rule the CLI-bridge hover implements in `extension.js`, mirrored so both
/// surfaces report the same type.
pub fn best_hover_entry(entries: &[TypeMapEntry], offset: u32) -> Option<&TypeMapEntry> {
    best_hover_entry_in_file(entries, FileId::ENTRY, offset)
}

pub fn best_hover_entry_in_file(
    entries: &[TypeMapEntry],
    file: FileId,
    offset: u32,
) -> Option<&TypeMapEntry> {
    let mut best: Option<&TypeMapEntry> = None;
    for entry in entries {
        if entry.span.file != file {
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

/// Import declarations are represented by semantic references rather than
/// ordinary type-map entries. Build an import-specific hover from the same
/// compiler-resolved target so aliases and module paths explain what they
/// refer to without guessing from the spelling.
fn import_hover_content(
    program: &zirk_ast::Program,
    semantics: &SemanticIndex,
    file: FileId,
    offset: u32,
) -> Option<(String, Span)> {
    fn contains(span: Span, file: FileId, offset: u32) -> bool {
        span.file == file && span.start <= offset && offset < span.end
    }

    fn declaration_text(symbol: &zirk_sema::SemanticSymbol) -> String {
        use SemanticSymbolKind as Kind;

        let typed_name = |label: &str| match symbol.type_text.as_deref() {
            Some(ty) => format!("{label} {}: {ty}", symbol.name),
            None => format!("{label} {}", symbol.name),
        };
        match symbol.kind {
            Kind::Function | Kind::Method => match symbol.type_text.as_deref() {
                Some(signature) if signature.starts_with("fn") => {
                    format!("fn {}{}", symbol.name, &signature[2..])
                }
                _ => format!("fn {}", symbol.name),
            },
            Kind::Variable => typed_name("variable"),
            Kind::Parameter => typed_name("parameter"),
            Kind::Field => typed_name("field"),
            Kind::Type => format!("type {}", symbol.name),
            Kind::Class => format!("class {}", symbol.name),
            Kind::Contract => format!("contract {}", symbol.name),
            Kind::Enum => format!("enum {}", symbol.name),
            Kind::EnumVariant => format!("variant {}", symbol.name),
            Kind::TypeAlias => match symbol.type_text.as_deref() {
                Some(target) => format!("type {} = {target}", symbol.name),
                None => format!("type {}", symbol.name),
            },
            Kind::TypeParameter => format!("type parameter {}", symbol.name),
            Kind::Module => format!("module {}", symbol.name),
            Kind::Import => format!("import {}", symbol.name),
        }
    }

    fn visit(
        program: &zirk_ast::Program,
        semantics: &SemanticIndex,
        file: FileId,
        offset: u32,
    ) -> Option<(String, Span)> {
        for import in &program.imports {
            let source = &import.source;
            let source_span = source.span();
            if contains(source_span, file, offset) {
                let (module, description) = match source {
                    zirk_ast::ImportSource::Package { path, .. } => {
                        let description = if path.starts_with("std.") {
                            "standard library module"
                        } else {
                            "package module"
                        };
                        (format!("module {path}"), description)
                    }
                    zirk_ast::ImportSource::Local { path, .. } => {
                        (format!("module \"{path}\""), "local module")
                    }
                };
                return Some((
                    format!("```zirk\n{module}\n```\n\n{description}"),
                    source_span,
                ));
            }

            for item in &import.names {
                let alias_hit = item
                    .alias
                    .as_ref()
                    .is_some_and(|alias| contains(alias.span, file, offset));
                let source_name_hit = contains(item.name.span, file, offset);
                if !alias_hit && !source_name_hit {
                    continue;
                }

                let Some(target) = semantics
                    .reference_at_position(file, item.name.span.start)
                    .and_then(|reference| semantics.symbol(reference.target))
                else {
                    continue;
                };
                let declaration = declaration_text(target);
                let binding = item.alias.as_ref().map(|alias| alias.name.as_str());
                let relation = match binding {
                    Some(alias) => format!("as `{alias}`"),
                    None => format!("as `{}`", item.name.name),
                };
                let origin = source.path();
                let details = if target.is_standard_library {
                    format!("Standard library export, imported {relation} from `{origin}`.")
                } else {
                    format!("Imported {relation} from `{origin}`.")
                };
                return Some((
                    format!("```zirk\n{declaration}\n```\n\n{details}"),
                    if alias_hit {
                        item.alias.as_ref().expect("alias hit implies alias").span
                    } else {
                        item.name.span
                    },
                ));
            }
        }

        for module in &program.modules {
            if let Some(found) = visit(&module.body, semantics, file, offset) {
                return Some(found);
            }
        }
        None
    }

    visit(program, semantics, file, offset)
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
    document_symbols_in_file(program, sources, FileId::ENTRY)
}

pub fn document_symbols_in_file(
    program: &zirk_ast::Program,
    sources: &SourceMap,
    file_id: FileId,
) -> Vec<DocumentSymbol> {
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
        for constructor in &class.constructors {
            for param in constructor
                .params
                .iter()
                .filter(|param| param.promoted.is_some())
            {
                children.push(symbol(
                    sources,
                    &param.name.name,
                    None,
                    SymbolKind::FIELD,
                    param.span,
                    param.name.span,
                    Vec::new(),
                ));
            }
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
        symbol(
            sources,
            &class.name.name,
            Some(class.kind.as_str().to_string()),
            SymbolKind::CLASS,
            class.span,
            class.name.span,
            children,
        )
    }

    fn in_file(sources: &SourceMap, file_id: FileId, span: Span) -> bool {
        span.file == file_id && span.start as usize <= sources.file(file_id).text().len()
    }

    let mut symbols = Vec::new();

    for function in &program.functions {
        if !in_file(sources, file_id, function.span) {
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
        if !in_file(sources, file_id, class.span) {
            continue;
        }
        symbols.push(class_symbols(sources, class));
    }

    for enumeration in &program.enums {
        if !in_file(sources, file_id, enumeration.span) {
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
        if !in_file(sources, file_id, contract.span) {
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
        if !in_file(sources, file_id, alias.span) {
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
        if !in_file(sources, file_id, external.span) {
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

const SEMANTIC_TOKEN_TYPES: [&str; 14] = [
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "function",
    "method",
    "local",
];

const SEMANTIC_TOKEN_MODIFIERS: [&str; 4] =
    ["declaration", "defaultLibrary", "reference", "import"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemanticTokenEntry {
    line: u32,
    start: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

fn semantic_token_types(
    kind: zirk_sema::SemanticSymbolKind,
    visibility: zirk_sema::SemanticVisibility,
) -> &'static [&'static str] {
    use zirk_sema::SemanticSymbolKind as Kind;
    match kind {
        Kind::Module => &["namespace", "type"],
        Kind::Type | Kind::TypeAlias => &["type"],
        Kind::Class => &["class", "type"],
        Kind::Enum => &["enum", "type"],
        Kind::Contract => &["interface", "type"],
        Kind::TypeParameter => &["typeParameter", "type"],
        Kind::Parameter => &["parameter", "variable"],
        Kind::Variable if visibility == zirk_sema::SemanticVisibility::Local => {
            &["local", "variable"]
        }
        Kind::Variable => &["variable"],
        Kind::Import => &["variable"],
        Kind::Field => &["property", "variable"],
        Kind::EnumVariant => &["enumMember", "variable"],
        Kind::Function => &["function"],
        Kind::Method => &["method", "function"],
    }
}

fn imported_symbol_kind(
    index: &zirk_sema::SemanticIndex,
    symbol: &zirk_sema::SemanticSymbol,
) -> zirk_sema::SemanticSymbolKind {
    if symbol.kind != zirk_sema::SemanticSymbolKind::Import {
        return symbol.kind;
    }
    index
        .references()
        .iter()
        .find(|reference| reference.rename_target == Some(symbol.id))
        .and_then(|reference| index.symbol(reference.target))
        .map_or(symbol.kind, |target| target.kind)
}

fn semantic_token_entries(
    index: &zirk_sema::SemanticIndex,
    program: &zirk_ast::Program,
    sources: &SourceMap,
    file_id: FileId,
    legend: &SemanticTokensLegend,
) -> Vec<SemanticTokenEntry> {
    fn is_import_binding_name(program: &zirk_ast::Program, span: Span) -> bool {
        program.imports.iter().any(|import| {
            import
                .names
                .iter()
                .any(|item| item.bound_name().span == span)
        }) || program
            .modules
            .iter()
            .any(|module| is_import_binding_name(&module.body, span))
    }

    fn is_aliased_import_name(program: &zirk_ast::Program, span: Span) -> bool {
        program.imports.iter().any(|import| {
            import
                .names
                .iter()
                .any(|item| item.alias.is_some() && item.name.span == span)
        }) || program
            .modules
            .iter()
            .any(|module| is_aliased_import_name(&module.body, span))
    }

    let mut tokens = HashMap::new();
    let mut add = |span: Span,
                   kind: zirk_sema::SemanticSymbolKind,
                   visibility: zirk_sema::SemanticVisibility,
                   modifiers: u32| {
        if span.file != file_id || span.is_empty() {
            return;
        }
        let source = sources.file(file_id);
        let (line, start) = source.line_column_utf16(span.start);
        let (end_line, end) = source.line_column_utf16(span.end);
        if line != end_line || end <= start {
            return;
        }
        let Some(token_type) =
            semantic_token_types(kind, visibility)
                .iter()
                .find_map(|candidate| {
                    legend
                        .token_types
                        .iter()
                        .position(|token_type| token_type.as_str() == *candidate)
                })
        else {
            return;
        };
        tokens.entry((line, start)).or_insert(SemanticTokenEntry {
            line,
            start,
            length: end - start,
            token_type: token_type as u32,
            modifiers,
        });
    };

    for symbol in index.symbols() {
        // Import declarations already have precise TextMate scopes for their
        // exported/local spellings. Semantic import symbols can be reported as
        // generic `variable` tokens while the index is incomplete, which
        // overrides those scopes with the wrong palette color.
        if is_import_binding_name(program, symbol.selection_span) {
            continue;
        }
        let declaration_modifier = legend
            .token_modifiers
            .iter()
            .position(|modifier| modifier.as_str() == "declaration")
            .map_or(0, |index| 1 << index);
        let library_modifier = if symbol.is_standard_library {
            legend
                .token_modifiers
                .iter()
                .position(|modifier| modifier.as_str() == "defaultLibrary")
                .map_or(0, |index| 1 << index)
        } else {
            0
        };
        let modifiers = declaration_modifier | library_modifier;
        let mut modifiers = modifiers;
        if symbol.kind == zirk_sema::SemanticSymbolKind::Import {
            modifiers |= legend
                .token_modifiers
                .iter()
                .position(|modifier| modifier.as_str() == "import")
                .map_or(0, |index| 1 << index);
        }
        add(
            symbol.selection_span,
            imported_symbol_kind(index, symbol),
            symbol.visibility,
            modifiers,
        );
    }
    for reference in index.references() {
        if reference.role == zirk_sema::ReferenceRole::Import
            && is_import_binding_name(program, reference.span)
        {
            continue;
        }
        // The exported spelling in `source -> LocalName` is intentionally
        // muted by the TextMate grammar. Let that scope provide its gray color
        // instead of covering it with the exported symbol's semantic color;
        // the local alias still receives its semantic token as a declaration.
        if reference.role == zirk_sema::ReferenceRole::Import
            && is_aliased_import_name(program, reference.span)
        {
            continue;
        }
        if let Some(symbol) = index.symbol(reference.target) {
            let mut modifiers = legend
                .token_modifiers
                .iter()
                .position(|modifier| modifier.as_str() == "reference")
                .map_or(0, |index| 1 << index);
            if reference.role == zirk_sema::ReferenceRole::Import
                || reference.rename_target.is_some()
            {
                modifiers |= legend
                    .token_modifiers
                    .iter()
                    .position(|modifier| modifier.as_str() == "import")
                    .map_or(0, |index| 1 << index);
            }
            if symbol.is_standard_library {
                modifiers |= legend
                    .token_modifiers
                    .iter()
                    .position(|modifier| modifier.as_str() == "defaultLibrary")
                    .map_or(0, |index| 1 << index)
            }
            add(reference.span, symbol.kind, symbol.visibility, modifiers);
        }
    }

    let mut tokens = tokens.into_values().collect::<Vec<_>>();
    tokens.sort_unstable();
    tokens
}

fn encode_semantic_tokens(
    mut entries: Vec<SemanticTokenEntry>,
    range: Option<Range>,
    result_id: String,
) -> SemanticTokens {
    if let Some(range) = range {
        entries.retain(|token| {
            let after_start = token.line > range.start.line
                || (token.line == range.start.line && token.start >= range.start.character);
            let before_end = token.line < range.end.line
                || (token.line == range.end.line
                    && token.start < range.end.character
                    && token.start + token.length <= range.end.character);
            after_start && before_end
        });
    }
    let mut data = Vec::with_capacity(entries.len());
    let mut previous_line = 0;
    let mut previous_start = 0;
    for token in entries {
        let delta_line = token.line - previous_line;
        let delta_start = if delta_line == 0 {
            token.start - previous_start
        } else {
            token.start
        };
        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.length,
            token_type: token.token_type,
            token_modifiers_bitset: token.modifiers,
        });
        previous_line = token.line;
        previous_start = token.start;
    }
    SemanticTokens {
        result_id: Some(result_id),
        data,
    }
}

fn semantic_tokens_for(
    analysis: &Analysis,
    uri: &Url,
    range: Option<Range>,
    legend: &SemanticTokensLegend,
) -> Option<SemanticTokens> {
    let frontend = analysis.frontend.as_ref()?;
    let file_id = file_id_for_uri(&frontend.sources, uri)?;
    let index = &frontend.checked.semantic_index;
    Some(encode_semantic_tokens(
        semantic_token_entries(index, &frontend.program, &frontend.sources, file_id, legend),
        range,
        index.snapshot_id().to_string(),
    ))
}

fn negotiate_semantic_tokens(
    client: &ClientCapabilities,
) -> Option<(SemanticTokensLegend, SemanticTokensOptions)> {
    let client = client.text_document.as_ref()?.semantic_tokens.as_ref()?;
    let (full, delta) = match client.requests.full.as_ref() {
        Some(SemanticTokensFullOptions::Bool(true)) => {
            (Some(SemanticTokensFullOptions::Bool(true)), false)
        }
        Some(SemanticTokensFullOptions::Delta { delta }) => (
            Some(if *delta == Some(true) {
                SemanticTokensFullOptions::Delta { delta: Some(true) }
            } else {
                SemanticTokensFullOptions::Bool(true)
            }),
            *delta == Some(true),
        ),
        _ => (None, false),
    };
    let range = client.requests.range == Some(true);
    if full.is_none() && !range {
        return None;
    }

    let token_types = SEMANTIC_TOKEN_TYPES
        .iter()
        .filter(|token_type| {
            client
                .token_types
                .iter()
                .any(|client_type| client_type.as_str() == **token_type)
        })
        .map(|token_type| SemanticTokenType::new(token_type))
        .collect::<Vec<_>>();
    if token_types.is_empty() {
        return None;
    }
    let token_modifiers = SEMANTIC_TOKEN_MODIFIERS
        .iter()
        .filter(|modifier| {
            client
                .token_modifiers
                .iter()
                .any(|client_modifier| client_modifier.as_str() == **modifier)
        })
        .map(|modifier| SemanticTokenModifier::new(modifier))
        .collect();
    let legend = SemanticTokensLegend {
        token_types,
        token_modifiers,
    };
    let options = SemanticTokensOptions {
        legend: legend.clone(),
        range: range.then_some(true),
        full: full.map(|full| {
            if delta {
                SemanticTokensFullOptions::Delta { delta: Some(true) }
            } else {
                full
            }
        }),
        ..SemanticTokensOptions::default()
    };
    Some((legend, options))
}

fn remember_semantic_tokens(history: &mut VecDeque<SemanticTokens>, tokens: SemanticTokens) {
    if let Some(result_id) = &tokens.result_id {
        history.retain(|previous| previous.result_id.as_ref() != Some(result_id));
    }
    history.push_back(tokens);
    while history.len() > 4 {
        history.pop_front();
    }
}

fn semantic_tokens_delta(
    previous: &SemanticTokens,
    current: &SemanticTokens,
) -> SemanticTokensDelta {
    let mut prefix = 0;
    while prefix < previous.data.len()
        && prefix < current.data.len()
        && previous.data[prefix] == current.data[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < previous.data.len().saturating_sub(prefix)
        && suffix < current.data.len().saturating_sub(prefix)
        && previous.data[previous.data.len() - suffix - 1]
            == current.data[current.data.len() - suffix - 1]
    {
        suffix += 1;
    }
    let changed = prefix != previous.data.len() || prefix != current.data.len();
    let edits = if changed {
        vec![SemanticTokensEdit {
            // The LSP edit offsets count the flattened five-integer token
            // records, while this crate stores one struct per record.
            start: (prefix * 5) as u32,
            delete_count: ((previous.data.len() - prefix - suffix) * 5) as u32,
            data: Some(current.data[prefix..current.data.len() - suffix].to_vec()),
        }]
    } else {
        Vec::new()
    };
    SemanticTokensDelta {
        result_id: current.result_id.clone(),
        edits,
    }
}

struct Document {
    /// The text the editor last reported — the only truth the server reads.
    text: String,
    /// The document version `text` belongs to.
    version: i32,
    /// Analysis of `text` at `version`, absent until the first pass lands.
    analysis: Option<Arc<Analysis>>,
    /// A standalone analysis used only when the open file is outside the
    /// workspace entry's loaded import graph. Project analysis remains the
    /// source of truth for workspace queries; this lets an explicitly opened
    /// orphan file still receive compiler-backed semantic colors.
    semantic_fallback: Option<Arc<Analysis>>,
    /// URIs diagnostics were published to, so a clean run can clear them —
    /// an absent publish would leave last run's squiggles behind forever.
    published_uris: Vec<Url>,
    semantic_token_history: VecDeque<SemanticTokens>,
}

#[derive(Default)]
struct State {
    documents: HashMap<Url, Document>,
    module_indexes: HashMap<std::path::PathBuf, Arc<zirk_ide::ModuleIndex>>,
    module_index_generation: u64,
    workspace_generations: HashMap<std::path::PathBuf, u64>,
    semantic_token_legend: Option<SemanticTokensLegend>,
    supports_document_changes: bool,
    supports_work_done_progress: bool,
    next_progress_id: u64,
}

fn analysis_snapshot_is_current(
    state: &State,
    uri: &Url,
    root: &Path,
    version: i32,
    overlay_versions: &HashMap<Url, i32>,
    generation: u64,
) -> bool {
    let snapshot_uris = overlay_versions
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let current_uris = state
        .documents
        .keys()
        .filter(|open_uri| {
            open_uri
                .to_file_path()
                .ok()
                .is_some_and(|path| workspace_root(&path) == root)
        })
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let overlay_versions_current = overlay_versions.iter().all(|(open_uri, open_version)| {
        state
            .documents
            .get(open_uri)
            .is_some_and(|current| current.version == *open_version)
    });
    let document_version_current = state
        .documents
        .get(uri)
        .is_some_and(|document| document.version == version);
    let generation_current =
        state.workspace_generations.get(root).copied().unwrap_or(0) == generation;

    document_version_current
        && overlay_versions_current
        && snapshot_uris == current_uris
        && generation_current
}

async fn begin_analysis_progress(
    client: &Client,
    token: Option<ProgressToken>,
) -> Option<ProgressToken> {
    let token = token?;
    client
        .send_request::<WorkDoneProgressCreate>(WorkDoneProgressCreateParams {
            token: token.clone(),
        })
        .await
        .ok()?;
    client
        .send_notification::<Progress>(ProgressParams {
            token: token.clone(),
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
                title: "Analyzing Zirk workspace".into(),
                cancellable: Some(false),
                message: Some("Loading and checking project sources".into()),
                percentage: None,
            })),
        })
        .await;
    Some(token)
}

async fn end_analysis_progress(client: &Client, token: Option<ProgressToken>, message: &str) {
    let Some(token) = token else { return };
    client
        .send_notification::<Progress>(ProgressParams {
            token,
            value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                message: Some(message.to_owned()),
            })),
        })
        .await;
}

/// The LSP endpoint. One global analysis lock serializes runs so a burst of
/// edits never stacks compiler invocations on top of each other.
pub struct Backend {
    client: Client,
    state: Arc<Mutex<State>>,
    analysis_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Backend {
    /// Builds the bounded filesystem index only when an import-path completion
    /// needs it, then reuses it until a watched project input changes.
    async fn module_index_for(
        &self,
        uri: &Url,
        analysis: &Arc<Analysis>,
    ) -> Option<Arc<zirk_ide::ModuleIndex>> {
        let path = uri.to_file_path().ok()?;
        let root = workspace_root(&path);
        let roots = analysis.frontend.as_ref()?.module_roots.clone();
        let (generation, cached) = {
            let state = self.state.lock().unwrap();
            (
                state.module_index_generation,
                state.module_indexes.get(&root).cloned(),
            )
        };
        if cached.is_some() {
            return cached;
        }

        let index = tokio::task::spawn_blocking(move || {
            zirk_ide::ModuleIndex::from_authorized_roots(&roots.local_root, &roots.packages)
        })
        .await
        .ok()?;
        let mut state = self.state.lock().unwrap();
        if state.module_index_generation != generation
            || !state
                .documents
                .get(uri)
                .and_then(|document| document.analysis.as_ref())
                .is_some_and(|current| Arc::ptr_eq(current, analysis))
        {
            return None;
        }
        Some(
            state
                .module_indexes
                .entry(root)
                .or_insert_with(|| Arc::new(index))
                .clone(),
        )
    }

    /// Ensures semantic providers observe a completed analysis for the latest
    /// open-document set, even when the client sends a request immediately
    /// after `didOpen` or `didChange`.
    async fn ensure_analysis(&self, uri: &Url) {
        let needs_analysis = self
            .state
            .lock()
            .unwrap()
            .documents
            .get(uri)
            .is_some_and(|document| document.analysis.is_none());
        if needs_analysis {
            self.run_analysis(uri.clone()).await;
        }
    }

    /// Returns the project snapshot for a file in the loaded module graph.
    /// If the user explicitly opens another Zirk file in that workspace,
    /// analyze that file as an independent root for semantic highlighting
    /// and document diagnostics. The fallback is versioned by the document
    /// and invalidated with the workspace snapshot, so it cannot color newer
    /// text with stale results.
    async fn analysis_for_semantic_tokens(&self, uri: &Url) -> Option<Arc<Analysis>> {
        self.ensure_analysis(uri).await;

        let (project_analysis, version, path, overlays) = {
            let state = self.state.lock().unwrap();
            let document = state.documents.get(uri)?;
            let project_analysis = document.analysis.as_ref()?.clone();
            if project_analysis
                .frontend
                .as_ref()
                .and_then(|frontend| file_id_for_uri(&frontend.sources, uri))
                .is_some()
            {
                return Some(project_analysis);
            }
            if let Some(fallback) = &document.semantic_fallback {
                return Some(fallback.clone());
            }

            let path = uri.to_file_path().ok()?;
            let root = workspace_root(&path);
            let overlays = state
                .documents
                .iter()
                .filter_map(|(open_uri, open_document)| {
                    let open_path = open_uri.to_file_path().ok()?;
                    (workspace_root(&open_path) == root)
                        .then(|| (open_path, open_document.text.clone()))
                })
                .collect::<HashMap<_, _>>();
            (project_analysis, document.version, path, overlays)
        };

        let fallback = tokio::task::spawn_blocking(move || analyze_with_overlays(&path, &overlays))
            .await
            .ok()
            .map(Arc::new)?;
        {
            let mut state = self.state.lock().unwrap();
            let document = state.documents.get_mut(uri)?;
            if document.version != version
                || !document
                    .analysis
                    .as_ref()
                    .is_some_and(|analysis| Arc::ptr_eq(analysis, &project_analysis))
            {
                return None;
            }
            document.semantic_fallback = Some(fallback.clone());
            if !document.published_uris.contains(uri) {
                document.published_uris.push(uri.clone());
            }
        }

        // The project entry analysis intentionally excludes files that are
        // outside its import graph. Publish the standalone pass's diagnostics
        // too, so the editor does not wait forever for a notification that
        // the project analysis cannot produce for this document.
        let mut diagnostics = Vec::new();
        for diagnostic in &fallback.diagnostics {
            let (file, lsp) = to_lsp_diagnostic(diagnostic);
            let target = file
                .and_then(|file| Url::from_file_path(&file).ok())
                .unwrap_or_else(|| uri.clone());
            if target == *uri {
                diagnostics.push(lsp);
            }
        }
        self.client
            .publish_diagnostics(uri.clone(), diagnostics, Some(version))
            .await;
        Some(fallback)
    }

    /// Runs the analysis for `uri`'s current text and publishes the outcome.
    ///
    /// Results are applied only when the document's version still matches the
    /// text that was analyzed — a superseded run is dropped, and the run for
    /// the newer text is already scheduled or in flight.
    async fn run_analysis(&self, uri: Url) {
        Self::run_analysis_shared(
            self.client.clone(),
            self.state.clone(),
            self.analysis_lock.clone(),
            uri,
        )
        .await;
    }

    /// Schedule analysis outside the notification handler so editor changes
    /// remain responsive while a compiler pass is running. The analysis gate
    /// and snapshot-version checks coalesce superseded work before publication.
    fn schedule_analysis(&self, uri: Url) {
        let client = self.client.clone();
        let state = self.state.clone();
        let analysis_lock = self.analysis_lock.clone();
        tokio::spawn(async move {
            Self::run_analysis_shared(client, state, analysis_lock, uri).await;
        });
    }

    fn invalidate_from_file_events(&self, changes: &[FileEvent]) -> Vec<Url> {
        let changed_paths = changes
            .iter()
            .filter_map(|event| event.uri.to_file_path().ok())
            .collect::<Vec<_>>();
        if changed_paths.is_empty() {
            return Vec::new();
        }

        let mut state = self.state.lock().unwrap();
        state.module_index_generation = state.module_index_generation.wrapping_add(1);
        state.module_indexes.clear();
        let mut affected_roots = std::collections::HashSet::new();
        for (uri, document) in &state.documents {
            let Ok(document_path) = uri.to_file_path() else {
                continue;
            };
            let root = workspace_root(&document_path);
            let loaded_dependency_changed = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
                .is_some_and(|frontend| {
                    frontend.sources.files().any(|(_, source)| {
                        changed_paths
                            .iter()
                            .any(|changed| Path::new(source.name()) == changed)
                    })
                });
            let workspace_input_changed = changed_paths.iter().any(|changed| {
                changed.starts_with(&root)
                    || (changed.file_name().is_some_and(|name| name == ".zkinit")
                        && changed
                            .parent()
                            .is_some_and(|parent| document_path.starts_with(parent)))
            });
            if loaded_dependency_changed || workspace_input_changed {
                affected_roots.insert(root);
            }
        }

        for root in &affected_roots {
            let generation = state.workspace_generations.entry(root.clone()).or_default();
            *generation = generation.wrapping_add(1);
        }

        let mut invalidated = Vec::new();
        for (uri, document) in &mut state.documents {
            if uri
                .to_file_path()
                .ok()
                .is_some_and(|path| affected_roots.contains(&workspace_root(&path)))
            {
                document.analysis = None;
                document.semantic_fallback = None;
                invalidated.push(uri.clone());
            }
        }
        invalidated
    }

    async fn run_analysis_shared(
        client: Client,
        state: Arc<Mutex<State>>,
        analysis_lock: Arc<tokio::sync::Mutex<()>>,
        uri: Url,
    ) {
        // Hold the gate before reading document state. A semantic request may
        // race the analysis scheduled by didOpen/didChange; after acquiring
        // the gate it must either reuse that completed snapshot or analyze
        // the newest overlay set itself.
        let permit = analysis_lock.lock().await;
        let (version, root, path, overlays, overlay_versions, generation, progress_token) = {
            let mut state = state.lock().unwrap();
            let Some(document) = state.documents.get(&uri) else {
                return;
            };
            if document.analysis.is_some() {
                return;
            }
            let version = document.version;
            let Ok(path) = uri.to_file_path() else {
                return;
            };
            let root = workspace_root(&path);
            let mut overlays = HashMap::new();
            let mut versions = HashMap::new();
            for (open_uri, open_document) in &state.documents {
                let Ok(open_path) = open_uri.to_file_path() else {
                    continue;
                };
                if workspace_root(&open_path) != root {
                    continue;
                }
                overlays.insert(open_path, open_document.text.clone());
                versions.insert(open_uri.clone(), open_document.version);
            }
            let generation = *state.workspace_generations.get(&root).unwrap_or(&0);
            let progress_token = if state.supports_work_done_progress {
                state.next_progress_id = state.next_progress_id.wrapping_add(1);
                Some(ProgressToken::String(format!(
                    "zirk-analysis-{}",
                    state.next_progress_id
                )))
            } else {
                None
            };
            (
                version,
                root,
                path,
                overlays,
                versions,
                generation,
                progress_token,
            )
        };
        let root_for_validation = root.clone();
        let progress_token = begin_analysis_progress(&client, progress_token).await;

        let analysis = match tokio::task::spawn_blocking(move || {
            let target = if root.join(".zkinit").is_file() {
                root
            } else {
                path
            };
            analyze_with_overlays(&target, &overlays)
        })
        .await
        {
            Ok(analysis) => analysis,
            Err(_) => {
                end_analysis_progress(&client, progress_token, "Workspace analysis failed").await;
                return;
            }
        };

        let outcome = {
            let mut state = state.lock().unwrap();
            if !analysis_snapshot_is_current(
                &state,
                &uri,
                &root_for_validation,
                version,
                &overlay_versions,
                generation,
            ) {
                None
            } else {
                let mut previously = std::collections::HashSet::new();
                for open_uri in overlay_versions.keys() {
                    if let Some(document) = state.documents.get_mut(open_uri) {
                        document.analysis = None;
                        document.semantic_fallback = None;
                        previously.extend(std::mem::take(&mut document.published_uris));
                    }
                }
                let analysis = Arc::new(analysis);
                for open_uri in overlay_versions.keys() {
                    if let Some(document) = state.documents.get_mut(open_uri) {
                        document.analysis = Some(analysis.clone());
                        document.semantic_fallback = None;
                    }
                }
                Some((analysis, previously.into_iter().collect::<Vec<_>>()))
            }
        };
        drop(permit);

        let Some((analysis, previously)) = outcome else {
            end_analysis_progress(
                &client,
                progress_token,
                "Superseded by newer workspace changes",
            )
            .await;
            return;
        };

        // Group by target URI so diagnostics on imported files reach their
        // own documents; a diagnostic with no location lands on the entry.
        let mut by_uri: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
        for diagnostic in &analysis.diagnostics {
            let (file, lsp) = to_lsp_diagnostic(diagnostic);
            let target = file
                .and_then(|file| Url::from_file_path(&file).ok())
                .unwrap_or_else(|| uri.clone());
            by_uri.entry(target).or_default().push(lsp);
        }

        let mut published = Vec::new();
        for (target, diags) in by_uri {
            published.push(target.clone());
            client
                .publish_diagnostics(
                    target.clone(),
                    diags,
                    overlay_versions.get(&target).copied(),
                )
                .await;
        }

        // The entry always gets a publish — even an empty one — so fixing the
        // last error clears the squiggles instead of leaving them behind.
        if !published.contains(&uri) {
            published.push(uri.clone());
            client
                .publish_diagnostics(
                    uri.clone(),
                    Vec::new(),
                    overlay_versions.get(&uri).copied().or(Some(version)),
                )
                .await;
        }
        for target in previously {
            if !published.contains(&target) {
                client.publish_diagnostics(target, Vec::new(), None).await;
            }
        }

        {
            let mut state = state.lock().unwrap();
            for open_uri in overlay_versions.keys() {
                if let Some(document) = state.documents.get_mut(open_uri) {
                    document.published_uris = published.clone();
                }
            }
        }
        end_analysis_progress(&client, progress_token, "Workspace analysis complete").await;
    }

    /// Replaces a document's text and schedules a fresh analysis for it.
    async fn update_document(&self, uri: Url, text: String, version: i32) {
        let root = uri.to_file_path().ok().map(|path| workspace_root(&path));
        let uris = {
            let mut state = self.state.lock().unwrap();
            if state
                .documents
                .get(&uri)
                .is_some_and(|current| version <= current.version)
            {
                return;
            }
            let published_uris = state
                .documents
                .get(&uri)
                .map(|document| document.published_uris.clone())
                .unwrap_or_default();
            let semantic_token_history = state
                .documents
                .get(&uri)
                .map(|document| document.semantic_token_history.clone())
                .unwrap_or_default();
            if let Some(root) = &root {
                let generation = state.workspace_generations.entry(root.clone()).or_default();
                *generation = generation.wrapping_add(1);
            }
            state.documents.insert(
                uri.clone(),
                Document {
                    text,
                    version,
                    analysis: None,
                    semantic_fallback: None,
                    published_uris,
                    semantic_token_history,
                },
            );
            let uris = state
                .documents
                .keys()
                .filter(|candidate| {
                    let Some(root) = &root else {
                        return *candidate == &uri;
                    };
                    candidate
                        .to_file_path()
                        .ok()
                        .is_some_and(|path| workspace_root(&path) == *root)
                })
                .cloned()
                .collect::<Vec<_>>();
            for open_uri in &uris {
                if let Some(document) = state.documents.get_mut(open_uri) {
                    document.analysis = None;
                    document.semantic_fallback = None;
                }
            }
            uris
        };
        for open_uri in uris {
            self.schedule_analysis(open_uri);
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> RpcResult<InitializeResult> {
        let semantic_tokens = negotiate_semantic_tokens(&params.capabilities);
        let supports_document_changes = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.workspace_edit.as_ref())
            .and_then(|workspace_edit| workspace_edit.document_changes)
            .unwrap_or(false);
        let supports_work_done_progress = params
            .capabilities
            .window
            .as_ref()
            .and_then(|window| window.work_done_progress)
            .unwrap_or(false);
        {
            let mut state = self.state.lock().unwrap();
            state.semantic_token_legend =
                semantic_tokens.as_ref().map(|(legend, _)| legend.clone());
            state.supports_document_changes = supports_document_changes;
            state.supports_work_done_progress = supports_work_done_progress;
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".to_owned()]),
                    resolve_provider: Some(true),
                    ..CompletionOptions::default()
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
                    retrigger_characters: Some(vec![",".to_owned()]),
                    ..SignatureHelpOptions::default()
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: supports_document_changes.then(|| {
                    CodeActionProviderCapability::Options(CodeActionOptions {
                        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
                        ..CodeActionOptions::default()
                    })
                }),
                inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                    InlayHintOptions::default(),
                ))),
                document_formatting_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                type_definition_provider: Some(TypeDefinitionProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                })),
                semantic_tokens_provider: semantic_tokens
                    .map(|(_, options)| SemanticTokensServerCapabilities::from(options)),
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

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        for uri in self.invalidate_from_file_events(&params.changes) {
            self.schedule_analysis(uri);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        let root = uri.to_file_path().ok().map(|path| workspace_root(&path));
        let remaining = {
            let mut state = self.state.lock().unwrap();
            state.documents.remove(&uri);
            if let Some(root) = &root {
                let generation = state.workspace_generations.entry(root.clone()).or_default();
                *generation = generation.wrapping_add(1);
            }
            let remaining = state
                .documents
                .keys()
                .filter(|candidate| {
                    let Some(root) = &root else { return false };
                    candidate
                        .to_file_path()
                        .ok()
                        .is_some_and(|path| workspace_root(&path) == *root)
                })
                .cloned()
                .collect::<Vec<_>>();
            for open_uri in &remaining {
                if let Some(document) = state.documents.get_mut(open_uri) {
                    document.analysis = None;
                    document.semantic_fallback = None;
                }
            }
            remaining
        };
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
        for open_uri in remaining {
            self.schedule_analysis(open_uri);
        }
    }

    async fn hover(&self, params: HoverParams) -> RpcResult<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        self.ensure_analysis(uri).await;

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

        let Some(file_id) = file_id_for_uri(&result.sources, uri) else {
            return Ok(None);
        };
        let file = result.sources.file(file_id);
        let offset = file.offset_at_utf16(position.line, position.character);
        if let Some((text, span)) = import_hover_content(
            &result.program,
            &result.checked.semantic_index,
            file_id,
            offset,
        ) {
            let (sl, sc) = file.line_column_utf16(span.start);
            let (el, ec) = file.line_column_utf16(span.end);
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: text,
                }),
                range: Some(Range::new(Position::new(sl, sc), Position::new(el, ec))),
            }));
        }
        let Some(entry) = best_hover_entry_in_file(&result.checked.type_map, file_id, offset)
        else {
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

    async fn completion(&self, params: CompletionParams) -> RpcResult<Option<CompletionResponse>> {
        let cursor = params.text_document_position;
        let uri = &cursor.text_document.uri;
        self.ensure_analysis(uri).await;
        let Some((analysis, file, version)) = ({
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(uri) else {
                return Ok(None);
            };
            let Some(analysis) = document.analysis.as_ref() else {
                return Ok(None);
            };
            let Some(frontend) = analysis.frontend.as_ref() else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, uri) else {
                return Ok(None);
            };
            Some((analysis.clone(), file, document.version))
        }) else {
            return Ok(None);
        };
        let Some(frontend) = analysis.frontend.as_ref() else {
            return Ok(None);
        };
        let position = IdePosition {
            line: cursor.position.line,
            character: cursor.position.character,
        };
        let snapshot =
            IdeSnapshot::new(version, &frontend.sources, &frontend.checked.semantic_index);
        let module_index = if snapshot.completion_needs_module_index(file, position) {
            self.module_index_for(uri, &analysis).await
        } else {
            None
        };
        let current = self
            .state
            .lock()
            .unwrap()
            .documents
            .get(uri)
            .is_some_and(|document| {
                document.version == version
                    && document
                        .analysis
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &analysis))
            });
        if !current {
            return Ok(None);
        }
        Ok(Some(CompletionResponse::Array(completion_items(
            &frontend.checked.semantic_index,
            file,
            position,
            &frontend.sources,
            uri,
            version,
            module_index.as_deref(),
        ))))
    }

    async fn completion_resolve(&self, mut item: CompletionItem) -> RpcResult<CompletionItem> {
        let Some(data) = item.data.clone() else {
            return Ok(item);
        };
        let Some(origin_uri) = data
            .get("originUri")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Url::parse(value).ok())
        else {
            return Ok(item);
        };
        let Some(origin_version) = data
            .get("originVersion")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
        else {
            return Ok(item);
        };
        let Some(declaration_uri) = data
            .get("declarationUri")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| Url::parse(value).ok())
        else {
            return Ok(item);
        };
        let Some(start) = data
            .get("start")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return Ok(item);
        };
        let Some(end) = data
            .get("end")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            return Ok(item);
        };

        self.ensure_analysis(&origin_uri).await;
        let state = self.state.lock().unwrap();
        let Some(document) = state.documents.get(&origin_uri) else {
            return Ok(item);
        };
        if document.version != origin_version {
            return Ok(item);
        }
        let Some(frontend) = document
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.frontend.as_ref())
        else {
            return Ok(item);
        };
        let Some(file) = file_id_for_uri(&frontend.sources, &declaration_uri) else {
            return Ok(item);
        };
        let span = Span::in_file(file, start, end);
        let Some(symbol) = frontend.checked.semantic_index.symbol_at_declaration(span) else {
            return Ok(item);
        };
        if item.detail.is_none() {
            item.detail = symbol.type_text.clone();
        }
        if item.documentation.is_none() {
            item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: symbol.type_text.as_ref().map_or_else(
                    || format!("`{}`", symbol.name),
                    |type_text| format!("```zirk\n{}\n```", type_text),
                ),
            }));
        }
        Ok(item)
    }

    async fn code_action(&self, params: CodeActionParams) -> RpcResult<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        self.ensure_analysis(uri).await;
        let state = self.state.lock().unwrap();
        if !state.supports_document_changes {
            return Ok(None);
        }
        let Some(document) = state.documents.get(uri) else {
            return Ok(None);
        };
        let Some(analysis) = document.analysis.as_ref() else {
            return Ok(None);
        };
        let Some(frontend) = analysis.frontend.as_ref() else {
            return Ok(Some(Vec::new()));
        };
        let Some(file) = file_id_for_uri(&frontend.sources, uri) else {
            return Ok(Some(Vec::new()));
        };
        let source = frontend.sources.file(file);
        let mut actions = Vec::new();
        for requested in &params.context.diagnostics {
            if !ranges_intersect(params.range, requested.range) {
                continue;
            }
            // LSP diagnostics can be stale or client-authored. Only offer an
            // edit when the exact diagnostic still exists in this generation.
            let is_current = analysis.diagnostics.iter().any(|diagnostic| {
                let (path, current) = to_lsp_diagnostic(diagnostic);
                path.as_deref() == Some(source.name())
                    && current.code == requested.code
                    && current.range == requested.range
                    && current.message == requested.message
                    && current.source == requested.source
            });
            if !is_current {
                continue;
            }
            if let Some(action) = versioned_type_quickfix(uri, document.version, source, requested)
            {
                actions.push(CodeActionOrCommand::CodeAction(action));
            }
        }
        Ok(Some(actions))
    }

    async fn signature_help(
        &self,
        params: SignatureHelpParams,
    ) -> RpcResult<Option<SignatureHelp>> {
        let cursor = params.text_document_position_params;
        let uri = &cursor.text_document.uri;
        self.ensure_analysis(uri).await;
        let state = self.state.lock().unwrap();
        let Some(document) = state.documents.get(uri) else {
            return Ok(None);
        };
        let Some(frontend) = document
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.frontend.as_ref())
        else {
            return Ok(None);
        };
        let Some(file) = file_id_for_uri(&frontend.sources, uri) else {
            return Ok(None);
        };
        let Some(mut signature) = IdeSnapshot::new(
            document.version,
            &frontend.sources,
            &frontend.checked.semantic_index,
        )
        .signature_help(
            file,
            IdePosition {
                line: cursor.position.line,
                character: cursor.position.character,
            },
        ) else {
            return Ok(None);
        };
        let source = frontend.sources.file(file);
        let offset = source.offset_at_utf16(cursor.position.line, cursor.position.character);
        let substitutions =
            generic_substitutions_at(frontend, file, offset, &signature.callee_name);
        signature.apply_type_substitutions(&substitutions);
        let parameters = signature
            .parameters
            .into_iter()
            .map(|label| ParameterInformation {
                label: ParameterLabel::Simple(label),
                documentation: None,
            })
            .collect();
        Ok(Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: signature.label,
                documentation: None,
                parameters: Some(parameters),
                active_parameter: Some(signature.active_parameter),
            }],
            active_signature: Some(0),
            active_parameter: Some(signature.active_parameter),
        }))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> RpcResult<Option<GotoDefinitionResponse>> {
        let cursor = params.text_document_position_params;
        self.ensure_analysis(&cursor.text_document.uri).await;
        let result = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(&cursor.text_document.uri) else {
                return Ok(None);
            };
            let Some(frontend) = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
            else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, &cursor.text_document.uri) else {
                return Ok(None);
            };
            let query = IdeSnapshot::new(
                document.version,
                &frontend.sources,
                &frontend.checked.semantic_index,
            );
            query.definition(
                file,
                IdePosition {
                    line: cursor.position.line,
                    character: cursor.position.character,
                },
            )
        };
        let Some(location) = result else {
            return Ok(None);
        };
        let Ok(uri) = Url::from_file_path(location.path) else {
            return Ok(None);
        };
        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range: lsp_range(location.range),
        })))
    }

    async fn goto_type_definition(
        &self,
        params: GotoTypeDefinitionParams,
    ) -> RpcResult<Option<GotoTypeDefinitionResponse>> {
        let cursor = params.text_document_position_params;
        self.ensure_analysis(&cursor.text_document.uri).await;
        let result = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(&cursor.text_document.uri) else {
                return Ok(None);
            };
            let Some(frontend) = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
            else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, &cursor.text_document.uri) else {
                return Ok(None);
            };
            let query = IdeSnapshot::new(
                document.version,
                &frontend.sources,
                &frontend.checked.semantic_index,
            );
            query.type_definition(
                file,
                IdePosition {
                    line: cursor.position.line,
                    character: cursor.position.character,
                },
            )
        };
        let Some(location) = result else {
            return Ok(None);
        };
        let Ok(uri) = Url::from_file_path(location.path) else {
            return Ok(None);
        };
        Ok(Some(GotoTypeDefinitionResponse::Scalar(Location {
            uri,
            range: lsp_range(location.range),
        })))
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> RpcResult<Option<PrepareRenameResponse>> {
        let uri = &params.text_document.uri;
        self.ensure_analysis(uri).await;
        let prepared = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(uri) else {
                return Ok(None);
            };
            if document
                .analysis
                .as_ref()
                .is_some_and(|analysis| analysis.has_unresolved_imports())
            {
                return Ok(None);
            }
            let Some(frontend) = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
            else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, uri) else {
                return Ok(None);
            };
            IdeSnapshot::new(
                document.version,
                &frontend.sources,
                &frontend.checked.semantic_index,
            )
            .prepare_rename(
                file,
                IdePosition {
                    line: params.position.line,
                    character: params.position.character,
                },
            )
        };
        Ok(
            prepared.map(|target| PrepareRenameResponse::RangeWithPlaceholder {
                range: lsp_range(target.range),
                placeholder: target.placeholder,
            }),
        )
    }

    async fn rename(&self, params: RenameParams) -> RpcResult<Option<WorkspaceEdit>> {
        let cursor = params.text_document_position;
        let uri = cursor.text_document.uri;
        self.ensure_analysis(&uri).await;
        let prepared = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(&uri) else {
                return Ok(None);
            };
            if document
                .analysis
                .as_ref()
                .is_some_and(|analysis| analysis.has_unresolved_imports())
            {
                return Ok(None);
            }
            let Some(frontend) = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
            else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, &uri) else {
                return Ok(None);
            };
            let query = IdeSnapshot::new(
                document.version,
                &frontend.sources,
                &frontend.checked.semantic_index,
            );
            let edits = match query.rename(
                file,
                IdePosition {
                    line: cursor.position.line,
                    character: cursor.position.character,
                },
                &params.new_name,
            ) {
                Ok(edits) if !edits.is_empty() => edits,
                _ => return Ok(None),
            };

            let entry_path = uri.to_file_path().ok();
            let Some(entry_path) = entry_path else {
                return Ok(None);
            };
            let root = workspace_root(&entry_path);
            let analysis_path = if root.join(".zkinit").is_file() {
                root
            } else {
                entry_path
            };
            let mut overlays = HashMap::new();
            let mut versions = HashMap::new();
            for (open_uri, open_document) in &state.documents {
                let Ok(open_path) = open_uri.to_file_path() else {
                    continue;
                };
                if workspace_root(&open_path) == workspace_root(&analysis_path) {
                    overlays.insert(open_path, open_document.text.clone());
                    versions.insert(open_uri.clone(), open_document.version);
                }
            }
            let source_texts = frontend
                .sources
                .files()
                .map(|(_, source)| {
                    (
                        std::path::PathBuf::from(source.name()),
                        source.text().to_owned(),
                    )
                })
                .collect::<HashMap<_, _>>();
            let Some(candidate_overlays) = apply_rename_edits(overlays, &source_texts, &edits)
            else {
                return Ok(None);
            };
            let expectations =
                reference_expectations(&frontend.sources, &frontend.checked.semantic_index);
            let supports_document_changes = state.supports_document_changes;
            (
                edits,
                candidate_overlays,
                analysis_path,
                expectations,
                versions,
                supports_document_changes,
            )
        };

        let (edits, overlays, analysis_path, expectations, versions, supports_document_changes) =
            prepared;
        let candidate_edits = edits.clone();
        let safe = tokio::task::spawn_blocking(move || {
            rename_preserves_bindings(&analysis_path, &overlays, &candidate_edits, &expectations)
        })
        .await
        .unwrap_or(false);
        if !safe {
            return Ok(None);
        }
        {
            let state = self.state.lock().unwrap();
            if !versions.iter().all(|(open_uri, version)| {
                state
                    .documents
                    .get(open_uri)
                    .is_some_and(|document| document.version == *version)
            }) {
                return Ok(None);
            }
        }
        Ok(workspace_edit(edits, &versions, supports_document_changes))
    }

    async fn references(&self, params: ReferenceParams) -> RpcResult<Option<Vec<Location>>> {
        let cursor = params.text_document_position;
        self.ensure_analysis(&cursor.text_document.uri).await;
        let locations = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(&cursor.text_document.uri) else {
                return Ok(None);
            };
            let Some(frontend) = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
            else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, &cursor.text_document.uri) else {
                return Ok(None);
            };
            let query = IdeSnapshot::new(
                document.version,
                &frontend.sources,
                &frontend.checked.semantic_index,
            );
            query.references(
                file,
                IdePosition {
                    line: cursor.position.line,
                    character: cursor.position.character,
                },
                params.context.include_declaration,
            )
        };

        let locations = locations
            .into_iter()
            .filter_map(|item| {
                Some(Location {
                    uri: Url::from_file_path(item.location.path).ok()?,
                    range: lsp_range(item.location.range),
                })
            })
            .collect();
        Ok(Some(locations))
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> RpcResult<Option<Vec<DocumentHighlight>>> {
        let cursor = params.text_document_position_params;
        self.ensure_analysis(&cursor.text_document.uri).await;
        let highlights = {
            let state = self.state.lock().unwrap();
            let Some(document) = state.documents.get(&cursor.text_document.uri) else {
                return Ok(None);
            };
            let Some(frontend) = document
                .analysis
                .as_ref()
                .and_then(|analysis| analysis.frontend.as_ref())
            else {
                return Ok(None);
            };
            let Some(file) = file_id_for_uri(&frontend.sources, &cursor.text_document.uri) else {
                return Ok(None);
            };
            let query = IdeSnapshot::new(
                document.version,
                &frontend.sources,
                &frontend.checked.semantic_index,
            );
            query.document_highlights(
                file,
                IdePosition {
                    line: cursor.position.line,
                    character: cursor.position.character,
                },
            )
        };
        Ok(Some(
            highlights
                .into_iter()
                .map(|item| DocumentHighlight {
                    range: lsp_range(item.location.range),
                    kind: Some(if item.role == Some(zirk_sema::ReferenceRole::Write) {
                        DocumentHighlightKind::WRITE
                    } else {
                        DocumentHighlightKind::READ
                    }),
                })
                .collect(),
        ))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> RpcResult<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let Some(analysis) = self.analysis_for_semantic_tokens(&uri).await else {
            return Ok(None);
        };
        let tokens = {
            let mut state = self.state.lock().unwrap();
            let Some(legend) = state.semantic_token_legend.clone() else {
                return Ok(None);
            };
            let Some(document) = state.documents.get_mut(&uri) else {
                return Ok(None);
            };
            let Some(tokens) = semantic_tokens_for(&analysis, &uri, None, &legend) else {
                return Ok(None);
            };
            remember_semantic_tokens(&mut document.semantic_token_history, tokens.clone());
            tokens
        };
        Ok(Some(SemanticTokensResult::Tokens(tokens)))
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> RpcResult<Option<SemanticTokensFullDeltaResult>> {
        let uri = params.text_document.uri;
        let Some(analysis) = self.analysis_for_semantic_tokens(&uri).await else {
            return Ok(None);
        };
        let result = {
            let mut state = self.state.lock().unwrap();
            let Some(legend) = state.semantic_token_legend.clone() else {
                return Ok(None);
            };
            let Some(document) = state.documents.get_mut(&uri) else {
                return Ok(None);
            };
            let Some(current) = semantic_tokens_for(&analysis, &uri, None, &legend) else {
                return Ok(None);
            };
            let previous = document
                .semantic_token_history
                .iter()
                .find(|tokens| tokens.result_id.as_deref() == Some(&params.previous_result_id))
                .cloned();
            remember_semantic_tokens(&mut document.semantic_token_history, current.clone());
            previous.map_or_else(
                || SemanticTokensFullDeltaResult::Tokens(current.clone()),
                |previous| {
                    SemanticTokensFullDeltaResult::TokensDelta(semantic_tokens_delta(
                        &previous, &current,
                    ))
                },
            )
        };
        Ok(Some(result))
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> RpcResult<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let Some(analysis) = self.analysis_for_semantic_tokens(&uri).await else {
            return Ok(None);
        };
        let state = self.state.lock().unwrap();
        let Some(legend) = state.semantic_token_legend.as_ref() else {
            return Ok(None);
        };
        if !state.documents.contains_key(&uri) {
            return Ok(None);
        }
        let Some(tokens) = semantic_tokens_for(&analysis, &uri, Some(params.range), legend) else {
            return Ok(None);
        };
        Ok(Some(SemanticTokensRangeResult::Tokens(tokens)))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> RpcResult<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        self.ensure_analysis(uri).await;
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
        let Some(file_id) = file_id_for_uri(&result.sources, uri) else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(
            document_symbols_in_file(&result.program, &result.sources, file_id),
        )))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> RpcResult<Option<Vec<InlayHint>>> {
        let uri = &params.text_document.uri;
        self.ensure_analysis(uri).await;
        let state = self.state.lock().unwrap();
        let Some(document) = state.documents.get(uri) else {
            return Ok(None);
        };
        let Some(frontend) = document
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.frontend.as_ref())
        else {
            return Ok(None);
        };
        let Some(file) = file_id_for_uri(&frontend.sources, uri) else {
            return Ok(None);
        };
        let mut hints = inferred_type_hints(
            &frontend.checked.type_map,
            &frontend.sources,
            file,
            params.range,
        );
        let snapshot = IdeSnapshot::new(
            document.version,
            &frontend.sources,
            &frontend.checked.semantic_index,
        );
        hints.extend(
            snapshot
                .parameter_hints(
                    file,
                    zirk_ide::TextRange {
                        start: IdePosition {
                            line: params.range.start.line,
                            character: params.range.start.character,
                        },
                        end: IdePosition {
                            line: params.range.end.line,
                            character: params.range.end.character,
                        },
                    },
                )
                .into_iter()
                .map(|hint| InlayHint {
                    position: Position::new(hint.position.line, hint.position.character),
                    label: hint.label.into(),
                    kind: Some(InlayHintKind::PARAMETER),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(false),
                    padding_right: Some(true),
                    data: None,
                }),
        );
        hints.sort_by_key(|hint| hint.position);
        Ok(Some(hints))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> RpcResult<Option<Vec<SymbolInformation>>> {
        let state = self.state.lock().unwrap();
        let Some(frontend) = state
            .documents
            .values()
            .filter_map(|document| document.analysis.as_ref())
            .filter_map(|analysis| analysis.frontend.as_ref())
            .next()
        else {
            return Ok(Some(Vec::new()));
        };
        Ok(Some(workspace_symbols(
            &frontend.sources,
            &frontend.checked.semantic_index,
            &params.query,
        )))
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

        Ok(formatting_edit(&name, &text).map(|edit| vec![edit]))
    }
}

/// Builds the `(service, socket)` pair `main` serves over stdio.
pub fn service() -> (LspService<Backend>, tower_lsp::ClientSocket) {
    let (service, socket, _) = service_with_state();
    (service, socket)
}

fn service_with_state() -> (
    LspService<Backend>,
    tower_lsp::ClientSocket,
    Arc<Mutex<State>>,
) {
    let state = Arc::new(Mutex::new(State::default()));
    let backend_state = state.clone();
    let analysis_lock = Arc::new(tokio::sync::Mutex::new(()));
    let (service, socket) = LspService::new(move |client| Backend {
        client,
        state: backend_state.clone(),
        analysis_lock: analysis_lock.clone(),
    });
    (service, socket, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tower::{Service, ServiceExt};
    use tower_lsp::jsonrpc::Request;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zirk-lsp-test-{}-{}", std::process::id(), tag));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const EXAMPLE: &str = "import { stdout } from std.io;\n\nfn square(n: Int32) {\n    return n * n;\n}\n\nfn main() {\n    stdout.println(square(7));\n}\n";

    #[test]
    fn workspace_generation_and_overlay_versions_reject_stale_analysis() {
        let root = scratch("analysis-generation");
        let first = Url::from_file_path(root.join("first.zrk")).unwrap();
        let second = Url::from_file_path(root.join("second.zrk")).unwrap();
        let mut state = State::default();
        for (uri, version) in [(&first, 3), (&second, 8)] {
            state.documents.insert(
                uri.clone(),
                Document {
                    text: String::new(),
                    version,
                    analysis: None,
                    semantic_fallback: None,
                    published_uris: Vec::new(),
                    semantic_token_history: VecDeque::new(),
                },
            );
        }
        let overlay_versions = HashMap::from([(first.clone(), 3), (second.clone(), 8)]);
        assert!(analysis_snapshot_is_current(
            &state,
            &first,
            &root,
            3,
            &overlay_versions,
            0,
        ));

        state.workspace_generations.insert(root.clone(), 1);
        assert!(!analysis_snapshot_is_current(
            &state,
            &first,
            &root,
            3,
            &overlay_versions,
            0,
        ));

        state.workspace_generations.clear();
        state.documents.get_mut(&second).unwrap().version = 9;
        assert!(!analysis_snapshot_is_current(
            &state,
            &first,
            &root,
            3,
            &overlay_versions,
            0,
        ));

        state.documents.get_mut(&second).unwrap().version = 8;
        state.documents.remove(&second);
        assert!(!analysis_snapshot_is_current(
            &state,
            &first,
            &root,
            3,
            &overlay_versions,
            0,
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn analysis_reports_diagnostics_for_broken_text() {
        let dir = scratch("broken");
        let path = dir.join("broken.zrk");
        let analysis = analyze(&path, "fn broken(x: Int32) {\n    return x + ;\n}\n");
        assert!(analysis.frontend.is_none());
        assert!(!analysis.diagnostics.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn semantic_tokens_analyze_open_files_outside_the_project_import_graph() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let detached = fixture.join("src/debug_main.zrk");
        let entry_text = fs::read_to_string(&entry).unwrap();
        let detached_text = fs::read_to_string(&detached).unwrap();
        let detached_uri = Url::from_file_path(&detached).unwrap();
        let project_analysis = Arc::new(analyze_with_overlays(
            &fixture,
            &HashMap::from([
                (entry, entry_text),
                (detached.clone(), detached_text.clone()),
            ]),
        ));
        assert!(
            file_id_for_uri(
                &project_analysis.frontend.as_ref().unwrap().sources,
                &detached_uri,
            )
            .is_none(),
            "the fixture file is intentionally not imported by the project entry"
        );

        let (service, _socket, state) = service_with_state();
        state.lock().unwrap().documents.insert(
            detached_uri.clone(),
            Document {
                text: detached_text,
                version: 1,
                analysis: Some(project_analysis),
                semantic_fallback: None,
                published_uris: Vec::new(),
                semantic_token_history: VecDeque::new(),
            },
        );

        let standalone = service
            .inner()
            .analysis_for_semantic_tokens(&detached_uri)
            .await
            .expect("an opened standalone Zirk file gets semantic analysis");
        let frontend = standalone.frontend.as_ref().expect("valid standalone file");
        assert!(file_id_for_uri(&frontend.sources, &detached_uri).is_some());
        let legend = SemanticTokensLegend {
            token_types: SEMANTIC_TOKEN_TYPES
                .iter()
                .map(|token_type| SemanticTokenType::new(token_type))
                .collect(),
            token_modifiers: SEMANTIC_TOKEN_MODIFIERS
                .iter()
                .map(|modifier| SemanticTokenModifier::new(modifier))
                .collect(),
        };
        let tokens = semantic_tokens_for(&standalone, &detached_uri, None, &legend)
            .expect("standalone file produces semantic tokens");
        assert!(!tokens.data.is_empty());

        let cached = service
            .inner()
            .analysis_for_semantic_tokens(&detached_uri)
            .await
            .expect("the standalone semantic snapshot is cached");
        assert!(Arc::ptr_eq(&standalone, &cached));
    }

    #[test]
    fn document_formatting_returns_one_full_edit_or_no_edit_for_invalid_text() {
        let text = "fn main(): Void {  stdout.println(  1 ); }\n";
        let edit = formatting_edit("memory.zrk", text).expect("formatting edit");
        assert_eq!(edit.range.start, Position::new(0, 0));
        assert_eq!(edit.range.end, Position::new(1, 0));
        assert_eq!(edit.new_text, "fn main(): Void { stdout.println(1); }\n");
        assert!(formatting_edit("broken.zrk", "fn main(: Void {\n").is_none());
    }

    #[test]
    fn semantic_token_delta_edits_are_aligned_to_five_integer_records() {
        let token = |start| SemanticToken {
            delta_line: 0,
            delta_start: start,
            length: 1,
            token_type: 8,
            token_modifiers_bitset: 0,
        };
        let previous = SemanticTokens {
            result_id: Some("old".into()),
            data: vec![token(0), token(2), token(4)],
        };
        let current = SemanticTokens {
            result_id: Some("new".into()),
            data: vec![token(0), token(3), token(4)],
        };

        let delta = semantic_tokens_delta(&previous, &current);
        assert_eq!(delta.result_id.as_deref(), Some("new"));
        assert_eq!(delta.edits.len(), 1);
        assert_eq!(delta.edits[0].start, 5);
        assert_eq!(delta.edits[0].delete_count, 5);
        assert_eq!(delta.edits[0].data.as_deref(), Some(&[token(3)][..]));
    }

    #[test]
    fn semantic_tokens_are_only_advertised_when_the_client_negotiates_them() {
        assert!(negotiate_semantic_tokens(&ClientCapabilities::default()).is_none());
        let (_, options) = negotiate_semantic_tokens(
            &serde_json::from_value(serde_json::json!({
                "textDocument": {
                    "semanticTokens": {
                        "requests": {"range": true, "full": {"delta": true}},
                        "tokenTypes": ["function"],
                        "tokenModifiers": ["declaration"],
                        "formats": ["relative"]
                    }
                }
            }))
            .unwrap(),
        )
        .expect("client negotiated semantic tokens");
        assert_eq!(options.legend.token_types[0].as_str(), "function");
        assert_eq!(options.legend.token_modifiers[0].as_str(), "declaration");
    }

    #[test]
    fn semantic_token_categories_preserve_local_import_and_reference_roles() {
        use zirk_sema::{SemanticSymbolKind as Kind, SemanticVisibility as Visibility};

        assert_eq!(
            semantic_token_types(Kind::Variable, Visibility::Local)[0],
            "local"
        );
        assert_eq!(
            semantic_token_types(Kind::Variable, Visibility::Module)[0],
            "variable"
        );
        assert_eq!(
            semantic_token_types(Kind::Import, Visibility::Local)[0],
            "variable"
        );
        for (kind, expected) in [
            (Kind::Module, "namespace"),
            (Kind::Type, "type"),
            (Kind::TypeAlias, "type"),
            (Kind::Class, "class"),
            (Kind::Contract, "interface"),
            (Kind::Enum, "enum"),
            (Kind::EnumVariant, "enumMember"),
            (Kind::TypeParameter, "typeParameter"),
            (Kind::Parameter, "parameter"),
            (Kind::Field, "property"),
            (Kind::Function, "function"),
            (Kind::Method, "method"),
        ] {
            assert_eq!(semantic_token_types(kind, Visibility::Module)[0], expected);
        }
        assert!(SEMANTIC_TOKEN_MODIFIERS.contains(&"declaration"));
        assert!(SEMANTIC_TOKEN_MODIFIERS.contains(&"reference"));
        assert!(SEMANTIC_TOKEN_MODIFIERS.contains(&"import"));
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
    fn member_completion_excludes_private_members() {
        let dir = scratch("private-member-completion");
        let path = dir.join("members.zrk");
        let text = "class Box {\n    private value: Int32;\n    construct() { this.value = 1; }\n    private hide(): Void {}\n    reveal(): Void {}\n}\nfn main(): Void {\n    inmut box = Box();\n    box.reveal();\n}\n";
        let analysis = analyze(&path, text);
        let frontend = analysis.frontend.unwrap();
        let offset = text.find("box.reveal").unwrap() as u32 + "box.".len() as u32;
        let (line, character) = frontend.sources.entry().line_column_utf16(offset);
        let snapshot = IdeSnapshot::new(1, &frontend.sources, &frontend.checked.semantic_index);
        let completions = snapshot.completions(FileId::ENTRY, IdePosition { line, character });

        assert!(
            completions
                .iter()
                .any(|candidate| candidate.label == "reveal")
        );
        assert!(
            !completions
                .iter()
                .any(|candidate| candidate.label == "hide")
        );
        assert!(
            !completions
                .iter()
                .any(|candidate| candidate.label == "value")
        );
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
    fn import_hovers_describe_export_kind_alias_and_source_module() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let text = fs::read_to_string(&entry).unwrap();
        let analysis = analyze(&entry, &text);
        let frontend = analysis.frontend.expect("valid semantic fixture");
        let file = frontend.sources.file(FileId::ENTRY);
        let semantics = &frontend.checked.semantic_index;
        let hover_at = |needle: &str, relative_offset: usize| {
            let offset = text.find(needle).unwrap() + relative_offset;
            import_hover_content(&frontend.program, semantics, FileId::ENTRY, offset as u32)
                .unwrap()
                .0
        };

        let identity = "identity -> keep";
        let function_hover = hover_at(identity, 2);
        assert!(
            function_hover.contains("fn identity(value: T): T"),
            "{function_hover}"
        );
        assert!(function_hover.contains("Imported as `keep` from `./domain/geometry`"));
        let alias_hover = hover_at(identity, "identity -> ".len() + 1);
        assert!(
            alias_hover.contains("fn identity(value: T): T"),
            "{alias_hover}"
        );

        let standard_export_hover = hover_at("stdout", 1);
        assert!(
            standard_export_hover.contains("module stdout"),
            "{standard_export_hover}"
        );
        assert!(standard_export_hover.contains("Standard library export"));

        let contract_hover = hover_at("Named -> NamedValue", 1);
        assert!(
            contract_hover.contains("contract Named"),
            "{contract_hover}"
        );
        let class_hover = hover_at("User, Named", 1);
        assert!(class_hover.contains("class User"), "{class_hover}");

        let standard_module_hover = hover_at("std.io", 2);
        assert!(standard_module_hover.contains("module std.io"));
        assert!(standard_module_hover.contains("standard library module"));
        let local_module_hover = hover_at("./domain/geometry", 3);
        assert!(local_module_hover.contains("module \"./domain/geometry\""));
        assert!(local_module_hover.contains("local module"));

        let typed_root = scratch("import-hover-types");
        let source_dir = typed_root.join("src");
        fs::create_dir_all(&source_dir).unwrap();
        let typed_entry = source_dir.join("main.zrk");
        fs::write(
            source_dir.join("types.zrk"),
            "share type UserId = Int32;\nshare class Account { construct() { } }\n",
        )
        .unwrap();
        let typed_text = "import { UserId, Account } from \"./types\";\nfn main(): Void { inmut id: UserId = 5; }\n";
        let typed_analysis = analyze(&typed_entry, typed_text);
        let typed_frontend = typed_analysis.frontend.expect("valid type import fixture");
        let typed_semantics = &typed_frontend.checked.semantic_index;
        let typed_hover_at = |needle: &str| {
            let offset = typed_text.find(needle).unwrap() as u32;
            import_hover_content(
                &typed_frontend.program,
                typed_semantics,
                FileId::ENTRY,
                offset,
            )
            .unwrap()
            .0
        };
        let alias_type_hover = typed_hover_at("UserId");
        assert!(
            alias_type_hover.contains("type UserId = Int32"),
            "{alias_type_hover}"
        );
        let class_hover = typed_hover_at("Account");
        assert!(class_hover.contains("class Account"), "{class_hover}");

        assert_eq!(file.id(), FileId::ENTRY);
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
    fn promoted_constructor_parameters_are_field_symbols() {
        let dir = scratch("promoted-symbols");
        let path = dir.join("promoted.zrk");
        let result = analyze(
            &path,
            "class User { construct(public name: String) { } } fn main(): Void { }",
        );
        let frontend = result.frontend.as_ref().unwrap();
        let symbols = document_symbols(&frontend.program, &frontend.sources);
        let user = symbols.iter().find(|symbol| symbol.name == "User").unwrap();
        assert!(
            user.children
                .as_ref()
                .unwrap()
                .iter()
                .any(|symbol| symbol.name == "name" && symbol.kind == SymbolKind::FIELD)
        );
        assert!(frontend.checked.type_map.iter().any(|entry| {
            entry.name.as_deref() == Some("name")
                && entry.kind == TypeMapKind::Field
                && hover_text(entry) == "name: String"
        }));
    }

    #[test]
    fn implicit_constructor_is_not_a_document_symbol() {
        let dir = scratch("implicit-constructor-symbols");
        let path = dir.join("implicit.zrk");
        let result = analyze(
            &path,
            "class Marker { } fn main(): Void { mut marker = Marker(); }",
        );
        let frontend = result.frontend.as_ref().unwrap();
        let symbols = document_symbols(&frontend.program, &frontend.sources);
        let marker = symbols
            .iter()
            .find(|symbol| symbol.name == "Marker")
            .unwrap();
        assert!(marker.children.is_none());
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

    #[test]
    fn semantic_fixture_checks_local_imports_aliases_generics_and_contracts() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let text = fs::read_to_string(&entry).unwrap();
        let analysis = analyze(&entry, &text);

        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let frontend = analysis.frontend.expect("valid fixture frontend");
        assert!(frontend.sources.files().count() >= 3);
        assert!(
            frontend
                .sources
                .files()
                .any(|(_, file)| file.name().ends_with("geometry.zrk"))
        );
        assert!(
            frontend
                .sources
                .files()
                .any(|(_, file)| file.name().ends_with("model.zrk"))
        );
        assert!(
            !frontend
                .sources
                .files()
                .any(|(_, file)| file.name().ends_with("not_in_root.zrk"))
        );
        assert!(frontend.checked.type_map.iter().any(|entry| {
            entry.name.as_deref() == Some("identity") && entry.kind == TypeMapKind::Function
        }));
        assert!(
            frontend
                .program
                .contracts
                .iter()
                .any(|contract| contract.name.name == "Named")
        );

        let named_use = text.find("value: NamedValue").unwrap() as u32 + "value: ".len() as u32;
        let type_reference = frontend
            .checked
            .semantic_index
            .reference_at_position(FileId::ENTRY, named_use)
            .expect("the imported type use resolves semantically");
        assert_eq!(
            frontend
                .checked
                .semantic_index
                .symbol(type_reference.target)
                .unwrap()
                .name,
            "Named"
        );

        let println_declarations = frontend
            .checked
            .semantic_index
            .symbols()
            .iter()
            .filter(|symbol| {
                symbol.name == "println" && symbol.kind == zirk_sema::SemanticSymbolKind::Function
            })
            .map(|symbol| symbol.id)
            .collect::<std::collections::HashSet<_>>();
        assert!(
            println_declarations.len() >= 2,
            "same-named declarations keep distinct identities"
        );

        let alias_use = text.find("keep(user").unwrap() as u32;
        let reference = frontend
            .checked
            .semantic_index
            .reference_at_position(FileId::ENTRY, alias_use)
            .expect("the alias use resolves to a semantic reference");
        let target = frontend
            .checked
            .semantic_index
            .symbol(reference.target)
            .expect("the reference target belongs to this snapshot");
        assert_eq!(target.name, "identity");
        assert!(
            frontend
                .sources
                .file(target.selection_span.file)
                .name()
                .ends_with("geometry.zrk")
        );

        let (line, character) = frontend.sources.entry().line_column_utf16(alias_use);
        let query =
            zirk_ide::IdeSnapshot::new(7, &frontend.sources, &frontend.checked.semantic_index);
        let definition = query
            .definition(FileId::ENTRY, zirk_ide::Position { line, character })
            .expect("transport-neutral definition query resolves the import");
        assert!(definition.path.ends_with("geometry.zrk"));
        let member_offset = text.find("user.name()").unwrap() as u32 + "user.".len() as u32;
        let member_position = frontend.sources.entry().line_column_utf16(member_offset);
        let member_definition = query
            .definition(
                FileId::ENTRY,
                IdePosition {
                    line: member_position.0,
                    character: member_position.1,
                },
            )
            .expect("member definition follows the resolved method");
        assert!(member_definition.path.ends_with("model.zrk"));

        let stdlib_offset =
            text.find("stdout.println(name)").unwrap() as u32 + "stdout.".len() as u32;
        let stdlib_position = frontend.sources.entry().line_column_utf16(stdlib_offset);
        let stdlib_definition = query
            .definition(
                FileId::ENTRY,
                IdePosition {
                    line: stdlib_position.0,
                    character: stdlib_position.1,
                },
            )
            .expect("standard-library calls navigate to the library declaration");
        assert!(stdlib_definition.path.ends_with("stdlib/std/io/stdout.zrk"));
        let module_import_offset =
            text.find("import { stdout }").unwrap() as u32 + "import { ".len() as u32;
        let module_import_position = frontend
            .sources
            .entry()
            .line_column_utf16(module_import_offset);
        let module_definition = query
            .definition(
                FileId::ENTRY,
                IdePosition {
                    line: module_import_position.0,
                    character: module_import_position.1,
                },
            )
            .expect("standard module imports navigate to their module declaration");
        assert!(
            module_definition
                .path
                .ends_with("stdlib/std/io/package.zrk"),
            "{}",
            module_definition.path
        );

        let import_names_offset = text.find("identity -> keep").unwrap() as u32;
        let import_names_position = frontend
            .sources
            .entry()
            .line_column_utf16(import_names_offset);
        let import_names = query.completions(
            FileId::ENTRY,
            IdePosition {
                line: import_names_position.0,
                character: import_names_position.1,
            },
        );
        assert!(
            import_names
                .iter()
                .any(|candidate| candidate.label == "twice"),
            "available exports: {:?}",
            import_names
                .iter()
                .map(|candidate| &candidate.label)
                .collect::<Vec<_>>()
        );
        assert!(
            !import_names
                .iter()
                .any(|candidate| candidate.label == "area")
        );
        assert!(
            !import_names
                .iter()
                .any(|candidate| candidate.label == "identity")
        );

        let module_path_offset = text.find("std.io;").unwrap() as u32 + "std.io".len() as u32;
        let module_path_position = frontend
            .sources
            .entry()
            .line_column_utf16(module_path_offset);
        let module_paths = query.completions(
            FileId::ENTRY,
            IdePosition {
                line: module_path_position.0,
                character: module_path_position.1,
            },
        );
        assert!(
            module_paths
                .iter()
                .any(|candidate| candidate.label == "stdout"),
            "module path completions: {:?}; identities: {:?}",
            module_paths
                .iter()
                .map(|candidate| &candidate.label)
                .collect::<Vec<_>>(),
            frontend
                .sources
                .files()
                .filter_map(|(_, source)| {
                    source.identity().map(|identity| {
                        (
                            identity.package.clone(),
                            identity.canonical_module_path.clone(),
                        )
                    })
                })
                .collect::<Vec<_>>()
        );

        let local_path_offset =
            text.find("./domain/model").unwrap() as u32 + "./domain/m".len() as u32;
        let local_path_position = frontend
            .sources
            .entry()
            .line_column_utf16(local_path_offset);
        let local_paths = query.completions(
            FileId::ENTRY,
            IdePosition {
                line: local_path_position.0,
                character: local_path_position.1,
            },
        );
        assert!(
            local_paths
                .iter()
                .any(|candidate| candidate.label == "./domain/model")
        );

        let visible = query.completions(FileId::ENTRY, IdePosition { line, character });
        assert!(visible.iter().any(|candidate| candidate.label == "keep"));
        assert!(
            visible.iter().any(|candidate| candidate.label == "user"),
            "visible completions: {:?}",
            visible
                .iter()
                .map(|candidate| &candidate.label)
                .collect::<Vec<_>>()
        );

        let member_completion_offset =
            text.find("user.name()").unwrap() as u32 + "user.".len() as u32;
        let member_completion_position = frontend
            .sources
            .entry()
            .line_column_utf16(member_completion_offset);
        let members = query.completions(
            FileId::ENTRY,
            IdePosition {
                line: member_completion_position.0,
                character: member_completion_position.1,
            },
        );
        assert!(members.iter().any(|candidate| candidate.label == "name"));

        let module_completion_offset =
            text.find("stdout.println(name)").unwrap() as u32 + "stdout.".len() as u32;
        let module_completion_position = frontend
            .sources
            .entry()
            .line_column_utf16(module_completion_offset);
        let module_members = query.completions(
            FileId::ENTRY,
            IdePosition {
                line: module_completion_position.0,
                character: module_completion_position.1,
            },
        );
        assert!(
            module_members
                .iter()
                .any(|candidate| candidate.label == "println")
        );
        assert!(
            !module_members
                .iter()
                .any(|candidate| candidate.label == "write")
        );
        let area_cursor = text.find("area(3)").unwrap() as u32 + "area(".len() as u32;
        let area_position = frontend.sources.entry().line_column_utf16(area_cursor);
        let signature = query
            .signature_help(
                FileId::ENTRY,
                IdePosition {
                    line: area_position.0,
                    character: area_position.1,
                },
            )
            .expect("the compiler-resolved call exposes its signature");
        assert!(signature.label.contains("area(radius: Int32): Int32"));
        assert_eq!(signature.parameters, vec!["radius: Int32"]);
        assert_eq!(signature.active_parameter, 0);
        assert!(
            query
                .references(FileId::ENTRY, zirk_ide::Position { line, character }, true,)
                .iter()
                .any(|item| item.location.path.ends_with("main.zrk"))
        );

        let alias_start =
            text.find("identity -> keep").unwrap() as u32 + "identity -> ".len() as u32;
        let alias_symbol = frontend
            .checked
            .semantic_index
            .symbol_at_position(FileId::ENTRY, alias_start)
            .expect("alias declaration has a local import identity");
        assert_eq!(alias_symbol.name, "keep");
        assert_eq!(alias_symbol.kind, zirk_sema::SemanticSymbolKind::Import);
        assert!(
            frontend
                .checked
                .semantic_index
                .reference_at_position(FileId::ENTRY, alias_start)
                .is_none(),
            "the alias token must not resolve as the exported name"
        );

        let alias_position = frontend.sources.entry().line_column_utf16(alias_start);
        let alias_edits = query
            .rename(
                FileId::ENTRY,
                IdePosition {
                    line: alias_position.0,
                    character: alias_position.1,
                },
                "lookup_user",
            )
            .expect("local alias can be renamed");
        assert_eq!(alias_edits.len(), 2, "rename edits alias and its use only");
        assert!(alias_edits.iter().all(|edit| {
            edit.location.path.ends_with("main.zrk")
                && frontend
                    .sources
                    .file(edit.location.file)
                    .slice(edit.location.span)
                    == "keep"
        }));
        assert!(matches!(
            query.rename(
                FileId::ENTRY,
                IdePosition {
                    line: alias_position.0,
                    character: alias_position.1,
                },
                "fn",
            ),
            Err(zirk_ide::RenameError::InvalidIdentifier)
        ));

        let imported_name_start = text.find("identity -> keep").unwrap() as u32;
        let imported_name_position = frontend
            .sources
            .entry()
            .line_column_utf16(imported_name_start);
        let exported_edits = query
            .rename(
                FileId::ENTRY,
                IdePosition {
                    line: imported_name_position.0,
                    character: imported_name_position.1,
                },
                "identify",
            )
            .expect("export can be renamed while preserving the local alias");
        assert!(exported_edits.iter().any(|edit| {
            edit.location.path.ends_with("geometry.zrk")
                && frontend
                    .sources
                    .file(edit.location.file)
                    .slice(edit.location.span)
                    == "identity"
        }));
        assert!(exported_edits.iter().any(|edit| {
            edit.location.path.ends_with("main.zrk")
                && frontend
                    .sources
                    .file(edit.location.file)
                    .slice(edit.location.span)
                    == "identity"
        }));
        assert!(!exported_edits.iter().any(|edit| {
            edit.location.path.ends_with("main.zrk")
                && frontend
                    .sources
                    .file(edit.location.file)
                    .slice(edit.location.span)
                    == "keep"
        }));
    }

    #[test]
    fn same_named_locals_in_separate_functions_keep_distinct_reference_sets() {
        let source = "fn first(): Int32 {\n    inmut item = 1;\n    return item;\n}\n\nfn second(): Int32 {\n    inmut item = 2;\n    return item;\n}\n\nfn main(): Void {}\n";
        let path = scratch("same-named-locals").join("same_named_locals.zrk");
        let analysis = analyze(&path, source);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let frontend = analysis
            .frontend
            .expect("valid source has a frontend result");
        let semantics = &frontend.checked.semantic_index;
        let locals = semantics
            .symbols()
            .iter()
            .filter(|symbol| {
                symbol.name == "item" && symbol.kind == zirk_sema::SemanticSymbolKind::Variable
            })
            .collect::<Vec<_>>();
        assert_eq!(locals.len(), 2, "each function contributes its own local");
        assert_ne!(locals[0].id, locals[1].id);

        let use_offsets = source
            .match_indices("return item")
            .map(|(offset, _)| (offset + "return ".len()) as u32)
            .collect::<Vec<_>>();
        assert_eq!(use_offsets.len(), 2);
        for (use_offset, expected) in use_offsets.into_iter().zip(locals) {
            let reference = semantics
                .reference_at_position(FileId::ENTRY, use_offset)
                .expect("each return expression resolves to a local");
            assert_eq!(reference.target, expected.id);
            let position = frontend.sources.entry().line_column_utf16(use_offset);
            let references = zirk_ide::IdeSnapshot::new(1, &frontend.sources, semantics)
                .references(
                    FileId::ENTRY,
                    IdePosition {
                        line: position.0,
                        character: position.1,
                    },
                    true,
                );
            assert_eq!(references.len(), 2, "only this local's declaration and use");
            assert!(references.iter().all(|item| {
                item.location.span == expected.selection_span
                    || item.location.span == reference.span
            }));
        }
    }

    #[test]
    fn closures_keep_captured_and_parameter_symbol_identities() {
        let source = "import { stdout } from std.io;\nfn main(): Void {\n    inmut outer = 5;\n    inmut g = (factor: Int32): Int32 => outer * factor;\n    stdout.println(g(3));\n}\n";
        let path = scratch("semantic-closure").join("closure.zrk");
        let analysis = analyze(&path, source);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let frontend = analysis.frontend.unwrap();
        let semantics = &frontend.checked.semantic_index;
        for name in ["outer", "factor"] {
            let declaration = source.find(name).unwrap() as u32;
            let symbol = semantics
                .symbol_at_position(FileId::ENTRY, declaration)
                .unwrap_or_else(|| panic!("missing semantic symbol for {name}"));
            assert_eq!(symbol.name, name);
            let references = semantics
                .references_to(symbol.id)
                .map(|reference| {
                    source[reference.span.start as usize..reference.span.end as usize].to_owned()
                })
                .collect::<Vec<_>>();
            assert!(references.len() >= 1, "missing resolved use for {name}");
            assert!(references.iter().all(|reference| reference == name));
            let use_offset = source.rfind(name).unwrap() as u32;
            assert_eq!(
                semantics
                    .reference_at_position(FileId::ENTRY, use_offset)
                    .unwrap()
                    .target,
                symbol.id,
                "closure use must resolve to the same symbol identity for {name}"
            );
        }
    }

    #[test]
    fn pattern_bindings_have_explicit_semantic_identity_and_uses() {
        let source = "enum Maybe { Value(value: Int32), Empty }\nfn unwrap(item: Maybe): Int32 {\n    return match item {\n        Maybe.Value(number) => number;\n        Maybe.Empty => 0;\n    };\n}\nfn main(): Void {}\n";
        let path = scratch("semantic-pattern").join("pattern.zrk");
        let analysis = analyze(&path, source);
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
        let frontend = analysis.frontend.unwrap();
        let semantics = &frontend.checked.semantic_index;
        let declaration = source.find("Value(number)").unwrap() as u32 + "Value(".len() as u32;
        let pattern = semantics
            .reference_at_position(FileId::ENTRY, declaration)
            .expect("pattern binding is explicitly recorded");
        assert_eq!(pattern.role, zirk_sema::ReferenceRole::PatternBinding);
        let symbol = semantics.symbol(pattern.target).unwrap();
        assert_eq!(symbol.name, "number");
        let use_offset = source.find("=> number").unwrap() as u32 + "=> ".len() as u32;
        let use_reference = semantics
            .reference_at_position(FileId::ENTRY, use_offset)
            .expect("pattern use resolves back to its binding");
        assert_eq!(use_reference.target, symbol.id);
    }

    #[test]
    fn semantic_fixture_reports_shadowing_and_incomplete_edits() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let shadowing = fixture.join("invalid-shadowing.zrk");
        let shadowing_text = fs::read_to_string(&shadowing).unwrap();
        let invalid = analyze(&shadowing, &shadowing_text);
        assert!(
            invalid
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("shadows a variable") })
        );

        let entry = fixture.join("src/main.zrk");
        let text = fs::read_to_string(&entry).unwrap();
        let baseline_snapshot = analyze(&entry, &text)
            .frontend
            .unwrap()
            .checked
            .semantic_index
            .snapshot_id();
        let incomplete = text.replace("keep(user.name())", "keep(user.name()");
        let edited = analyze(&entry, &incomplete);
        assert!(!edited.diagnostics.is_empty());
        if let Some(frontend) = edited.frontend {
            assert_ne!(
                frontend.checked.semantic_index.snapshot_id(),
                baseline_snapshot,
                "reanalysis publishes a new snapshot generation"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn protocol_reports_workspace_analysis_progress_when_negotiated() {
        let root = scratch("work-progress");
        let path = root.join("main.zrk");
        let uri = Url::from_file_path(&path).unwrap();
        let (mut server, socket) = service();
        let (mut requests, mut responses) = socket.split();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            use futures_util::{SinkExt, StreamExt};
            while let Some(request) = requests.next().await {
                match request.method() {
                    "window/workDoneProgress/create" => {
                        if let Some(id) = request.id().cloned() {
                            responses
                                .send(tower_lsp::jsonrpc::Response::from_ok(
                                    id,
                                    serde_json::Value::Null,
                                ))
                                .await
                                .unwrap();
                        }
                    }
                    "$/progress" => {
                        let _ = progress_tx.send(request.params().cloned().unwrap_or_default());
                    }
                    _ => {}
                }
            }
        });

        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({
                        "capabilities": {"window": {"workDoneProgress": true}}
                    }))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didOpen")
                    .params(serde_json::json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": "zirk",
                            "version": 1,
                            "text": "fn main(): Void {}\n"
                        }
                    }))
                    .finish(),
            )
            .await
            .unwrap();

        let begin = progress_rx.recv().await.unwrap();
        let end = progress_rx.recv().await.unwrap();
        assert_eq!(begin["value"]["kind"], "begin");
        assert_eq!(begin["value"]["title"], "Analyzing Zirk workspace");
        assert_eq!(end["value"]["kind"], "end");
        assert_eq!(begin["token"], end["token"]);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn protocol_cancels_an_in_flight_semantic_request() {
        let root = scratch("cancel-request");
        let path = root.join("main.zrk");
        let uri = Url::from_file_path(&path).unwrap();
        let text = (0..4_000)
            .map(|index| format!("fn value_{index}(): Int32 {{ return {index}; }}\n"))
            .collect::<String>();
        let (mut server, socket) = service();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut socket = socket;
            while socket.next().await.is_some() {}
        });
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({"capabilities": {}}))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();

        // Insert the open document without scheduling background analysis so
        // this request itself owns the expensive frontend pass being cancelled.
        server.inner().state.lock().unwrap().documents.insert(
            uri.clone(),
            Document {
                text,
                version: 1,
                analysis: None,
                semantic_fallback: None,
                published_uris: Vec::new(),
                semantic_token_history: VecDeque::new(),
            },
        );
        let request = server.ready().await.unwrap().call(
            Request::build("textDocument/definition")
                .params(serde_json::json!({
                    "textDocument": {"uri": uri},
                    "position": {"line": 0, "character": 4}
                }))
                .id(2)
                .finish(),
        );
        let cancel = server.ready().await.unwrap().call(
            Request::build("$/cancelRequest")
                .params(serde_json::json!({"id": 2}))
                .finish(),
        );
        let (request, cancel) = tokio::join!(request, cancel);
        assert!(cancel.unwrap().is_none());
        let response = request.unwrap().expect("cancelled request responds");
        assert_eq!(
            response.error().unwrap().code,
            tower_lsp::jsonrpc::ErrorCode::RequestCancelled
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn signature_help_survives_nested_and_incomplete_argument_lists() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let source = fs::read_to_string(&entry).unwrap();
        let analysis = analyze(&entry, &source);
        let frontend = analysis
            .frontend
            .as_ref()
            .expect("valid source retains checked semantic data");
        let query = IdeSnapshot::new(4, &frontend.sources, &frontend.checked.semantic_index);

        let inner_offset = source.find("user.name()").unwrap() as u32
            + "user.".len() as u32
            + "name(".len() as u32;
        let (inner_line, inner_character) =
            frontend.sources.entry().line_column_utf16(inner_offset);
        let inner = query
            .signature_help(
                FileId::ENTRY,
                IdePosition {
                    line: inner_line,
                    character: inner_character,
                },
            )
            .expect("nested method call gets its own signature");
        assert_eq!(inner.callee_name, "name");
        assert!(inner.label.contains("name(): String"));

        let incomplete = source.replace("keep(user.name())", "keep()");
        let incomplete_analysis = analyze(&entry, &incomplete);
        let incomplete_frontend = incomplete_analysis
            .frontend
            .as_ref()
            .expect("a call with a missing argument still has a parsed call node");
        let incomplete_query = IdeSnapshot::new(
            5,
            &incomplete_frontend.sources,
            &incomplete_frontend.checked.semantic_index,
        );
        let outer_offset = incomplete.find("keep()").unwrap() as u32 + "keep(".len() as u32;
        let (outer_line, outer_character) = incomplete_frontend
            .sources
            .entry()
            .line_column_utf16(outer_offset);
        let outer = incomplete_query
            .signature_help(
                FileId::ENTRY,
                IdePosition {
                    line: outer_line,
                    character: outer_character,
                },
            )
            .expect("call with a missing argument retains signature help");
        assert_eq!(outer.callee_name, "identity");
        assert_eq!(outer.active_parameter, 0);
    }

    #[test]
    fn workspace_analysis_observes_unsaved_imported_file_overlays() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let geometry = fixture.join("src/domain/geometry.zrk");
        let main_text = fs::read_to_string(&entry).unwrap();
        let geometry_text = fs::read_to_string(&geometry)
            .unwrap()
            .replace("return radius * radius * 3;", "return \"wrong\";");
        let overlays = HashMap::from([(entry, main_text), (geometry, geometry_text)]);
        let analysis = analyze_with_overlays(&fixture, &overlays);
        let frontend = analysis
            .frontend
            .as_ref()
            .expect("project analysis succeeds");
        assert!(frontend.sources.files().any(|(_, file)| {
            file.name().ends_with("geometry.zrk") && file.text().contains("return \"wrong\";")
        }));
        assert!(
            analysis.diagnostics.iter().any(|diagnostic| {
                diagnostic.cause.as_deref().is_some_and(|cause| {
                    cause.contains("the function declares `Int32` and this returns `String`")
                })
            }),
            "unexpected overlay diagnostics: {:?}",
            analysis.diagnostics
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn protocol_advertises_and_resolves_cross_module_definition_and_references() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let text = fs::read_to_string(&entry).unwrap();
        let uri = Url::from_file_path(&entry).unwrap();
        let direct =
            analyze_with_overlays(&fixture, &HashMap::from([(entry.clone(), text.clone())]));
        assert!(direct.diagnostics.is_empty(), "{:?}", direct.diagnostics);
        let direct_frontend = direct.frontend.as_ref().unwrap();
        let direct_file =
            file_id_for_uri(&direct_frontend.sources, &uri).expect("fixture URI maps to source");
        let alias_offset = text.find("keep(user").unwrap();
        let prefix = &text[..alias_offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.encode_utf16().count() as u32)
            .unwrap_or_else(|| prefix.encode_utf16().count() as u32);
        assert!(
            IdeSnapshot::new(
                1,
                &direct_frontend.sources,
                &direct_frontend.checked.semantic_index,
            )
            .definition(direct_file, IdePosition { line, character })
            .is_some()
        );

        let (mut server, socket, test_state) = service_with_state();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut socket = socket;
            while socket.next().await.is_some() {}
        });
        let initialized = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({
                        "capabilities": {
                            "workspace": {
                                "workspaceEdit": {"documentChanges": true}
                            },
                            "textDocument": {
                                "semanticTokens": {
                                    "requests": {"range": true, "full": {"delta": true}},
                                    "tokenTypes": [
                                        "namespace", "type", "class", "enum", "interface",
                                        "struct", "typeParameter", "parameter", "variable",
                                        "property", "enumMember", "function", "method", "local"
                                    ],
                                    "tokenModifiers": ["declaration", "defaultLibrary", "reference", "import"],
                                    "formats": ["relative"]
                                }
                            }
                        }
                    }))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let capabilities = &initialized.result().unwrap()["capabilities"];
        assert_eq!(capabilities["definitionProvider"], true);
        assert_eq!(capabilities["typeDefinitionProvider"], true);
        assert_eq!(capabilities["referencesProvider"], true);
        assert_eq!(capabilities["renameProvider"]["prepareProvider"], true);
        assert_eq!(capabilities["documentHighlightProvider"], true);
        assert_eq!(
            capabilities["completionProvider"]["triggerCharacters"][0],
            "."
        );
        assert_eq!(
            capabilities["signatureHelpProvider"]["triggerCharacters"][0],
            "("
        );
        assert_eq!(capabilities["completionProvider"]["resolveProvider"], true);
        assert_eq!(capabilities["workspaceSymbolProvider"], true);
        assert!(capabilities["inlayHintProvider"].is_object());
        assert_eq!(
            capabilities["semanticTokensProvider"]["full"]["delta"],
            true
        );
        assert_eq!(capabilities["semanticTokensProvider"]["range"], true);
        assert_eq!(
            capabilities["semanticTokensProvider"]["legend"]["tokenTypes"][11],
            "function"
        );

        let did_open = server.ready().await.unwrap().call(
            Request::build("textDocument/didOpen")
                .params(serde_json::json!({
                    "textDocument": {
                        "uri": uri.clone(),
                        "languageId": "zirk",
                        "version": 1,
                        "text": text
                    }
                }))
                .finish(),
        );

        // The real stdio transport runs requests concurrently. Send this
        // immediately after didOpen to verify the semantic provider waits for
        // the initial snapshot rather than returning a transient null.
        let definition = server.ready().await.unwrap().call(
            Request::build("textDocument/definition")
                .params(serde_json::json!({
                    "textDocument": {"uri": uri.clone()},
                    "position": {"line": line, "character": character}
                }))
                .id(2)
                .finish(),
        );
        let (did_open, definition) = tokio::join!(did_open, definition);
        did_open.unwrap();
        let definition = definition.unwrap().unwrap();
        let definition_result = definition.result().unwrap();
        assert!(!definition_result.is_null(), "definition returned null");
        let definition_uri = definition_result["uri"].as_str().unwrap();
        assert!(definition_uri.ends_with("geometry.zrk"), "{definition_uri}");

        let references = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/references")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": line, "character": character},
                        "context": {"includeDeclaration": true}
                    }))
                    .id(3)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let references = references.result().unwrap().as_array().unwrap();
        assert!(references.iter().any(|item| {
            item["uri"]
                .as_str()
                .is_some_and(|value| value.ends_with("geometry.zrk"))
        }));
        assert!(references.iter().any(|item| {
            item["uri"]
                .as_str()
                .is_some_and(|value| value.ends_with("main.zrk"))
        }));

        let semantic_tokens = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/semanticTokens/full")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()}
                    }))
                    .id(11)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let token_types = capabilities["semanticTokensProvider"]["legend"]["tokenTypes"]
            .as_array()
            .unwrap();
        let token_modifiers = capabilities["semanticTokensProvider"]["legend"]["tokenModifiers"]
            .as_array()
            .unwrap();
        let token_type_index = |name: &str| {
            token_types
                .iter()
                .position(|token_type| token_type.as_str() == Some(name))
                .unwrap() as u32
        };
        let modifier_bit = |name: &str| {
            1 << token_modifiers
                .iter()
                .position(|modifier| modifier.as_str() == Some(name))
                .unwrap()
        };
        let mut line = 0;
        let mut character = 0;
        let tokens = semantic_tokens.result().unwrap()["data"]
            .as_array()
            .unwrap()
            .chunks_exact(5)
            .map(|record| {
                let delta_line = record[0].as_u64().unwrap() as u32;
                let delta_start = record[1].as_u64().unwrap() as u32;
                if delta_line == 0 {
                    character += delta_start;
                } else {
                    line += delta_line;
                    character = delta_start;
                }
                (
                    line,
                    character,
                    record[2].as_u64().unwrap() as u32,
                    record[3].as_u64().unwrap() as u32,
                    record[4].as_u64().unwrap() as u32,
                )
            })
            .collect::<Vec<_>>();
        let unaliased_import_offset = text.find("stdout }").unwrap() as u32;
        let (unaliased_import_line, unaliased_import_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(unaliased_import_offset);
        assert!(
            !tokens.iter().any(|token| {
                token.0 == unaliased_import_line && token.1 == unaliased_import_character
            }),
            "an unaliased import binding keeps its TextMate import scope"
        );
        let token_for = |needle: &str| {
            let offset = text.find(needle).unwrap() as u32;
            let (line, character) = direct_frontend.sources.entry().line_column_utf16(offset);
            tokens
                .iter()
                .find(|token| token.0 == line && token.1 == character)
                .copied()
                .unwrap_or_else(|| panic!("semantic token missing for `{needle}`"))
        };
        let import_declaration_offset = text.find("keep }").unwrap() as u32;
        let (import_declaration_line, import_declaration_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(import_declaration_offset);
        assert!(
            !tokens.iter().any(|token| {
                token.0 == import_declaration_line && token.1 == import_declaration_character
            }),
            "the local import binding keeps its TextMate import scope"
        );
        let aliased_source_offset = text.find("identity -> keep").unwrap() as u32;
        let (aliased_source_line, aliased_source_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(aliased_source_offset);
        assert!(
            !tokens.iter().any(|token| {
                token.0 == aliased_source_line && token.1 == aliased_source_character
            }),
            "the exported spelling in an aliased import must keep its muted TextMate color"
        );
        let aliased_type_source_offset = text.find("Named -> NamedValue").unwrap() as u32;
        let (aliased_type_line, aliased_type_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(aliased_type_source_offset);
        assert!(
            !tokens
                .iter()
                .any(|token| { token.0 == aliased_type_line && token.1 == aliased_type_character }),
            "the exported type spelling in an aliased import must also stay muted"
        );
        let imported_type_offset = text.find("NamedValue }").unwrap() as u32;
        let (imported_type_line, imported_type_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(imported_type_offset);
        assert!(
            !tokens.iter().any(|token| {
                token.0 == imported_type_line && token.1 == imported_type_character
            }),
            "the local type import keeps its TextMate import scope"
        );
        let local_declaration = token_for("total = 0");
        assert_eq!(local_declaration.3, token_type_index("local"));
        assert_ne!(local_declaration.4 & modifier_bit("declaration"), 0);
        let function_reference = token_for("keep(user");
        assert_eq!(function_reference.3, token_type_index("function"));
        assert_ne!(function_reference.4 & modifier_bit("reference"), 0);
        assert_ne!(function_reference.4 & modifier_bit("import"), 0);

        let type_offset = text.find("value: NamedValue").unwrap() as u32 + "value: ".len() as u32;
        let (type_line, type_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(type_offset);
        let type_definition = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/typeDefinition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": type_line, "character": type_character}
                    }))
                    .id(8)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            type_definition.result().unwrap()["uri"]
                .as_str()
                .unwrap()
                .ends_with("model.zrk")
        );

        let alias_offset = text.find("keep(user").unwrap() as u32;
        let (alias_line, alias_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(alias_offset);
        let prepared_rename = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/prepareRename")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": alias_line, "character": alias_character}
                    }))
                    .id(9)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prepared_rename.result().unwrap()["placeholder"], "keep");

        let renamed = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/rename")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": alias_line, "character": alias_character},
                        "newName": "lookup_user"
                    }))
                    .id(10)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let rename_result = renamed.result().unwrap();
        assert_eq!(
            rename_result["documentChanges"][0]["textDocument"]["version"],
            1
        );
        assert_eq!(
            rename_result["documentChanges"][0]["edits"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let conflicting_rename = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/rename")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": alias_line, "character": alias_character},
                        "newName": "area"
                    }))
                    .id(11)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(conflicting_rename.result().unwrap().is_null());

        let imported_name_offset = text.find("identity -> keep").unwrap() as u32;
        let (import_line, import_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(imported_name_offset);
        let exported_rename = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/rename")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": import_line, "character": import_character},
                        "newName": "identify"
                    }))
                    .id(12)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let documents = exported_rename.result().unwrap()["documentChanges"]
            .as_array()
            .unwrap();
        assert_eq!(
            documents.len(),
            2,
            "export rename edits its declaration and import"
        );
        assert!(documents.iter().any(|document| {
            document["textDocument"]["uri"]
                .as_str()
                .is_some_and(|value| value.ends_with("geometry.zrk"))
                && document["textDocument"]["version"].is_null()
                && document["edits"]
                    .as_array()
                    .is_some_and(|edits| edits.len() == 1)
        }));
        assert!(documents.iter().any(|document| {
            document["textDocument"]["uri"]
                .as_str()
                .is_some_and(|value| value.ends_with("main.zrk"))
                && document["textDocument"]["version"] == 1
                && document["edits"]
                    .as_array()
                    .is_some_and(|edits| edits.len() == 1)
        }));

        let workspace_symbols = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("workspace/symbol")
                    .params(serde_json::json!({"query": "Named"}))
                    .id(13)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            workspace_symbols
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|symbol| symbol["name"] == "Named")
        );

        let inlay_hints = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/inlayHint")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 1000, "character": 0}
                        }
                    }))
                    .id(14)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            inlay_hints
                .result()
                .unwrap()
                .as_array()
                .is_some_and(|hints| hints.iter().any(|hint| hint["label"]
                    .as_str()
                    .is_some_and(|label| label.contains("Int32"))))
        );
        assert!(
            inlay_hints
                .result()
                .unwrap()
                .as_array()
                .is_some_and(|hints| hints.iter().any(|hint| hint["label"] == "radius:")),
            "parameter hints: {}",
            inlay_hints.result().unwrap()
        );

        let completions = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": alias_line, "character": alias_character}
                    }))
                    .id(15)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let completion_item = completions
            .result()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["label"] == "keep")
            .cloned()
            .expect("completion returns the imported alias");
        assert!(completion_item["data"].is_object());
        let resolved_completion = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("completionItem/resolve")
                    .params(completion_item)
                    .id(22)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            resolved_completion.result().unwrap()["documentation"].is_object(),
            "completion resolve enriches the original item"
        );

        let import_name_offset = text.find("identity -> keep").unwrap() as u32;
        let (import_completion_line, import_completion_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(import_name_offset);
        let import_completions = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {
                            "line": import_completion_line,
                            "character": import_completion_character
                        }
                    }))
                    .id(18)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            import_completions
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "twice")
        );

        let module_path_offset = text.find("std.io;").unwrap() as u32 + "std.io".len() as u32;
        let (module_line, module_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(module_path_offset);
        let module_completions = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": module_line, "character": module_character}
                    }))
                    .id(19)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            module_completions
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "stdout")
        );

        let member_offset = text.find("user.name()").unwrap() as u32 + "user.".len() as u32;
        let (member_line, member_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(member_offset);
        let member_completions = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": member_line, "character": member_character}
                    }))
                    .id(16)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            member_completions
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "name")
        );

        let area_cursor = text.find("area(3)").unwrap() as u32 + "area(".len() as u32;
        let (area_line, area_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(area_cursor);
        let signature_help = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/signatureHelp")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": area_line, "character": area_character}
                    }))
                    .id(17)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            signature_help.result().unwrap()["signatures"][0]["label"]
                .as_str()
                .unwrap()
                .contains("area(radius: Int32): Int32")
        );

        let generic_cursor = text.find("keep(user.name())").unwrap() as u32 + "keep(".len() as u32;
        let (generic_line, generic_character) = direct_frontend
            .sources
            .entry()
            .line_column_utf16(generic_cursor);
        let generic_signature = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/signatureHelp")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": generic_line, "character": generic_character}
                    }))
                    .id(20)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            generic_signature.result().unwrap()["signatures"][0]["label"]
                .as_str()
                .unwrap()
                .contains("identity(value: String): String"),
            "generic signature: {}",
            generic_signature.result().unwrap()
        );

        let highlights = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/documentHighlight")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": line, "character": character}
                    }))
                    .id(7)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(!highlights.result().unwrap().as_array().unwrap().is_empty());

        let tokens_response = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/semanticTokens/full")
                    .params(serde_json::json!({"textDocument": {"uri": uri.clone()}}))
                    .id(4)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let tokens = tokens_response.result().unwrap();
        assert!(
            tokens["data"]
                .as_array()
                .is_some_and(|data| !data.is_empty())
        );
        let encoded = tokens["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>();
        assert!(encoded.chunks_exact(5).any(|token| token[3] == 11));
        assert!(
            encoded
                .chunks_exact(5)
                .any(|token| token[3] == 11 && token[4] & 1 != 0)
        );
        let result_id = tokens["resultId"].as_str().unwrap();

        let snapshot_id = || {
            test_state
                .lock()
                .unwrap()
                .documents
                .get(&uri)
                .and_then(|document| document.analysis.as_ref())
                .and_then(|analysis| analysis.frontend.as_ref())
                .map(|frontend| frontend.checked.semantic_index.snapshot_id())
                .expect("the opened document has a semantic snapshot")
        };
        let cached_snapshot_id = snapshot_id();
        for id in [23, 24] {
            let hover = server
                .ready()
                .await
                .unwrap()
                .call(
                    Request::build("textDocument/hover")
                        .params(serde_json::json!({
                            "textDocument": {"uri": uri.clone()},
                            "position": {"line": line, "character": character}
                        }))
                        .id(id)
                        .finish(),
                )
                .await
                .unwrap()
                .unwrap();
            assert!(!hover.result().unwrap().is_null());
            assert_eq!(snapshot_id(), cached_snapshot_id);
        }

        for (request_id, (needle, relative_offset, expected)) in [
            ("stdout", 1, "module stdout"),
            ("identity -> keep", 0, "fn identity(value: T): T"),
            (
                "identity -> keep",
                "identity -> ".len() + 1,
                "Imported as `keep`",
            ),
            ("Named -> NamedValue", 1, "contract Named"),
            (
                "Named -> NamedValue",
                "Named -> ".len() + 1,
                "contract Named",
            ),
            (
                "Named -> NamedValue",
                "Named -> ".len() + 1,
                "Imported as `NamedValue`",
            ),
            ("User, Named", 1, "class User"),
            ("std.io", 2, "module std.io"),
            ("./domain/geometry", 3, "module \"./domain/geometry\""),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = (text.find(needle).unwrap() + relative_offset) as u32;
            let (hover_line, hover_character) =
                direct_frontend.sources.entry().line_column_utf16(offset);
            let hover = server
                .ready()
                .await
                .unwrap()
                .call(
                    Request::build("textDocument/hover")
                        .params(serde_json::json!({
                            "textDocument": {"uri": uri.clone()},
                            "position": {"line": hover_line, "character": hover_character}
                        }))
                        .id(30 + request_id as i64)
                        .finish(),
                )
                .await
                .unwrap()
                .unwrap();
            let result = hover.result().unwrap();
            let value = result["contents"]["value"]
                .as_str()
                .expect("hover returns markdown");
            assert!(
                value.contains(expected),
                "expected `{expected}` in hover response {value}"
            );
        }

        let changed_text = text.replace("stdout.println(total);", "stdout.println(user.name());");
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didChange")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone(), "version": 2},
                        "contentChanges": [{"text": changed_text}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();

        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didChange")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone(), "version": 1},
                        "contentChanges": [{"text": "fn stale(): Void {}"}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let after_stale_change = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": member_line, "character": member_character}
                    }))
                    .id(21)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            after_stale_change.result().unwrap()["uri"]
                .as_str()
                .is_some_and(|path| path.ends_with("model.zrk")),
            "a stale didChange must not replace the current snapshot"
        );
        assert_ne!(
            snapshot_id(),
            cached_snapshot_id,
            "a newer document version must publish a fresh semantic snapshot"
        );

        let delta_response = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/semanticTokens/full/delta")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "previousResultId": result_id
                    }))
                    .id(5)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            !delta_response.result().unwrap()["edits"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        let range_response = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/semanticTokens/range")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri},
                        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 100, "character": 0}}
                    }))
                    .id(6)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            range_response.result().unwrap()["data"]
                .as_array()
                .is_some_and(|data| !data.is_empty())
        );

        let malformed = text.replace("keep(user.name())", "keep(");
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didChange")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone(), "version": 3},
                        "contentChanges": [{"text": malformed}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let stale_definition = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri.clone()},
                        "position": {"line": line, "character": character}
                    }))
                    .id(8)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(stale_definition.result().unwrap().is_null());
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didClose")
                    .params(serde_json::json!({"textDocument": {"uri": uri}}))
                    .finish(),
            )
            .await
            .unwrap();
        let shutdown = server
            .ready()
            .await
            .unwrap()
            .call(Request::build("shutdown").id(9).finish())
            .await
            .unwrap()
            .unwrap();
        assert!(shutdown.result().unwrap().is_null());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn code_actions_use_current_diagnostics_and_versioned_edits() {
        let root =
            std::env::temp_dir().join(format!("zirk-lsp-code-actions-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("main.zrk");
        let text = "fn main(): Void {\n    mut value: BinaryFloat32 = 1.0;\n}\n";
        fs::write(&path, text).unwrap();
        let uri = Url::from_file_path(&path).unwrap();
        let analysis = analyze(&path, text);
        let legacy = analysis
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("BinaryFloat32"))
            .expect("legacy type spelling is diagnosed");
        let (_, diagnostic) = to_lsp_diagnostic(legacy);

        let (mut server, socket) = service();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut socket = socket;
            while socket.next().await.is_some() {}
        });
        let initialized = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({
                        "capabilities": {
                            "workspace": {
                                "workspaceEdit": {"documentChanges": true}
                            }
                        }
                    }))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            initialized.result().unwrap()["capabilities"]["codeActionProvider"]["codeActionKinds"]
                [0],
            "quickfix"
        );
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didOpen")
                    .params(serde_json::json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": "zirk",
                            "version": 7,
                            "text": text
                        }
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let actions = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/codeAction")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri},
                        "range": diagnostic.range,
                        "context": {"diagnostics": [diagnostic]}
                    }))
                    .id(2)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let action = &actions.result().unwrap()[0];
        assert_eq!(action["title"], "Replace `BinaryFloat32` with `Float32`");
        assert_eq!(
            action["edit"]["documentChanges"][0]["textDocument"]["version"],
            7
        );
        assert_eq!(
            action["edit"]["documentChanges"][0]["edits"][0]["newText"],
            "Float32"
        );

        let changed = text.replace("BinaryFloat32", "Float32");
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didChange")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri, "version": 8},
                        "contentChanges": [{"text": changed}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let stale_actions = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/codeAction")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri},
                        "range": diagnostic.range,
                        "context": {"diagnostics": [diagnostic]}
                    }))
                    .id(3)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            stale_actions
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn protocol_analysis_uses_open_unsaved_imported_documents() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic-workspace");
        let entry = fixture.join("src/main.zrk");
        let geometry = fixture.join("src/domain/geometry.zrk");
        let entry_text = fs::read_to_string(&entry).unwrap();
        let geometry_text = fs::read_to_string(&geometry).unwrap();
        let edited_geometry = geometry_text.replace("share fn area", "share fn renamed_area");
        let entry_uri = Url::from_file_path(&entry).unwrap();
        let geometry_uri = Url::from_file_path(&geometry).unwrap();
        let call_offset = entry_text.find("area(3)").unwrap();
        let prefix = &entry_text[..call_offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.encode_utf16().count() as u32)
            .unwrap_or_else(|| prefix.encode_utf16().count() as u32);

        let (mut server, socket) = service();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut socket = socket;
            while socket.next().await.is_some() {}
        });
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({
                        "capabilities": {
                            "workspace": {
                                "workspaceEdit": {"documentChanges": true}
                            }
                        }
                    }))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        for (uri, version, text) in [
            (geometry_uri.clone(), 4, edited_geometry.clone()),
            (entry_uri.clone(), 9, entry_text.clone()),
        ] {
            server
                .ready()
                .await
                .unwrap()
                .call(
                    Request::build("textDocument/didOpen")
                        .params(serde_json::json!({
                            "textDocument": {
                                "uri": uri,
                                "languageId": "zirk",
                                "version": version,
                                "text": text
                            }
                        }))
                        .finish(),
                )
                .await
                .unwrap();
        }

        let definition = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(2)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            definition.result().unwrap().is_null(),
            "definition must reflect the open dependency's unsaved export rename"
        );

        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didChange")
                    .params(serde_json::json!({
                        "textDocument": {"uri": geometry_uri, "version": 5},
                        "contentChanges": [{"text": geometry_text}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let restored_definition = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(3)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            restored_definition.result().unwrap()["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("geometry.zrk"))
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn watched_import_changes_invalidate_workspace_semantics() {
        let root = scratch("watched-import");
        let entry = root.join("main.zrk");
        let dependency = root.join("geometry.zrk");
        let entry_text = "import { area } from \"./geometry\";\n\nfn main(): Void {\n    inmut total = area(3);\n}\n";
        let dependency_text =
            "share fn area(radius: Int32): Int32 {\n    return radius * radius;\n}\n";
        fs::write(&entry, entry_text).unwrap();
        fs::write(&dependency, dependency_text).unwrap();
        let entry_uri = Url::from_file_path(&entry).unwrap();
        let dependency_uri = Url::from_file_path(&dependency).unwrap();
        let call_offset = entry_text.find("area(3)").unwrap();
        let prefix = &entry_text[..call_offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let character = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.encode_utf16().count() as u32)
            .unwrap_or_else(|| prefix.encode_utf16().count() as u32);

        let (mut server, socket) = service();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut socket = socket;
            while socket.next().await.is_some() {}
        });
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({"capabilities": {}}))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didOpen")
                    .params(serde_json::json!({
                        "textDocument": {
                            "uri": entry_uri,
                            "languageId": "zirk",
                            "version": 1,
                            "text": entry_text
                        }
                    }))
                    .finish(),
            )
            .await
            .unwrap();

        let initial = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(2)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            initial.result().unwrap()["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("geometry.zrk"))
        );

        // The filesystem-backed module index is built on demand for import
        // completion and reused until a watched source change invalidates it.
        let import_cursor = entry_text.find("./geometry").unwrap() as u32 + 2;
        let initial_imports = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": 0, "character": import_cursor}
                    }))
                    .id(7)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            initial_imports
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "geometry"),
            "initial import completions: {}",
            initial_imports.result().unwrap()
        );
        let generation_before_module_create = server
            .inner()
            .state
            .lock()
            .unwrap()
            .workspace_generations
            .get(&root)
            .copied()
            .unwrap_or(0);

        let new_module = root.join("extra.zrk");
        let new_module_uri = Url::from_file_path(&new_module).unwrap();
        fs::write(&new_module, "share fn extra(): Void {}\n").unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("workspace/didChangeWatchedFiles")
                    .params(serde_json::json!({
                        "changes": [{"uri": new_module_uri, "type": 1}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let generation_after_module_create = server
            .inner()
            .state
            .lock()
            .unwrap()
            .workspace_generations
            .get(&root)
            .copied()
            .unwrap_or(0);
        assert!(generation_after_module_create > generation_before_module_create);
        let refreshed_imports = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": 0, "character": import_cursor}
                    }))
                    .id(8)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            refreshed_imports
                .result()
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["label"] == "extra")
        );

        fs::write(
            &dependency,
            dependency_text.replace("share fn area", "share fn renamed_area"),
        )
        .unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("workspace/didChangeWatchedFiles")
                    .params(serde_json::json!({
                        "changes": [{"uri": dependency_uri, "type": 2}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let after_change = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(3)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(after_change.result().unwrap().is_null());

        fs::write(&dependency, dependency_text).unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("workspace/didChangeWatchedFiles")
                    .params(serde_json::json!({
                        "changes": [{"uri": dependency_uri, "type": 2}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let restored = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(4)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(
            restored.result().unwrap()["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("geometry.zrk"))
        );

        let manifest = root.join(".zkinit");
        let manifest_uri = Url::from_file_path(&manifest).unwrap();
        fs::write(
            &manifest,
            "project \"watch-fixture\"\nkind application\nentry \"missing.zrk\"\n",
        )
        .unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("workspace/didChangeWatchedFiles")
                    .params(serde_json::json!({
                        "changes": [{"uri": manifest_uri, "type": 1}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let after_manifest = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(5)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(after_manifest.result().unwrap().is_null());

        fs::write(
            &manifest,
            "project \"watch-fixture\"\nkind application\nentry \"main.zrk\"\n",
        )
        .unwrap();
        let lockfile = root.join("zirk.lock");
        let lockfile_uri = Url::from_file_path(&lockfile).unwrap();
        fs::write(&lockfile, "invalid lock entry\n").unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("workspace/didChangeWatchedFiles")
                    .params(serde_json::json!({
                        "changes": [{"uri": lockfile_uri, "type": 1}]
                    }))
                    .finish(),
            )
            .await
            .unwrap();
        let after_lockfile = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/definition")
                    .params(serde_json::json!({
                        "textDocument": {"uri": entry_uri},
                        "position": {"line": line, "character": character}
                    }))
                    .id(6)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(after_lockfile.result().unwrap().is_null());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn protocol_completes_unopened_module_during_an_unresolved_import() {
        let root = scratch("unresolved-import-completion");
        let entry = root.join("main.zrk");
        let modules = root.join("domain");
        fs::create_dir_all(&modules).unwrap();
        let text = "import { op } from \"./domain/un\";\nfn main(): Void {}\n";
        fs::write(&entry, text).unwrap();
        fs::write(
            modules.join("unopened.zrk"),
            "share fn open(): Void {}\nfn private_api(): Void {}\n",
        )
        .unwrap();
        let uri = Url::from_file_path(&entry).unwrap();

        let (mut server, socket) = service();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut socket = socket;
            while socket.next().await.is_some() {}
        });
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("initialize")
                    .params(serde_json::json!({"capabilities": {}}))
                    .id(1)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/didOpen")
                    .params(serde_json::json!({
                        "textDocument": {
                            "uri": uri,
                            "languageId": "zirk",
                            "version": 1,
                            "text": text
                        }
                    }))
                    .finish(),
            )
            .await
            .unwrap();

        let cursor = text.find("un\"").unwrap() as u32 + 2;
        let completion = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/completion")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri},
                        "position": {"line": 0, "character": cursor}
                    }))
                    .id(2)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        let items = completion.result().unwrap().as_array().unwrap();
        assert!(
            items
                .iter()
                .any(|item| item["label"] == "./domain/unopened"),
            "unresolved import completion: {}",
            completion.result().unwrap()
        );
        assert!(!items.iter().any(|item| item["label"] == "private_api"));

        let import_name = text.find("{ op").unwrap() as u32 + 2;
        let position = serde_json::json!({ "line": 0, "character": import_name });
        let prepare_rename = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/prepareRename")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri},
                        "position": position
                    }))
                    .id(3)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(prepare_rename.result().unwrap().is_null());
        let rename = server
            .ready()
            .await
            .unwrap()
            .call(
                Request::build("textDocument/rename")
                    .params(serde_json::json!({
                        "textDocument": {"uri": uri},
                        "position": position,
                        "newName": "open"
                    }))
                    .id(4)
                    .finish(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(rename.result().unwrap().is_null());
        let _ = fs::remove_dir_all(root);
    }
}
