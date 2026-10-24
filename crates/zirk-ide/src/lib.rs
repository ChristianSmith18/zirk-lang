//! Transport-neutral source queries built from compiler semantic snapshots.
//!
//! This crate has no editor protocol or process-I/O dependency. LSP, CLI
//! tooling, and future editor clients can share the same source locations and
//! compiler-resolved symbol identities.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use zirk_diagnostics::{FileId, SourceFile, SourceMap, Span};
use zirk_sema::{
    ReferenceRole, SemanticIndex, SemanticSymbol, SemanticSymbolKind, SemanticVisibility, SymbolId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextRange {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: FileId,
    pub path: String,
    pub span: Span,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceLocation {
    pub location: Location,
    pub role: Option<ReferenceRole>,
    pub is_declaration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameTarget {
    pub range: TextRange,
    pub placeholder: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameEdit {
    pub location: Location,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub label: String,
    pub kind: SemanticSymbolKind,
    pub detail: Option<String>,
    pub declaration: Option<Location>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHint {
    pub position: Position,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFix {
    pub title: String,
    pub range: TextRange,
    pub new_text: String,
}

/// Bounded module-path index built only from roots authorized by the
/// compiler's package loader. It indexes paths and parses only the selected
/// unopened module when import-name completion needs its exported names;
/// semantic completion for loaded modules still uses compiler-resolved facts.
#[derive(Debug, Clone, Default)]
pub struct ModuleIndex {
    local_files: Vec<PathBuf>,
    package_modules: BTreeMap<String, BTreeMap<String, PathBuf>>,
}

impl ModuleIndex {
    pub fn from_authorized_roots(
        local_root: &Path,
        package_roots: &std::collections::HashMap<String, PathBuf>,
    ) -> Self {
        const MAX_ENTRIES: usize = 32_768;
        const MAX_FILES: usize = 8_192;
        const MAX_DEPTH: usize = 32;

        fn walk(
            root: &Path,
            directory: &Path,
            package: Option<&str>,
            depth: usize,
            budget: &mut usize,
            indexed_files: &mut usize,
            files: &mut Vec<PathBuf>,
            modules: &mut BTreeMap<String, BTreeMap<String, PathBuf>>,
        ) {
            if depth > MAX_DEPTH || *budget == 0 {
                return;
            }
            let Ok(directory_entries) = std::fs::read_dir(directory) else {
                return;
            };
            let mut entries = Vec::with_capacity((*budget).min(1_024));
            for entry in directory_entries {
                if entries.len() >= *budget {
                    break;
                }
                if let Ok(entry) = entry {
                    entries.push(entry);
                }
            }
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                if *budget == 0 {
                    break;
                }
                *budget -= 1;
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    let name = entry.file_name();
                    if name.to_str().is_some_and(|name| {
                        name.starts_with('.') || matches!(name, "target" | "node_modules")
                    }) {
                        continue;
                    }
                    walk(
                        root,
                        &path,
                        package,
                        depth + 1,
                        budget,
                        indexed_files,
                        files,
                        modules,
                    );
                } else if kind.is_file()
                    && path.extension().is_some_and(|extension| extension == "zrk")
                    && *indexed_files < MAX_FILES
                {
                    *indexed_files += 1;
                    if let Some(package) = package {
                        if let Ok(relative) = path.strip_prefix(root) {
                            let module = relative
                                .with_extension("")
                                .display()
                                .to_string()
                                .replace('\\', ".")
                                .replace('/', ".");
                            modules
                                .entry(package.to_owned())
                                .or_default()
                                .insert(module, path);
                        }
                    } else {
                        files.push(path);
                    }
                }
            }
        }

        let mut index = Self::default();
        let local_root =
            std::fs::canonicalize(local_root).unwrap_or_else(|_| local_root.to_path_buf());
        let mut budget = MAX_ENTRIES;
        let mut indexed_files = 0;
        walk(
            &local_root,
            &local_root,
            None,
            0,
            &mut budget,
            &mut indexed_files,
            &mut index.local_files,
            &mut index.package_modules,
        );
        let mut package_roots = package_roots.iter().collect::<Vec<_>>();
        package_roots.sort_by(|left, right| left.0.cmp(right.0));
        for (package, root) in package_roots {
            if budget == 0 {
                break;
            }
            let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
            walk(
                &root,
                &root,
                Some(package.as_str()),
                0,
                &mut budget,
                &mut indexed_files,
                &mut index.local_files,
                &mut index.package_modules,
            );
        }
        index.local_files.sort();
        index.local_files.dedup();
        index
    }

    fn exported_names(
        &self,
        importing_file: &str,
        written_source: &str,
    ) -> Option<Vec<CompletionCandidate>> {
        let target = if written_source.len() >= 2
            && written_source.starts_with('"')
            && written_source.ends_with('"')
        {
            let path = &written_source[1..written_source.len().checked_sub(1)?];
            let importing_path = Path::new(importing_file);
            let candidate = comparable_path(
                &importing_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(path)
                    .with_extension("zrk"),
            );
            self.local_files
                .iter()
                .find(|indexed| comparable_path(indexed) == candidate)?
                .clone()
        } else {
            let (package, module) = self
                .package_modules
                .keys()
                .filter_map(|package| {
                    let suffix = written_source.strip_prefix(package)?;
                    (suffix.is_empty() || suffix.starts_with("::") || suffix.starts_with('.'))
                        .then_some((package, suffix))
                })
                .max_by_key(|(package, _)| package.len())?;
            let module = module
                .strip_prefix("::")
                .or_else(|| module.strip_prefix('.'))
                .unwrap_or(module)
                .replace("::", ".");
            let module = if module.is_empty() {
                "package"
            } else {
                &module
            };
            self.package_modules.get(package)?.get(module)?.clone()
        };

        Some(parse_exported_names(&target))
    }
}

fn parse_exported_names(path: &Path) -> Vec<CompletionCandidate> {
    use zirk_diagnostics::{DiagnosticSink, SourceFile};
    use zirk_lexer::tokenize;

    const MAX_SOURCE_BYTES: u64 = 1_048_576;
    if std::fs::canonicalize(path).ok().as_deref() != Some(path) {
        return Vec::new();
    }
    if std::fs::metadata(path)
        .ok()
        .is_none_or(|metadata| !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES)
    {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let source = SourceFile::new(path.to_string_lossy().to_string(), text);
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut diagnostics);
    let program = zirk_parser::parse(&source, &tokens, &mut diagnostics);
    let mut candidates = Vec::new();
    let mut push = |label: &str, kind| {
        candidates.push(CompletionCandidate {
            label: label.to_owned(),
            kind,
            detail: Some("exported declaration".to_owned()),
            declaration: None,
        });
    };

    for function in program.functions.iter().filter(|item| item.shared) {
        push(&function.name.name, SemanticSymbolKind::Function);
    }
    for class in program.classes.iter().filter(|item| item.shared) {
        let kind = SemanticSymbolKind::Class;
        push(&class.name.name, kind);
    }
    for contract in program.contracts.iter().filter(|item| item.shared) {
        push(&contract.name.name, SemanticSymbolKind::Contract);
    }
    for enumeration in program.enums.iter().filter(|item| item.shared) {
        push(&enumeration.name.name, SemanticSymbolKind::Enum);
    }
    for alias in program.type_aliases.iter().filter(|item| item.shared) {
        push(&alias.name.name, SemanticSymbolKind::TypeAlias);
    }
    for module in program.modules.iter().filter(|item| item.shared) {
        push(&module.name.name, SemanticSymbolKind::Module);
    }
    for module in &program.module_reexports {
        push(&module.name.name, SemanticSymbolKind::Module);
    }

    candidates.sort_by(|left, right| left.label.cmp(&right.label));
    candidates.dedup_by(|left, right| left.label == right.label && left.kind == right.kind);
    candidates
}

/// Returns a conservative, source-mapped fix for an explicitly supported
/// diagnostic. The protocol adapter is responsible for attaching the current
/// document version before exposing the edit to an editor.
pub fn diagnostic_quick_fix(
    source: &SourceFile,
    range: TextRange,
    diagnostic_code: &str,
) -> Option<CodeFix> {
    if diagnostic_code != "E0402" {
        return None;
    }
    let start = source.offset_at_utf16(range.start.line, range.start.character);
    let end = source.offset_at_utf16(range.end.line, range.end.character);
    let original = source.text().get(start as usize..end as usize)?;
    let replacement = match original {
        "BinaryFloat" => "Float",
        "BinaryFloat16" => "Float16",
        "BinaryFloat32" => "Float32",
        "BinaryFloat64" => "Float64",
        "BinaryFloat128" => "Float128",
        "Decimal16" | "Decimal32" | "Decimal64" | "Decimal128" => "Decimal",
        "Void?" => "Void",
        _ => return None,
    };
    Some(CodeFix {
        title: format!("Replace `{original}` with `{replacement}`"),
        range,
        new_text: replacement.to_owned(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub callee_name: String,
    pub label: String,
    pub parameters: Vec<String>,
    pub active_parameter: u32,
}

impl SignatureHelp {
    /// Applies compiler-resolved generic arguments to the displayed parameter
    /// and return types. Names are replaced as complete identifiers, so a
    /// parameter `T` never rewrites a type such as `Text`.
    pub fn apply_type_substitutions(
        &mut self,
        substitutions: &std::collections::HashMap<String, String>,
    ) {
        if substitutions.is_empty() {
            return;
        }
        self.label = substitute_type_names(&self.label, substitutions);
        for parameter in &mut self.parameters {
            *parameter = substitute_type_names(parameter, substitutions);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameError {
    NoSymbol,
    StandardLibrary,
    InvalidIdentifier,
    NameConflict,
}

/// Read-only queries against one analysis generation.
pub struct IdeSnapshot<'a> {
    pub version: i32,
    pub sources: &'a SourceMap,
    pub semantics: &'a SemanticIndex,
    module_index: Option<&'a ModuleIndex>,
}

impl<'a> IdeSnapshot<'a> {
    pub fn new(version: i32, sources: &'a SourceMap, semantics: &'a SemanticIndex) -> Self {
        Self {
            version,
            sources,
            semantics,
            module_index: None,
        }
    }

    pub fn with_module_index(mut self, module_index: &'a ModuleIndex) -> Self {
        self.module_index = Some(module_index);
        self
    }

    /// Resolve a cursor to a declaration, following the exact binding chosen
    /// by the compiler for references.
    pub fn symbol_at(&self, file: FileId, position: Position) -> Option<&'a SemanticSymbol> {
        let source = self.sources.file(file);
        let offset = source.offset_at_utf16(position.line, position.character);
        self.semantics
            .reference_at_position(file, offset)
            .and_then(|reference| self.semantics.symbol(reference.target))
            .or_else(|| self.semantics.symbol_at_position(file, offset))
    }

    pub fn definition(&self, file: FileId, position: Position) -> Option<Location> {
        let symbol = self.symbol_at(file, position)?;
        self.location(symbol.selection_span)
    }

    /// Returns the declaration of the written type name. Type references are
    /// resolved by the checker, so imported and generic type names do not rely
    /// on textual workspace search.
    pub fn type_definition(&self, file: FileId, position: Position) -> Option<Location> {
        let source = self.sources.file(file);
        let offset = source.offset_at_utf16(position.line, position.character);
        let reference = self.semantics.reference_at_position(file, offset)?;
        if reference.role != ReferenceRole::TypeUse {
            return None;
        }
        self.semantics
            .symbol(reference.target)
            .and_then(|symbol| self.location(symbol.selection_span))
    }

    pub fn references(
        &self,
        file: FileId,
        position: Position,
        include_declaration: bool,
    ) -> Vec<ReferenceLocation> {
        let Some(symbol) = self.symbol_at(file, position) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        if include_declaration && let Some(location) = self.location(symbol.selection_span) {
            results.push(ReferenceLocation {
                location,
                role: None,
                is_declaration: true,
            });
        }
        results.extend(
            self.semantics
                .references_to(symbol.id)
                .filter_map(|reference| {
                    self.location(reference.span)
                        .map(|location| ReferenceLocation {
                            location,
                            role: Some(reference.role),
                            is_declaration: false,
                        })
                }),
        );
        results.sort_by_key(|item| (item.location.file, item.location.span.start));
        results.dedup_by(|left, right| {
            left.location.file == right.location.file && left.location.span == right.location.span
        });
        results
    }

    pub fn document_highlights(&self, file: FileId, position: Position) -> Vec<ReferenceLocation> {
        self.references(file, position, true)
            .into_iter()
            .filter(|item| item.location.file == file)
            .collect()
    }

    pub fn type_text(&self, file: FileId, position: Position) -> Option<&str> {
        self.symbol_at(file, position)?.type_text.as_deref()
    }

    /// Returns identifiers visible at the cursor, or public members for a
    /// receiver before the trailing dot. Suggestions come only from the
    /// current file, its imported bindings, and the checked receiver type.
    pub fn completions(&self, file: FileId, position: Position) -> Vec<CompletionCandidate> {
        let source = self.sources.file(file);
        let offset = source.offset_at_utf16(position.line, position.character);
        if let Some(candidates) = self.import_completions(file, offset) {
            return candidates;
        }
        let prefix = &source.text()[..(offset as usize).min(source.text().len())];
        let receiver = receiver_before_dot(prefix);
        let mut candidates = Vec::new();

        if let Some(receiver) = receiver {
            let receiver_offset = source.text()[..offset as usize]
                .rfind(&receiver)
                .unwrap_or(0) as u32;
            let (line, character) = source.line_column_utf16(receiver_offset);
            let symbol = self
                .symbol_at(file, Position { line, character })
                .or_else(|| {
                    self.semantics.symbols().iter().find(|candidate| {
                        candidate.selection_span.file == file
                            && candidate.kind == SemanticSymbolKind::Import
                            && candidate.name == receiver
                    })
                });
            let Some(symbol) = symbol else {
                return candidates;
            };

            if symbol.kind == SemanticSymbolKind::Import {
                let modules = self
                    .semantics
                    .symbols()
                    .iter()
                    .filter(|candidate| {
                        candidate.kind == SemanticSymbolKind::Module
                            && candidate.name == symbol.name
                            && candidate.visibility == SemanticVisibility::Public
                    })
                    .collect::<Vec<_>>();
                if let [module] = modules.as_slice() {
                    let identity = self.sources.file(module.selection_span.file).identity();
                    if let Some(identity) = identity {
                        for candidate in self.semantics.symbols().iter().filter(|candidate| {
                            candidate.selection_span.file != module.selection_span.file
                                && candidate.visibility == SemanticVisibility::Public
                                && self
                                    .sources
                                    .file(candidate.selection_span.file)
                                    .identity()
                                    .is_some_and(|candidate_identity| {
                                        candidate_identity.package == identity.package
                                            && candidate_identity.canonical_module_path
                                                == module.name
                                    })
                        }) {
                            candidates.push(self.completion_candidate(candidate));
                        }
                    }
                }
            } else {
                let type_name = symbol
                    .type_text
                    .as_deref()
                    .unwrap_or(&symbol.name)
                    .trim_end_matches('?')
                    .split('<')
                    .next()
                    .unwrap_or_default();
                let type_symbol = self.semantics.symbols().iter().find(|candidate| {
                    candidate.name == type_name
                        && matches!(
                            candidate.kind,
                            SemanticSymbolKind::Class
                                | SemanticSymbolKind::Enum
                                | SemanticSymbolKind::Contract
                        )
                });
                if let Some(type_symbol) = type_symbol {
                    let owner_span = if type_symbol.kind == SemanticSymbolKind::Enum {
                        type_symbol.selection_span
                    } else {
                        type_symbol.declaration_span
                    };
                    candidates.extend(
                        self.semantics
                            .symbols()
                            .iter()
                            .filter(|candidate| {
                                candidate.owner_span == Some(owner_span)
                                    && candidate.visibility == SemanticVisibility::Public
                            })
                            .map(|candidate| self.completion_candidate(candidate)),
                    );
                }
            }
        } else {
            candidates.extend(
                self.semantics
                    .symbols()
                    .iter()
                    .filter(|symbol| {
                        if symbol.selection_span.file == file {
                            match symbol.visibility {
                                SemanticVisibility::Local => {
                                    symbol.kind == SemanticSymbolKind::Import
                                        || symbol.owner_span.is_some_and(|owner| {
                                            owner.file == file
                                                && owner.start <= offset
                                                && offset <= owner.end
                                                && symbol.selection_span.start <= offset
                                                && smallest_enclosing_block(
                                                    source,
                                                    symbol.selection_span.start,
                                                )
                                                .is_some_and(|scope| {
                                                    scope.start <= offset && offset <= scope.end
                                                })
                                        })
                                }
                                _ => true,
                            }
                        } else {
                            false
                        }
                    })
                    .map(|symbol| self.completion_candidate(symbol)),
            );
        }

        candidates.sort_by(|left, right| left.label.cmp(&right.label));
        candidates.dedup_by(|left, right| left.label == right.label && left.kind == right.kind);
        candidates
    }

    /// Whether the cursor is completing an import declaration with a source.
    /// Import-name completion can need the filesystem-backed index when its
    /// target module is not loaded; ordinary symbol/member completion stays in
    /// memory.
    pub fn completion_needs_module_index(&self, file: FileId, position: Position) -> bool {
        let source = self.sources.file(file);
        let offset = source.offset_at_utf16(position.line, position.character);
        let cursor = (offset as usize).min(source.text().len());
        let line_start = source.text()[..cursor]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_end = source.text()[cursor..]
            .find('\n')
            .map_or(source.text().len(), |index| cursor + index);
        let line = &source.text()[line_start..line_end];
        if !line.trim_start().starts_with("import") {
            return false;
        }
        find_keyword(line, "from").is_some()
    }

    fn completion_candidate(&self, symbol: &SemanticSymbol) -> CompletionCandidate {
        CompletionCandidate {
            label: symbol.name.clone(),
            kind: symbol.kind,
            detail: symbol.type_text.clone(),
            declaration: self.location(symbol.selection_span),
        }
    }

    /// Completes the source path or exported names of an import. The query is
    /// intentionally bounded to source files already admitted by the
    /// compiler's module loader; it never walks the filesystem or guesses at
    /// undeclared dependencies.
    fn import_completions(&self, file: FileId, offset: u32) -> Option<Vec<CompletionCandidate>> {
        let source = self.sources.file(file);
        let text = source.text();
        let cursor = (offset as usize).min(text.len());
        let line_start = text[..cursor].rfind('\n').map_or(0, |index| index + 1);
        let line_end = text[cursor..]
            .find('\n')
            .map_or(text.len(), |index| cursor + index);
        let line = &text[line_start..line_end];
        if !line.trim_start().starts_with("import") {
            return None;
        }

        let Some(from) = find_keyword(line, "from") else {
            return Some(Vec::new());
        };
        let cursor_in_line = cursor.saturating_sub(line_start);
        let source_start = from + "from".len();
        if cursor_in_line < source_start {
            let Some(written_source) = import_source_text(&line[source_start..]) else {
                return Some(Vec::new());
            };
            let already_imported = line[..from]
                .split_once('{')
                .map(|(_, names)| names.split('}').next().unwrap_or(names))
                .unwrap_or_default()
                .split(',')
                .filter_map(|name| name.split("->").next())
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect::<std::collections::HashSet<_>>();
            if let Some(target) = self.source_for_import(file, written_source)
                && let Some(target_identity) = self.sources.file(target).identity()
            {
                let candidates = self
                    .semantics
                    .symbols()
                    .iter()
                    .filter(|candidate| {
                        candidate.selection_span.file == target
                            && candidate.owner_span.is_none()
                            && candidate.visibility == SemanticVisibility::Public
                            && !already_imported.contains(&candidate.name)
                            && candidate.kind != SemanticSymbolKind::Import
                    })
                    .filter(|candidate| {
                        self.sources
                            .file(candidate.selection_span.file)
                            .identity()
                            .is_some_and(|identity| {
                                identity.package == target_identity.package
                                    && identity.canonical_module_path
                                        == target_identity.canonical_module_path
                            })
                    })
                    .map(|candidate| self.completion_candidate(candidate))
                    .collect();
                return Some(candidates);
            }

            let candidates = self
                .module_index
                .and_then(|index| index.exported_names(source.name(), written_source))
                .unwrap_or_default()
                .into_iter()
                .filter(|candidate| !already_imported.contains(&candidate.label))
                .collect();
            return Some(candidates);
        }

        let path_prefix = line[source_start..cursor_in_line.min(line.len())].trim();
        Some(self.module_path_completions(file, path_prefix))
    }

    fn source_for_import(&self, importing_file: FileId, written: &str) -> Option<FileId> {
        if written.starts_with('"') && written.ends_with('"') && written.len() >= 2 {
            let path = &written[1..written.len() - 1];
            let source_path = Path::new(self.sources.file(importing_file).name());
            let target = normalize_path(
                &source_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(path)
                    .with_extension("zrk"),
            );
            return self.sources.files().find_map(|(id, source)| {
                (normalize_path(Path::new(source.name())) == target).then_some(id)
            });
        }

        let mut packages = self
            .sources
            .files()
            .filter_map(|(_, source)| source.identity().map(|identity| identity.package.clone()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|package| written == package || written.starts_with(&format!("{package}::")))
            .collect::<Vec<_>>();
        packages.sort_by_key(|package| std::cmp::Reverse(package.len()));
        let package = packages.first()?;
        let module_path = written
            .strip_prefix(package)
            .unwrap_or_default()
            .strip_prefix("::")
            .unwrap_or_default()
            .replace("::", ".");
        self.sources.files().find_map(|(id, source)| {
            let identity = source.identity()?;
            let expected_path = if module_path.is_empty() {
                "package"
            } else {
                module_path.as_str()
            };
            (identity.package == *package && identity.canonical_module_path == expected_path)
                .then_some(id)
        })
    }

    fn module_path_completions(
        &self,
        importing_file: FileId,
        path_prefix: &str,
    ) -> Vec<CompletionCandidate> {
        if path_prefix.starts_with('"') {
            return self.local_path_completions(importing_file, path_prefix);
        }

        let mut candidates = Vec::new();
        let mut packages = self
            .sources
            .files()
            .filter_map(|(_, source)| source.identity().map(|identity| identity.package.clone()))
            .collect::<std::collections::BTreeSet<_>>();
        if let Some(index) = self.module_index {
            packages.extend(index.package_modules.keys().cloned());
        }
        let prefix = path_prefix.trim();
        if let Some((package, suffix)) = packages
            .iter()
            .filter_map(|package| {
                let suffix = prefix.strip_prefix(package)?;
                (suffix.is_empty() || suffix.starts_with("::") || suffix.starts_with('.'))
                    .then_some((package.as_str(), suffix))
            })
            .max_by_key(|(package, _)| package.len())
        {
            let suffix = suffix
                .strip_prefix("::")
                .or_else(|| suffix.strip_prefix('.'))
                .unwrap_or(suffix)
                .replace("::", ".");
            let mut modules = BTreeSet::new();
            if let Some(index) = self.module_index
                && let Some(package_modules) = index.package_modules.get(package)
            {
                modules.extend(
                    package_modules
                        .keys()
                        .filter(|module| module.as_str() != "package")
                        .cloned(),
                );
            }
            for (_, source) in self.sources.files() {
                if let Some(identity) = source.identity()
                    && identity.package == package
                    && identity.canonical_module_path != "package"
                {
                    modules.insert(identity.canonical_module_path.clone());
                }
            }
            for relative in modules {
                let suffix_components = suffix.split('.').filter(|part| !part.is_empty()).count();
                let suffix_is_complete = suffix.ends_with('.');
                let component_index = if suffix_is_complete {
                    suffix_components
                } else {
                    suffix_components.saturating_sub(1)
                };
                let label = relative.split('.').nth(component_index).unwrap_or_default();
                let suffix_prefix = if suffix.is_empty() || suffix_is_complete {
                    ""
                } else {
                    suffix.rsplit('.').next().unwrap_or_default()
                };
                if !label.is_empty()
                    && label.starts_with(suffix_prefix)
                    && !candidates
                        .iter()
                        .any(|candidate: &CompletionCandidate| candidate.label == label)
                {
                    candidates.push(CompletionCandidate {
                        label: label.to_owned(),
                        kind: SemanticSymbolKind::Module,
                        detail: Some(format!("{package}::{label}")),
                        declaration: None,
                    });
                }
            }
        } else {
            for package in packages {
                if let Some(next) = package_path_segment(&package, prefix) {
                    if !candidates.iter().any(|candidate| candidate.label == next) {
                        candidates.push(CompletionCandidate {
                            label: next,
                            kind: SemanticSymbolKind::Module,
                            detail: Some(package),
                            declaration: None,
                        });
                    }
                }
            }
        }
        candidates.sort_by(|left, right| left.label.cmp(&right.label));
        candidates
    }

    fn local_path_completions(
        &self,
        importing_file: FileId,
        path_prefix: &str,
    ) -> Vec<CompletionCandidate> {
        let quote = path_prefix.starts_with('"');
        let partial = path_prefix.trim_start_matches('"').replace('\\', "/");
        let current = Path::new(self.sources.file(importing_file).name());
        let parent = current.parent().unwrap_or_else(|| Path::new("."));
        let partial_path = Path::new(&partial);
        let directory = partial_path.parent().unwrap_or_else(|| Path::new(""));
        let filename_prefix = partial_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let target_directory = comparable_path(&parent.join(directory));
        let mut candidates = Vec::new();
        let mut files = self
            .sources
            .files()
            .map(|(_, candidate)| PathBuf::from(candidate.name()))
            .collect::<BTreeSet<_>>();
        if let Some(index) = self.module_index {
            files.extend(index.local_files.iter().cloned());
        }
        for candidate_path in files {
            if comparable_path(candidate_path.parent().unwrap_or_else(|| Path::new(".")))
                != target_directory
            {
                continue;
            }
            let Some(stem) = candidate_path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if stem.starts_with(filename_prefix) {
                let label = if quote {
                    let directory = directory.to_string_lossy().replace('\\', "/");
                    if directory.is_empty() {
                        stem.to_owned()
                    } else if directory == "." {
                        format!("./{stem}")
                    } else {
                        format!("{}/{stem}", directory.trim_end_matches('/'))
                    }
                } else {
                    stem.to_owned()
                };
                candidates.push(CompletionCandidate {
                    label,
                    kind: SemanticSymbolKind::Module,
                    detail: Some("local module".into()),
                    declaration: None,
                });
            }
        }
        candidates.sort_by(|left, right| left.label.cmp(&right.label));
        candidates.dedup_by(|left, right| left.label == right.label);
        candidates
    }

    pub fn signature_help(&self, file: FileId, position: Position) -> Option<SignatureHelp> {
        use zirk_diagnostics::DiagnosticSink;
        use zirk_lexer::{TokenKind, tokenize};

        let source = self.sources.file(file);
        let offset = source.offset_at_utf16(position.line, position.character);
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(source, &mut sink);
        let mut open_parens = Vec::new();
        for (index, token) in tokens.iter().enumerate() {
            if token.span.start >= offset {
                break;
            }
            match token.kind {
                TokenKind::LParen => open_parens.push(index),
                TokenKind::RParen => {
                    open_parens.pop();
                }
                _ => {}
            }
        }
        let open_index = *open_parens.last()?;
        let callee_token = tokens[..open_index]
            .iter()
            .rev()
            .find(|token| token.span.end <= tokens[open_index].span.start)?;
        if !matches!(callee_token.kind, TokenKind::Identifier(_)) {
            return None;
        }
        let (line, character) = source.line_column_utf16(callee_token.span.start);
        let symbol = self.symbol_at(file, Position { line, character })?;
        let type_text = symbol.type_text.as_deref()?;
        let parameters_text = type_text.strip_prefix("fn(")?.split_once(')')?.0;
        let parameters = split_signature_parameters(parameters_text);
        let label = match symbol.kind {
            SemanticSymbolKind::Method => {
                format!("{}{}", symbol.name, &type_text[2..])
            }
            _ => format!("fn {}{}", symbol.name, &type_text[2..]),
        };

        let mut active_parameter = 0u32;
        let mut nested_parens = 0u32;
        let mut nested_brackets = 0u32;
        let mut nested_braces = 0u32;
        for token in tokens
            .iter()
            .skip(open_index + 1)
            .take_while(|token| token.span.start < offset)
        {
            match token.kind {
                TokenKind::LParen => nested_parens += 1,
                TokenKind::RParen => nested_parens = nested_parens.saturating_sub(1),
                TokenKind::LBracket => nested_brackets += 1,
                TokenKind::RBracket => nested_brackets = nested_brackets.saturating_sub(1),
                TokenKind::LBrace => nested_braces += 1,
                TokenKind::RBrace => nested_braces = nested_braces.saturating_sub(1),
                TokenKind::Comma
                    if nested_parens == 0 && nested_brackets == 0 && nested_braces == 0 =>
                {
                    active_parameter += 1;
                }
                _ => {}
            }
        }
        if !parameters.is_empty() {
            active_parameter = active_parameter.min(parameters.len() as u32 - 1);
        }
        Some(SignatureHelp {
            callee_name: symbol.name.clone(),
            label,
            parameters,
            active_parameter,
        })
    }

    /// Returns parameter-name hints for positional call arguments. Calls are
    /// matched through compiler-resolved identities and checked signatures;
    /// named arguments, unresolved calls, and variadic signatures are omitted.
    pub fn parameter_hints(&self, file: FileId, range: TextRange) -> Vec<ParameterHint> {
        use zirk_diagnostics::DiagnosticSink;
        use zirk_lexer::{TokenKind, tokenize};

        let source = self.sources.file(file);
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(source, &mut sink);
        let mut hints = Vec::new();
        for (callee_index, token) in tokens.iter().enumerate() {
            if !matches!(token.kind, TokenKind::Identifier(_))
                || !matches!(
                    tokens.get(callee_index + 1).map(|token| &token.kind),
                    Some(TokenKind::LParen)
                )
            {
                continue;
            }
            let Some(reference) = self.semantics.reference_at_position(file, token.span.start)
            else {
                continue;
            };
            if !matches!(
                reference.role,
                ReferenceRole::Call | ReferenceRole::MemberCall
            ) {
                continue;
            }
            let Some(symbol) = self.semantics.symbol(reference.target) else {
                continue;
            };
            if !matches!(
                symbol.kind,
                SemanticSymbolKind::Function | SemanticSymbolKind::Method
            ) {
                continue;
            }
            let Some(type_text) = symbol.type_text.as_deref() else {
                continue;
            };
            let Some(parameters_text) = type_text
                .strip_prefix("fn(")
                .and_then(|text| balanced_signature_end(text).map(|end| &text[..end]))
            else {
                continue;
            };
            let parameters = split_signature_parameters(parameters_text)
                .into_iter()
                .map(|parameter| {
                    let (name, _) = parameter.split_once(':')?;
                    let name = name.trim().trim_start_matches("...");
                    (!name.is_empty() && !parameter.trim_start().starts_with("..."))
                        .then(|| name.to_owned())
                })
                .collect::<Vec<_>>();
            if parameters.is_empty() || parameters.iter().any(Option::is_none) {
                continue;
            }
            let parameter_names = parameters.into_iter().flatten().collect::<Vec<_>>();
            let Some(open_index) = callee_index.checked_add(1) else {
                continue;
            };
            let argument_starts = top_level_argument_starts(&tokens, open_index);
            let mut consumed = std::collections::HashSet::new();
            let mut next_positional = 0usize;
            for argument_start in argument_starts {
                let token = &tokens[argument_start];
                if let (TokenKind::Identifier(name), Some(TokenKind::Colon)) = (
                    &token.kind,
                    tokens.get(argument_start + 1).map(|token| &token.kind),
                ) {
                    consumed.insert(name.as_str());
                    continue;
                }
                while next_positional < parameter_names.len()
                    && consumed.contains(parameter_names[next_positional].as_str())
                {
                    next_positional += 1;
                }
                let Some(parameter) = parameter_names.get(next_positional) else {
                    break;
                };
                next_positional += 1;
                let (line, character) = source.line_column_utf16(token.span.start);
                let position = Position { line, character };
                if position < range.start || position > range.end {
                    continue;
                }
                hints.push(ParameterHint {
                    position,
                    label: format!("{parameter}:"),
                });
            }
        }
        hints.sort_by_key(|hint| hint.position);
        hints.dedup_by(|left, right| left.position == right.position && left.label == right.label);
        hints
    }

    pub fn prepare_rename(&self, file: FileId, position: Position) -> Option<RenameTarget> {
        let (symbol, selected_span) = self.rename_subject_at(file, position)?;
        if symbol.is_standard_library {
            return None;
        }
        Some(RenameTarget {
            range: self.location(selected_span)?.range,
            placeholder: symbol.name.clone(),
        })
    }

    /// Builds an all-or-nothing rename from the compiler's resolved identity
    /// graph. The caller must reanalyze the resulting workspace edit before
    /// applying it, so capture or newly introduced diagnostics are rejected.
    pub fn rename(
        &self,
        file: FileId,
        position: Position,
        new_name: &str,
    ) -> Result<Vec<RenameEdit>, RenameError> {
        if !is_identifier(new_name) {
            return Err(RenameError::InvalidIdentifier);
        }
        let (symbol, _) = self
            .rename_subject_at(file, position)
            .ok_or(RenameError::NoSymbol)?;
        if symbol.is_standard_library {
            return Err(RenameError::StandardLibrary);
        }
        if self.has_name_conflict(
            symbol.id,
            symbol.owner_span,
            symbol.selection_span.file,
            new_name,
        ) {
            return Err(RenameError::NameConflict);
        }

        let mut spans = vec![symbol.selection_span];
        spans.extend(
            self.semantics
                .rename_references_to(symbol.id)
                .map(|reference| reference.span),
        );
        spans.sort_by_key(|span| (span.file, span.start, span.end));
        spans.dedup();
        spans
            .into_iter()
            .map(|span| {
                self.location(span)
                    .map(|location| RenameEdit {
                        location,
                        new_text: new_name.to_owned(),
                    })
                    .ok_or(RenameError::NoSymbol)
            })
            .collect()
    }

    fn rename_subject_at(
        &self,
        file: FileId,
        position: Position,
    ) -> Option<(&'a SemanticSymbol, Span)> {
        let source = self.sources.file(file);
        let offset = source.offset_at_utf16(position.line, position.character);
        if let Some(reference) = self.semantics.reference_at_position(file, offset)
            && let Some(alias) = reference.rename_target
            && let Some(symbol) = self.semantics.symbol(alias)
        {
            return Some((symbol, reference.span));
        }
        let symbol = self.symbol_at(file, position)?;
        Some((symbol, symbol.selection_span))
    }

    fn has_name_conflict(
        &self,
        target: SymbolId,
        owner: Option<Span>,
        file: FileId,
        new_name: &str,
    ) -> bool {
        self.semantics.symbols().iter().any(|candidate| {
            if candidate.id == target
                || candidate.name != new_name
                || candidate.selection_span.file != file
            {
                return false;
            }
            match (owner, candidate.owner_span) {
                (Some(target_owner), Some(candidate_owner)) => target_owner == candidate_owner,
                (None, None) => true,
                _ => false,
            }
        })
    }

    pub fn location(&self, span: Span) -> Option<Location> {
        let source = self.sources.files().find(|(id, _)| *id == span.file)?.1;
        let (start_line, start_character) = source.line_column_utf16(span.start);
        let (end_line, end_character) = source.line_column_utf16(span.end);
        Some(Location {
            file: span.file,
            path: source.name().to_owned(),
            span,
            range: TextRange {
                start: Position {
                    line: start_line,
                    character: start_character,
                },
                end: Position {
                    line: end_line,
                    character: end_character,
                },
            },
        })
    }

    pub fn position_at_offset(&self, file: FileId, offset: u32) -> Option<Position> {
        let source = self.sources.files().find(|(id, _)| *id == file)?.1;
        let (line, character) = source.line_column_utf16(offset);
        Some(Position { line, character })
    }
}

fn find_keyword(text: &str, keyword: &str) -> Option<usize> {
    text.match_indices(keyword).find_map(|(index, _)| {
        let before_is_boundary = text[..index]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_alphanumeric() && character != '_');
        let end = index + keyword.len();
        let after_is_boundary = text[end..]
            .chars()
            .next()
            .is_none_or(|character| !character.is_alphanumeric() && character != '_');
        (before_is_boundary && after_is_boundary).then_some(index)
    })
}

fn import_source_text(text: &str) -> Option<&str> {
    let text = text.trim_start();
    if text.starts_with('"') {
        let end = text[1..].find('"')? + 2;
        return Some(&text[..end]);
    }
    let end = text
        .find(|character: char| character.is_whitespace() || character == ';')
        .unwrap_or(text.len());
    (end > 0).then_some(&text[..end])
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn comparable_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| normalize_path(path))
}

fn package_path_segment(package: &str, prefix: &str) -> Option<String> {
    let suffix = package.strip_prefix(prefix)?;
    if suffix.is_empty() {
        return None;
    }
    if !prefix.ends_with('.') && !prefix.ends_with("::") {
        return Some(package.to_owned());
    }
    let suffix = suffix.trim_start_matches('.');
    let suffix = suffix.trim_start_matches(':');
    Some(
        suffix
            .split(['.', ':'])
            .next()
            .unwrap_or_default()
            .to_owned(),
    )
}

fn receiver_before_dot(prefix: &str) -> Option<String> {
    let before_dot = prefix.trim_end().strip_suffix('.')?.trim_end();
    let start = before_dot
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_alphanumeric() && *ch != '_')
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let receiver = &before_dot[start..];
    (!receiver.is_empty()).then(|| receiver.to_owned())
}

fn smallest_enclosing_block(source: &zirk_diagnostics::SourceFile, offset: u32) -> Option<Span> {
    use zirk_diagnostics::DiagnosticSink;
    use zirk_lexer::{TokenKind, tokenize};

    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let mut braces = Vec::new();
    let mut enclosing = Vec::new();
    for token in tokens {
        match token.kind {
            TokenKind::LBrace => braces.push(token.span.start),
            TokenKind::RBrace => {
                if let Some(start) = braces.pop()
                    && start <= offset
                    && offset <= token.span.end
                {
                    enclosing.push(source.span(start, token.span.end));
                }
            }
            _ => {}
        }
    }
    enclosing.into_iter().min_by_key(|span| span.len())
}

fn split_signature_parameters(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut generic_depth = 0u32;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut brace_depth = 0u32;
    for (index, ch) in text.char_indices() {
        match ch {
            '<' => generic_depth += 1,
            '>' => generic_depth = generic_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if generic_depth == 0
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0 =>
            {
                let part = text[start..index].trim();
                if !part.is_empty() {
                    result.push(part.to_owned());
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let part = text[start..].trim();
    if !part.is_empty() {
        result.push(part.to_owned());
    }
    result
}

fn balanced_signature_end(text: &str) -> Option<usize> {
    let mut depth = 0u32;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(index),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn top_level_argument_starts(tokens: &[zirk_lexer::Token], open_index: usize) -> Vec<usize> {
    use zirk_lexer::TokenKind;

    let mut starts = Vec::new();
    let mut current = None;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut brace_depth = 0u32;
    for index in (open_index + 1)..tokens.len() {
        match tokens[index].kind {
            TokenKind::RParen if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                if let Some(start) = current {
                    starts.push(start);
                }
                break;
            }
            TokenKind::Comma if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                if let Some(start) = current.take() {
                    starts.push(start);
                }
            }
            TokenKind::LParen => {
                current.get_or_insert(index);
                paren_depth += 1;
            }
            TokenKind::RParen => {
                paren_depth = paren_depth.saturating_sub(1);
                current.get_or_insert(index);
            }
            TokenKind::LBracket => {
                current.get_or_insert(index);
                bracket_depth += 1;
            }
            TokenKind::RBracket => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.get_or_insert(index);
            }
            TokenKind::LBrace => {
                current.get_or_insert(index);
                brace_depth += 1;
            }
            TokenKind::RBrace => {
                brace_depth = brace_depth.saturating_sub(1);
                current.get_or_insert(index);
            }
            TokenKind::Eof => break,
            _ => {
                current.get_or_insert(index);
            }
        }
    }
    starts
}

fn substitute_type_names(
    text: &str,
    substitutions: &std::collections::HashMap<String, String>,
) -> String {
    let mut output = String::with_capacity(text.len());
    let mut identifier = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            identifier.push(character);
        } else {
            if !identifier.is_empty() {
                output.push_str(
                    substitutions
                        .get(&identifier)
                        .map(String::as_str)
                        .unwrap_or(&identifier),
                );
                identifier.clear();
            }
            output.push(character);
        }
    }
    if !identifier.is_empty() {
        output.push_str(
            substitutions
                .get(&identifier)
                .map(String::as_str)
                .unwrap_or(&identifier),
        );
    }
    output
}

/// Returns whether the compiler lexer sees exactly one ordinary identifier.
pub fn is_identifier(name: &str) -> bool {
    use zirk_diagnostics::{DiagnosticSink, SourceFile};
    use zirk_lexer::{TokenKind, tokenize};

    let source = SourceFile::new("<identifier>", name);
    let mut diagnostics = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut diagnostics);
    diagnostics.diagnostics().is_empty()
        && matches!(tokens.as_slice(), [token, eof]
            if matches!(&token.kind, TokenKind::Identifier(identifier) if identifier == name)
                && matches!(eof.kind, TokenKind::Eof))
}

#[cfg(test)]
mod tests {
    use super::{
        IdeSnapshot, ModuleIndex, Position, TextRange, comparable_path, diagnostic_quick_fix,
        is_identifier,
    };
    use zirk_diagnostics::{SourceFile, SourceMap};
    use zirk_sema::SemanticIndex;

    #[test]
    fn rename_names_follow_the_language_lexer() {
        assert!(is_identifier("customer_id"));
        assert!(is_identifier("café"));
        assert!(!is_identifier("fn"));
        assert!(!is_identifier("two words"));
        assert!(!is_identifier("name!"));
        assert!(!is_identifier(""));
    }

    #[test]
    fn ide_positions_and_ranges_use_utf16_columns() {
        let mut sources = SourceMap::new();
        let text = "😀name";
        let file = sources.add(SourceFile::new("unicode.zrk", text));
        let start = text.find("name").unwrap() as u32;
        let semantics = SemanticIndex::default();
        let snapshot = IdeSnapshot::new(1, &sources, &semantics);

        assert_eq!(
            snapshot.position_at_offset(file, start),
            Some(Position {
                line: 0,
                character: 2
            })
        );
        let location = snapshot
            .location(zirk_diagnostics::Span::in_file(file, start, start + 4))
            .unwrap();
        assert_eq!(location.range.start.character, 2);
        assert_eq!(location.range.end.character, 6);
    }

    #[test]
    fn diagnostic_quick_fixes_are_exact_and_utf16_mapped() {
        let cases = [
            ("BinaryFloat", "Float"),
            ("BinaryFloat16", "Float16"),
            ("BinaryFloat32", "Float32"),
            ("BinaryFloat64", "Float64"),
            ("BinaryFloat128", "Float128"),
            ("Decimal16", "Decimal"),
            ("Decimal32", "Decimal"),
            ("Decimal64", "Decimal"),
            ("Decimal128", "Decimal"),
            ("Void?", "Void"),
        ];
        let text = format!(
            "// 😀\n{}",
            cases
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join("\n")
        );
        let mut sources = SourceMap::new();
        let file = sources.add(SourceFile::new("quick-fixes.zrk", text));
        let source = sources.file(file);

        for (index, (original, replacement)) in cases.iter().enumerate() {
            let start = source.text().find(original).unwrap() as u32;
            let (line, character) = source.line_column_utf16(start);
            let (end_line, end_character) = source.line_column_utf16(start + original.len() as u32);
            let fix = diagnostic_quick_fix(
                source,
                TextRange {
                    start: Position { line, character },
                    end: Position {
                        line: end_line,
                        character: end_character,
                    },
                },
                "E0402",
            )
            .unwrap();
            assert_eq!(fix.new_text, *replacement, "case {index}");
            assert_eq!(
                fix.title,
                format!("Replace `{original}` with `{replacement}`")
            );
            assert_eq!(fix.range.start.line, index as u32 + 1);
        }
        assert!(
            diagnostic_quick_fix(
                source,
                TextRange {
                    start: Position {
                        line: 0,
                        character: 0
                    },
                    end: Position {
                        line: 0,
                        character: 1
                    },
                },
                "E0000",
            )
            .is_none()
        );
    }

    #[test]
    fn import_path_completion_indexes_only_authorized_local_and_package_roots() {
        use std::collections::HashMap;

        let root = std::env::temp_dir().join(format!(
            "zirk-ide-module-index-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let local_root = root.join("project");
        let local_modules = local_root.join("src/domain");
        let package_root = root.join("dependency/src");
        std::fs::create_dir_all(&local_modules).unwrap();
        std::fs::create_dir_all(package_root.join("http")).unwrap();
        std::fs::write(
            local_modules.join("unopened.zrk"),
            "share fn open(): Void {}\nfn private_api(): Void {}\n",
        )
        .unwrap();
        std::fs::write(
            package_root.join("http/server.zrk"),
            "share fn start(): Void {}\n",
        )
        .unwrap();
        std::fs::write(root.join("unauthorized.zrk"), "").unwrap();

        let packages = HashMap::from([("acme.net".to_owned(), package_root)]);
        let index = ModuleIndex::from_authorized_roots(&local_root, &packages);
        assert!(index.package_modules.contains_key("acme.net"));
        assert!(!index.package_modules.contains_key("rogue.package"));
        assert!(
            !index
                .local_files
                .contains(&comparable_path(&root.join("unauthorized.zrk")))
        );

        let cases = [
            (
                local_root.join("src/main.zrk"),
                "import { value } from \"./domain/un\";",
                "./domain/unopened",
            ),
            (
                local_root.join("src/main.zrk"),
                "import { value } from acme.net::http::se;",
                "server",
            ),
        ];
        for (path, text, expected) in cases {
            let cursor = if text.contains("\"./domain/un\"") {
                text.find("un\"").unwrap() + 2
            } else {
                text.find("se;").unwrap() + 2
            } as u32;
            let source = SourceFile::new(path.to_string_lossy().to_string(), text);
            let mut sources = SourceMap::new();
            let file = sources.add(source);
            let (line, character) = sources.file(file).line_column_utf16(cursor);
            let semantics = SemanticIndex::default();
            let snapshot = IdeSnapshot::new(1, &sources, &semantics).with_module_index(&index);

            assert!(snapshot.completion_needs_module_index(file, Position { line, character }));
            let candidates = snapshot.completions(file, Position { line, character });
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.label == expected),
                "expected {expected:?} in {:?}; indexed files: {:?}; source: {}",
                candidates
                    .iter()
                    .map(|candidate| &candidate.label)
                    .collect::<Vec<_>>(),
                index.local_files,
                path.display()
            );
        }

        let export_cases = [
            (
                local_root.join("src/main.zrk"),
                "import { op } from \"./domain/unopened\";",
                "open",
            ),
            (
                local_root.join("src/main.zrk"),
                "import { st } from acme.net::http::server;",
                "start",
            ),
        ];
        for (path, text, expected) in export_cases {
            let cursor = text
                .find("op }")
                .map(|offset| offset + 2)
                .or_else(|| text.find("st }").map(|offset| offset + 2))
                .unwrap() as u32;
            let mut sources = SourceMap::new();
            let file = sources.add(SourceFile::new(path.to_string_lossy().to_string(), text));
            let (line, character) = sources.file(file).line_column_utf16(cursor);
            let semantics = SemanticIndex::default();
            let snapshot = IdeSnapshot::new(1, &sources, &semantics).with_module_index(&index);

            assert!(snapshot.completion_needs_module_index(file, Position { line, character }));
            let candidates = snapshot.completions(file, Position { line, character });
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.label == expected),
                "expected exported name {expected:?} in {:?}",
                candidates
                    .iter()
                    .map(|candidate| &candidate.label)
                    .collect::<Vec<_>>()
            );
            if expected == "open" {
                assert!(
                    !candidates
                        .iter()
                        .any(|candidate| candidate.label == "private_api")
                );
            }
        }

        let ordinary_source = SourceFile::new(
            local_root
                .join("src/main.zrk")
                .to_string_lossy()
                .to_string(),
            "fn main(): Void {}",
        );
        let mut sources = SourceMap::new();
        let file = sources.add(ordinary_source);
        let (line, character) = sources.file(file).line_column_utf16(5);
        let semantics = SemanticIndex::default();
        assert!(
            !IdeSnapshot::new(1, &sources, &semantics)
                .completion_needs_module_index(file, Position { line, character })
        );

        std::fs::remove_dir_all(root).unwrap();
    }
}
