//! Loading the files of a crate.
//!
//! Module resolution is split in two: reading files and following `import`
//! happens here, because it is I/O and belongs with the driver; deciding what
//! each name resolves to and whether it is visible happens in `zirk-sema`,
//! because it is name resolution.
//!
//! The graph is walked from the entry file rather than from a directory
//! listing: a `.zrk` nobody imports is not part of the crate, and compiling it
//! by accident would report errors in code the program never uses.
//!
//! # Mutual imports are allowed
//!
//! Two files that import from each other are a normal program, not an error.
//! `import` brings names into scope and nothing in this phase depends on the
//! order files are read: signatures are collected before any body is checked.
//! What the loader does need is to read each file once, which is what the
//! visited set gives it — walking a cycle forever is the real hazard, and
//! rejecting the program was never the fix for it.

use crate::{
    codes,
    project::{self, ProjectManifest},
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use zirk_ast::{Ident, ImportDecl, ImportName, ImportSource, Program};
use zirk_diagnostics::{Diagnostic, DiagnosticSink, FileId, SourceFile, SourceMap, Span};

/// The parsed files of one crate.
pub struct Crate {
    pub sources: SourceMap,
    /// One parsed program per file, in the order the files were loaded.
    pub units: Vec<Unit>,
    /// Files named only as `.zkinit` custom-global type sources contribute
    /// declarations to semantic analysis, but their file-scope statements do
    /// not become runtime module initializers.
    pub type_only_files: HashSet<FileId>,
    /// Filesystem roots the loader authorized for this module graph. Editor
    /// indexes may inspect these roots, but must not broaden them.
    pub module_roots: ModuleRoots,
    /// The validated application/package model that owns this graph, if the
    /// entry was discovered through a project manifest.
    pub project: Option<ProjectContext>,
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub manifest: ProjectManifest,
    pub root: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct ModuleRoots {
    pub local_root: PathBuf,
    /// Package name to the directory containing its entry module.
    pub packages: HashMap<String, PathBuf>,
}

pub struct Unit {
    pub program: Program,
}

/// Reads and parses the entry file and everything it imports.
pub fn load(entry: &Path, sink: &mut DiagnosticSink) -> Option<Crate> {
    load_with_overlay(entry, &HashMap::new(), sink)
}

/// Loads the declared entrypoint of the closest project manifest. This is the
/// package-aware entry point used by project commands; single-file loading
/// remains available for scripts and editor overlays.
pub fn load_project(start: &Path, sink: &mut DiagnosticSink) -> Option<Crate> {
    load_project_with_overlay(start, &HashMap::new(), sink)
}

/// Loads the declared project entrypoint while letting editor buffers shadow
/// their on-disk files. The project manifest and dependency graph still come
/// from disk; only source text is overlaid.
pub fn load_project_with_overlay(
    start: &Path,
    overlay: &HashMap<PathBuf, String>,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    load_project_with_overlay_and_development(start, overlay, false, sink)
}

/// Loads an explicit source entry using the nearest project's dependency and
/// capability context. The entry must remain inside that project root.
pub fn load_project_entry(start: &Path, entry: &Path, sink: &mut DiagnosticSink) -> Option<Crate> {
    load_project_entry_with_overlay(start, entry, &HashMap::new(), sink)
}

pub fn load_project_entry_with_overlay(
    start: &Path,
    entry: &Path,
    overlay: &HashMap<PathBuf, String>,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    load_project_graph(start, Some(entry), overlay, false, sink)
}

/// Loads a project graph that includes the current application's direct
/// `dev_dependencies`. Development tools such as the future test runner use
/// this entry point; package development dependencies remain private.
pub fn load_project_for_development(start: &Path, sink: &mut DiagnosticSink) -> Option<Crate> {
    load_project_with_overlay_and_development(start, &HashMap::new(), true, sink)
}

pub fn load_project_with_overlay_and_development(
    start: &Path,
    overlay: &HashMap<PathBuf, String>,
    include_dev_dependencies: bool,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    load_project_graph(start, None, overlay, include_dev_dependencies, sink)
}

fn load_project_graph(
    start: &Path,
    entry_override: Option<&Path>,
    overlay: &HashMap<PathBuf, String>,
    include_dev_dependencies: bool,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    let overlay = normalize_overlay(overlay);
    let overlay_root = if start.file_name().is_some_and(|name| name == ".zkinit") {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    let manifest_path = project::discover(start).or_else(|| {
        overlay
            .keys()
            .find(|path| {
                path.file_name().is_some_and(|name| name == ".zkinit")
                    && (path.parent() == Some(overlay_root)
                        || path.parent().is_some_and(|parent| {
                            absolute_normalize(parent) == absolute_normalize(overlay_root)
                        }))
            })
            .cloned()
    })?;
    let manifest_source = absolute_normalize(&manifest_path);
    let manifest_result = overlay
        .get(&manifest_source)
        .map(|source| project::parse_manifest(source))
        .unwrap_or_else(|| project::read_manifest(&manifest_path));
    let manifest = match manifest_result {
        Ok(manifest) => manifest,
        Err(error) => {
            sink.emit(Diagnostic::error(codes::INVALID_PROJECT, error.to_string()));
            return None;
        }
    };
    let root = manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if let Err(error) = project::validate_global_type_sources(root, &manifest) {
        sink.emit(Diagnostic::error(codes::INVALID_PROJECT, error.to_string()));
        return None;
    }
    let lock_path = root.join("zirk.lock");
    let lock = if lock_path.exists() {
        match project::read_lockfile(&lock_path) {
            Ok(lock) => Some(lock),
            Err(error) => {
                sink.emit(Diagnostic::error(codes::INVALID_PROJECT, error.to_string()));
                return None;
            }
        }
    } else {
        None
    };
    if let Err(error) = project::validate_lockfile_with_dev(
        root,
        &manifest,
        lock.as_ref(),
        include_dev_dependencies,
    ) {
        sink.emit(Diagnostic::error(codes::INVALID_PROJECT, error.to_string()));
        return None;
    }
    let empty_lock = project::Lockfile {
        packages: std::collections::BTreeMap::new(),
    };
    let entry = entry_override
        .map(absolute_normalize)
        .unwrap_or_else(|| root.join(&manifest.entry));
    load_with_project(
        &entry,
        &manifest,
        lock.as_ref().unwrap_or(&empty_lock),
        root,
        &overlay,
        include_dev_dependencies,
        sink,
    )
}

/// Like [`load`], but files present in `overlay` are read from the map's text
/// instead of the filesystem.
///
/// This is what the language server feeds with the editor's in-memory text:
/// the open document is keyed by its filesystem path, so imports still
/// resolve relative to its directory while the entry's own bytes are the ones
/// the user is typing, saved or not.
pub fn load_with_overlay(
    entry: &Path,
    overlay: &HashMap<PathBuf, String>,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    // Keys are normalized once so callers may pass paths in any shape while
    // lookups always see the normalized form `Loader::load` produces.
    let overlay = normalize_overlay(overlay);

    let entry = absolute_normalize(entry);
    let local_root = entry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let package_roots = standard_package_roots();
    let mut loader = Loader {
        sources: SourceMap::new(),
        units: Vec::new(),
        by_path: HashMap::new(),
        loading_stack: Vec::new(),
        overlay: &overlay,
        package_roots: package_roots.clone(),
        local_root: local_root.clone(),
        sink,
    };

    loader.load(&entry, None, &loader.local_root.clone())?;

    Some(Crate {
        sources: loader.sources,
        units: loader.units,
        type_only_files: HashSet::new(),
        module_roots: ModuleRoots {
            local_root,
            packages: package_directories(&package_roots),
        },
        project: None,
    })
}

fn normalize_overlay(overlay: &HashMap<PathBuf, String>) -> HashMap<PathBuf, String> {
    overlay
        .iter()
        .map(|(path, text)| (absolute_normalize(path), text.clone()))
        .collect()
}

fn load_with_project(
    entry: &Path,
    manifest: &ProjectManifest,
    lock: &project::Lockfile,
    root: &Path,
    overlay: &HashMap<PathBuf, String>,
    include_dev_dependencies: bool,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    let resolved_package_roots: HashMap<String, PathBuf> =
        match project::resolve_package_roots_with_dev(
            root,
            manifest,
            lock,
            include_dev_dependencies,
        ) {
            Ok(roots) => roots
                .into_iter()
                .map(|(name, entry)| {
                    let entry = absolute_normalize(&entry);
                    let entry = std::fs::canonicalize(&entry).unwrap_or(entry);
                    (name, entry)
                })
                .collect(),
            Err(error) => {
                sink.emit(Diagnostic::error(
                    codes::PACKAGE_RESOLUTION,
                    error.to_string(),
                ));
                return None;
            }
        };
    let mut package_roots = standard_package_roots();
    package_roots.extend(resolved_package_roots);
    let local_root = absolute_normalize(root);
    let mut loader = Loader {
        sources: SourceMap::new(),
        units: Vec::new(),
        by_path: HashMap::new(),
        loading_stack: Vec::new(),
        overlay,
        package_roots: package_roots.clone(),
        local_root: local_root.clone(),
        sink,
    };
    loader.load(entry, None, &loader.local_root.clone())?;
    let type_sources = manifest
        .globals
        .values()
        .filter_map(|global| global.from.as_ref())
        .map(|source| root.join(source))
        .collect::<Vec<_>>();
    let mut type_only_files = HashSet::new();
    for source in type_sources {
        let loaded_before = loader.by_path.values().copied().collect::<HashSet<_>>();
        loader.load(&source, None, &loader.local_root.clone())?;
        for file in loader.by_path.values() {
            if !loaded_before.contains(file) {
                type_only_files.insert(*file);
            }
        }
    }
    Some(Crate {
        sources: loader.sources,
        units: loader.units,
        type_only_files,
        module_roots: ModuleRoots {
            local_root,
            packages: package_directories(&package_roots),
        },
        project: Some(ProjectContext {
            manifest: manifest.clone(),
            root: absolute_normalize(root),
        }),
    })
}

fn package_directories(entries: &HashMap<String, PathBuf>) -> HashMap<String, PathBuf> {
    entries
        .iter()
        .filter_map(|(name, entry)| Some((name.clone(), entry.parent()?.to_path_buf())))
        .collect()
}

/// Compiler-owned package roots are ordinary Zirk source trees bundled with
/// the toolchain. They enter the same graph as a user package, but their
/// logical identity is fixed and their trusted host bindings remain governed
/// by `zirk_sema::stdlib` rather than source syntax.
fn standard_package_roots() -> HashMap<String, PathBuf> {
    [
        "io",
        "environment",
        "http",
        "json",
        "net",
        "path",
        "fs",
        "collections",
        "time",
    ]
    .into_iter()
    .map(|package| {
        let entry = absolute_normalize(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../stdlib/std/{package}/package.zrk")),
        );
        (
            format!("std.{package}"),
            std::fs::canonicalize(&entry).unwrap_or(entry),
        )
    })
    .collect()
}

/// Bundled source stays available to the browser/WASM frontend without a
/// filesystem read. Native loading uses the same text, so the package graph is
/// identical across targets.
fn standard_source(path: &Path) -> Option<&'static str> {
    let directory =
        absolute_normalize(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std"));
    let relative = path
        .strip_prefix(&directory)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    match relative.as_str() {
        "io/package.zrk" => Some(include_str!("../../../stdlib/std/io/package.zrk")),
        "io/stdin.zrk" => Some(include_str!("../../../stdlib/std/io/stdin.zrk")),
        "io/stdout.zrk" => Some(include_str!("../../../stdlib/std/io/stdout.zrk")),
        "io/stderr.zrk" => Some(include_str!("../../../stdlib/std/io/stderr.zrk")),
        "environment/package.zrk" => {
            Some(include_str!("../../../stdlib/std/environment/package.zrk"))
        }
        "environment/environment.zrk" => Some(include_str!(
            "../../../stdlib/std/environment/environment.zrk"
        )),
        "http/package.zrk" => Some(include_str!("../../../stdlib/std/http/package.zrk")),
        "http/http.zrk" => Some(include_str!("../../../stdlib/std/http/http.zrk")),
        "json/package.zrk" => Some(include_str!("../../../stdlib/std/json/package.zrk")),
        "json/json.zrk" => Some(include_str!("../../../stdlib/std/json/json.zrk")),
        "net/package.zrk" => Some(include_str!("../../../stdlib/std/net/package.zrk")),
        "net/tcp.zrk" => Some(include_str!("../../../stdlib/std/net/tcp.zrk")),
        "net/dns.zrk" => Some(include_str!("../../../stdlib/std/net/dns.zrk")),
        "net/address.zrk" => Some(include_str!("../../../stdlib/std/net/address.zrk")),
        "net/tls.zrk" => Some(include_str!("../../../stdlib/std/net/tls.zrk")),
        "net/udp.zrk" => Some(include_str!("../../../stdlib/std/net/udp.zrk")),
        "net/local.zrk" => Some(include_str!("../../../stdlib/std/net/local.zrk")),
        "net/interfaces.zrk" => Some(include_str!("../../../stdlib/std/net/interfaces.zrk")),
        "path/package.zrk" => Some(include_str!("../../../stdlib/std/path/package.zrk")),
        "path/path.zrk" => Some(include_str!("../../../stdlib/std/path/path.zrk")),
        "fs/package.zrk" => Some(include_str!("../../../stdlib/std/fs/package.zrk")),
        "fs/fs.zrk" => Some(include_str!("../../../stdlib/std/fs/fs.zrk")),
        "collections/package.zrk" => {
            Some(include_str!("../../../stdlib/std/collections/package.zrk"))
        }
        "time/package.zrk" => Some(include_str!("../../../stdlib/std/time/package.zrk")),
        _ => None,
    }
}

struct Loader<'a> {
    sources: SourceMap,
    units: Vec<Unit>,
    /// Files already loaded, so a diamond of imports reads each one once.
    by_path: HashMap<PathBuf, FileId>,
    /// Active depth-first import path. It distinguishes executable
    /// initialization cycles from harmless declaration-only cycles.
    loading_stack: Vec<LoadingEntry>,
    /// In-memory text that shadows the filesystem, keyed by normalized path.
    overlay: &'a HashMap<PathBuf, String>,
    /// Logical package roots resolved from the project manifest. Non-local
    /// transports intentionally do not become filesystem imports here.
    package_roots: HashMap<String, PathBuf>,
    /// Filesystem boundary for the local project (canonicalized for each check).
    local_root: PathBuf,
    sink: &'a mut DiagnosticSink,
}

struct LoadingEntry {
    path: PathBuf,
    has_executable_body: bool,
}

impl Loader<'_> {
    /// Loads one file and, depth first, everything it imports.
    ///
    /// `from` is the span of the `import` that asked for it, so a missing file
    /// is reported where it was requested rather than at the top of nowhere.
    fn load(
        &mut self,
        path: &Path,
        from: Option<(FileId, zirk_diagnostics::Span)>,
        authorized_root: &Path,
    ) -> Option<()> {
        let path = absolute_normalize(path);
        let (path, identity_path) = self.authorize_path(&path, authorized_root, from)?;

        if let Some(cycle_start) = self
            .loading_stack
            .iter()
            .position(|entry| entry.path == identity_path)
        {
            let cycle = &self.loading_stack[cycle_start..];
            if cycle.iter().any(|entry| entry.has_executable_body) {
                let mut paths: Vec<_> = cycle
                    .iter()
                    .map(|entry| entry.path.display().to_string())
                    .collect();
                paths.push(identity_path.display().to_string());
                let message = format!(
                    "executable module initialization cycle: {}",
                    paths.join(" -> ")
                );
                let mut diagnostic = Diagnostic::error(codes::IMPORT_CYCLE, message)
                    .with_cause("at least one file in this import cycle has file-scope statements")
                    .with_help("break the cycle or move shared declarations into a declaration-only module");
                if let Some((file, span)) = from {
                    let span = zirk_diagnostics::Span::in_file(file, span.start, span.end);
                    diagnostic = diagnostic
                        .at(self.sources.location(span))
                        .with_snippet(self.sources.snippet(span));
                }
                self.sink.emit(diagnostic);
            }
            return Some(());
        }

        if self.by_path.contains_key(&identity_path) {
            return Some(());
        }

        let overlaid = self.overlay.get(&path).cloned();
        let read = match overlaid.or_else(|| standard_source(&path).map(str::to_owned)) {
            Some(text) => Ok(text),
            None => std::fs::read_to_string(&path),
        };
        let Ok(text) = read else {
            let mut diagnostic = Diagnostic::error(
                codes::UNREADABLE_FILE,
                format!("could not read `{}`", path.display()),
            )
            .with_cause("the file does not exist or is not readable");

            if let Some((file, span)) = from {
                let span = zirk_diagnostics::Span::in_file(file, span.start, span.end);
                diagnostic = diagnostic
                    .at(self.sources.location(span))
                    .with_snippet(self.sources.snippet(span))
                    .with_help("check the path written in the import");
            } else {
                diagnostic = diagnostic.with_help("check the path and its permissions");
            }

            self.sink.emit(diagnostic);
            return None;
        };

        let (package, package_root) = self
            .package_roots
            .iter()
            .find_map(|(name, entry)| {
                let root = entry.parent()?;
                identity_path
                    .starts_with(root)
                    .then(|| (name.clone(), root.to_path_buf()))
            })
            .unwrap_or_else(|| ("local".into(), self.local_root.clone()));
        let canonical_module_path = identity_path
            .strip_prefix(&package_root)
            .unwrap_or(&identity_path)
            .with_extension("")
            .display()
            .to_string()
            .replace('/', ".");
        let mut source = SourceFile::new(path.display().to_string(), text);
        source.set_identity(zirk_diagnostics::SourceIdentity {
            package,
            canonical_module_path,
            written_path: path.display().to_string(),
            package_root: package_root.display().to_string(),
        });
        let file = self.sources.add(source);
        self.by_path.insert(identity_path.clone(), file);

        let tokens = zirk_lexer::tokenize(self.sources.file(file), self.sink);
        let mut program = zirk_parser::parse(self.sources.file(file), &tokens, self.sink);
        let derives_json_codec = program.classes.iter().any(|class| {
            class.implements.iter().any(|implemented| {
                implemented.name == "JSONCodec"
                    && program.imports.iter().any(|import| {
                        matches!(&import.source, ImportSource::Package { path, .. } if path == "std.json")
                            && import.names.iter().any(|name| name.name.name == "JSONCodec")
                    })
            })
        });
        let has_json_binding = program.imports.iter().any(|import| {
            import
                .names
                .iter()
                .any(|name| name.bound_name().name == "JSON")
        });
        if derives_json_codec && !has_json_binding {
            let end = program.span.end;
            let span = Span::in_file(file, end, end);
            program.imports.push(ImportDecl {
                names: vec![ImportName {
                    name: Ident::new("JSON", span),
                    alias: None,
                    span,
                }],
                source: ImportSource::Package {
                    path: "std.json".into(),
                    span,
                },
                span,
            });
        }
        self.loading_stack.push(LoadingEntry {
            path: identity_path.clone(),
            has_executable_body: !program.body.is_empty(),
        });

        // Imports are followed even when the file had errors: reporting every
        // file's problems in one run beats one file per invocation.
        let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let import_root = PathBuf::from(
            self.sources
                .file(file)
                .identity()
                .expect("loader assigned source identity")
                .package_root
                .clone(),
        );

        for import in &program.imports {
            match &import.source {
                ImportSource::Local {
                    path: written,
                    span,
                } => {
                    self.load(
                        &directory.join(format!("{written}.zrk")),
                        Some((file, *span)),
                        &import_root,
                    );
                }
                ImportSource::Package { path, span } => {
                    let package = path.split("::").next().unwrap_or(path).to_string();
                    if zirk_sema::stdlib::standard_module(path).is_some() {
                        if let Some(root) = self.package_roots.get(&package).cloned() {
                            let package_root = root.parent().unwrap_or_else(|| Path::new("."));
                            self.load(&root, Some((file, *span)), package_root);
                        }
                        continue;
                    }
                    if package.starts_with("std.") {
                        self.package_error(
                            file,
                            *span,
                            format!(
                                "standard package module `{path}` is not provided by this toolchain"
                            ),
                        );
                        continue;
                    }
                    if let Some(root) = self.package_roots.get(&package).cloned() {
                        let package_root = root.parent().unwrap_or_else(|| Path::new("."));
                        self.load(&root, Some((file, *span)), package_root);
                    } else {
                        self.package_error(
                            file,
                            *span,
                            format!("package `{package}` is not declared by this project"),
                        );
                    }
                }
            }
        }
        for reexport in &program.module_reexports.clone() {
            let target = directory.join(format!("{}.zrk", reexport.source_path));
            self.load(&target, Some((file, reexport.span)), &import_root);
            let target = self.authorized_identity_path(&target);
            if let Some(target) = target
                && let Some(target_file) = self.by_path.get(&target)
                && let Some(target_program) = self
                    .units
                    .iter()
                    .find(|unit| unit.program.span.file == *target_file)
                    .map(|unit| unit.program.clone())
            {
                program.modules.push(zirk_ast::ModuleDecl {
                    name: reexport.name.clone(),
                    shared: true,
                    body: Box::new(target_program),
                    span: reexport.span,
                });
            }
        }

        self.loading_stack.pop();
        self.units.push(Unit { program });

        Some(())
    }

    /// Returns the normalized source path and canonical identity key only when
    /// the path is lexically contained and its existing target stays inside
    /// the same authorized root after symlink resolution.
    fn authorize_path(
        &mut self,
        path: &Path,
        authorized_root: &Path,
        from: Option<(FileId, zirk_diagnostics::Span)>,
    ) -> Option<(PathBuf, PathBuf)> {
        let lexical = absolute_normalize(path);
        let lexical_root = absolute_normalize(authorized_root);
        let canonical_root = std::fs::canonicalize(&lexical_root).ok();
        let Some(canonical_root) = canonical_root.filter(|_| lexical.starts_with(&lexical_root))
        else {
            self.outside_root(path, from);
            return None;
        };

        match std::fs::canonicalize(&lexical) {
            Ok(canonical) if canonical.starts_with(&canonical_root) => Some((lexical, canonical)),
            Ok(_) => {
                self.outside_root(path, from);
                None
            }
            Err(_) => {
                // An unsaved overlay entry may not exist, but its existing
                // parent must still resolve within the authorized root.
                if self.overlay.contains_key(&lexical) {
                    let parent = lexical.parent().unwrap_or(&canonical_root);
                    match std::fs::canonicalize(parent) {
                        Ok(canonical_parent) if canonical_parent.starts_with(&canonical_root) => {
                            Some((lexical.clone(), lexical))
                        }
                        _ => {
                            self.outside_root(path, from);
                            None
                        }
                    }
                } else {
                    // Preserve the normal missing/unreadable-file diagnostic.
                    Some((lexical.clone(), lexical))
                }
            }
        }
    }

    fn authorized_identity_path(&self, path: &Path) -> Option<PathBuf> {
        std::fs::canonicalize(absolute_normalize(path)).ok()
    }

    fn outside_root(&mut self, path: &Path, from: Option<(FileId, zirk_diagnostics::Span)>) {
        let mut diagnostic = Diagnostic::error(
            codes::IMPORT_OUTSIDE_ROOT,
            format!(
                "source path `{}` escapes its authorized root",
                path.display()
            ),
        )
        .with_help("keep the source within the project or declared package root");
        if let Some((file, span)) = from {
            let span = zirk_diagnostics::Span::in_file(file, span.start, span.end);
            diagnostic = diagnostic
                .at(self.sources.location(span))
                .with_snippet(self.sources.snippet(span));
        }
        self.sink.emit(diagnostic);
    }

    fn package_error(&mut self, file: FileId, span: zirk_diagnostics::Span, message: String) {
        let span = zirk_diagnostics::Span::in_file(file, span.start, span.end);
        self.sink.emit(
            Diagnostic::error(codes::PACKAGE_RESOLUTION, message)
                .at(self.sources.location(span))
                .with_snippet(self.sources.snippet(span))
                .with_help("declare the dependency in `.zkinit`, or use a quoted relative import"),
        );
    }
}

/// Makes a path absolute and removes `.` and `..` without touching the filesystem.
fn absolute_normalize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut result = PathBuf::new();

    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Writes `text` to a fresh file under `dir` and returns its path.
    fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, text).unwrap();
        path
    }

    /// A scratch directory unique to this test process.
    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zirk-modules-test-{}-{}", std::process::id(), tag));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn local_network_source_is_bundled_with_the_standard_package() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std/net/local.zrk");
        let source = standard_source(&absolute_normalize(&path)).unwrap();
        assert!(source.contains("package std.net.local;"));
        assert!(source.contains("share class LocalStream"));
        assert!(source.contains("share class LocalListener"));
        assert!(zirk_sema::stdlib::standard_module("std.net::local").is_some());
    }

    #[test]
    fn network_interfaces_source_is_bundled_with_the_standard_package() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std/net/interfaces.zrk");
        let source = standard_source(&absolute_normalize(&path)).unwrap();
        assert!(source.contains("package std.net.interfaces;"));
        assert!(source.contains("share class NetworkInterfaceAddress"));
        assert!(source.contains("network.inspect.interfaces"));
        assert!(zirk_sema::stdlib::standard_module("std.net::interfaces").is_some());
    }

    #[test]
    fn filesystem_and_path_packages_are_bundled_and_resolve_from_official_roots() {
        let std_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std");
        let path_source = standard_source(&absolute_normalize(&std_root.join("path/path.zrk")))
            .expect("bundled std.path source");
        let fs_source = standard_source(&absolute_normalize(&std_root.join("fs/fs.zrk")))
            .expect("bundled std.fs source");
        assert!(path_source.contains("share class Path"));
        assert!(fs_source.contains("share class File implements Resource<FileResourceError>"));
        assert_eq!(standard_package_roots().len(), 9);
        assert!(zirk_sema::stdlib::standard_module("std.collections").is_some());
        assert!(zirk_sema::stdlib::standard_module("std.path::path").is_some());
        assert!(zirk_sema::stdlib::standard_module("std.fs::fs").is_some());
        assert!(zirk_sema::stdlib::standard_module("std.time").is_some());

        let dir = scratch("bundled-filesystem-import");
        let entry = write(
            &dir,
            "main.zrk",
            "import { File, OpenOptions } from std.fs::fs;\nimport { Path } from std.path::path;\nmut result = File.read_text(Path.of(\"input.txt\"), 1024);\n",
        );
        let mut sink = DiagnosticSink::new();
        let krate = load(&entry, &mut sink).expect("the standard packages are resolvable");
        assert!(
            !sink.has_errors(),
            "{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );
        assert!(krate.sources.files().any(|(_, source)| {
            source
                .identity()
                .is_some_and(|identity| identity.package == "std.fs")
        }));
        assert!(krate.sources.files().any(|(_, source)| {
            source
                .identity()
                .is_some_and(|identity| identity.package == "std.path")
        }));

        let mut check_sink = DiagnosticSink::new();
        assert!(
            crate::frontend::run_frontend(&entry, &mut check_sink).is_some(),
            "{}",
            check_sink.render(zirk_diagnostics::RenderStyle::Human)
        );
        assert!(
            !check_sink.has_errors(),
            "{}",
            check_sink.render(zirk_diagnostics::RenderStyle::Human)
        );
    }

    #[test]
    fn collections_package_is_bundled_and_exports_the_canonical_builtin_aliases() {
        let std_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std");
        let source = standard_source(&absolute_normalize(
            &std_root.join("collections/package.zrk"),
        ))
        .expect("bundled std.collections source");

        for declaration in [
            "share type Array<T> = Array<T>;",
            "share type List<T> = List<T>;",
            "share type Map<K, V> = Map<K, V>;",
            "share type Set<T> = Set<T>;",
            "share type Range<T> = Range<T>;",
            "share type Deque<T> = Deque<T>;",
            "share type Queue<T> = Queue<T>;",
            "share type Stack<T> = Stack<T>;",
            "share type PriorityQueue<T> = PriorityQueue<T>;",
        ] {
            assert!(source.contains(declaration), "missing `{declaration}`");
        }

        let dir = scratch("bundled-collections-import");
        let entry = write(
            &dir,
            "main.zrk",
            "import { List } from std.collections;\nmut values: List<Int32> = List();\n",
        );
        let mut sink = DiagnosticSink::new();
        let krate = load(&entry, &mut sink).expect("std.collections resolves");
        assert!(
            !sink.has_errors(),
            "{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );
        assert!(krate.sources.files().any(|(_, source)| {
            source
                .name()
                .replace('\\', "/")
                .ends_with("/stdlib/std/collections/package.zrk")
        }));
    }

    #[test]
    fn filesystem_capability_effects_flow_through_direct_indirect_and_callback_calls() {
        let dir = scratch("filesystem-capability-effects");
        let entry = write(
            &dir,
            "main.zrk",
            "import { File } from std.fs::fs;\nimport { Path } from std.path::path;\nfn read_file(): Void { File.read_text(Path.of(\"input.txt\"), 1024); }\nfn read_file_async(): Void { File.read_text_async(Path.of(\"input.txt\"), 1024); }\nfn inspect_file(): Void { File.metadata_for(Path.of(\"input.txt\")); }\nfn invoke(callback: Fn(Int32) => Void): Void { callback(1); }\nfn callback_wrapper(): Void { invoke((value: Int32): Void => { File.read_text(Path.of(\"callback.txt\"), 1024); }); }\nfn main(): Void { read_file(); read_file_async(); inspect_file(); callback_wrapper(); }\n",
        );
        let mut sink = DiagnosticSink::new();
        let frontend = crate::frontend::run_frontend(&entry, &mut sink).unwrap_or_else(|| {
            panic!(
                "valid filesystem program: {}",
                sink.render(zirk_diagnostics::RenderStyle::Human)
            )
        });
        assert!(
            !sink.has_errors(),
            "{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );

        let read_effect = std::collections::BTreeSet::from(["filesystem.read".to_owned()]);
        for function_name in ["read_file", "callback_wrapper"] {
            let function = frontend
                .program
                .functions
                .iter()
                .find(|function| function.name.name == function_name)
                .expect("function declaration");
            assert_eq!(
                frontend
                    .checked
                    .callable_capability_effects
                    .get(&function.name.span),
                Some(&read_effect),
                "capability summary for {function_name}"
            );
        }
        let async_function = frontend
            .program
            .functions
            .iter()
            .find(|function| function.name.name == "read_file_async")
            .expect("async filesystem read declaration");
        assert!(
            frontend
                .checked
                .suspend_summaries
                .get(&async_function.name.span)
                .is_some_and(|effects| effects.may_suspend),
            "async filesystem effects must be retained in callable summaries"
        );
        let metadata_function = frontend
            .program
            .functions
            .iter()
            .find(|function| function.name.name == "inspect_file")
            .expect("metadata inspection declaration");
        let metadata_effect = std::collections::BTreeSet::from(["filesystem.metadata".to_owned()]);
        assert_eq!(
            frontend
                .checked
                .callable_capability_effects
                .get(&metadata_function.name.span),
            Some(&metadata_effect),
            "metadata operation has its own capability"
        );
        let main_function = frontend
            .program
            .functions
            .iter()
            .find(|function| function.name.name == "main")
            .expect("main declaration");
        assert_eq!(
            frontend
                .checked
                .callable_capability_effects
                .get(&main_function.name.span),
            Some(&std::collections::BTreeSet::from([
                "filesystem.metadata".to_owned(),
                "filesystem.read".to_owned(),
            ])),
            "direct and indirect effects are combined"
        );
        let native_read = frontend
            .checked
            .standard_filesystem_calls
            .iter()
            .find_map(|(&span, binding)| (binding == "std.fs.File.read_text").then_some(span))
            .expect("filesystem read call");
        assert_eq!(
            frontend.checked.protected_api_effects.get(&native_read),
            Some(&"filesystem.read".to_owned())
        );
        let native_metadata = frontend
            .checked
            .standard_filesystem_calls
            .iter()
            .find_map(|(&span, binding)| (binding == "std.fs.File.metadata_for").then_some(span))
            .expect("filesystem metadata call");
        assert_eq!(
            frontend.checked.protected_api_effects.get(&native_metadata),
            Some(&"filesystem.metadata".to_owned())
        );
        let app_requester =
            frontend
                .checked
                .protected_api_requesters
                .iter()
                .find_map(|(span, requester)| {
                    (requester.as_str() == "local"
                        && frontend
                            .sources
                            .file(span.file)
                            .identity()
                            .is_some_and(|identity| identity.package == "local"))
                    .then_some(requester.as_str())
                });
        assert_eq!(app_requester, Some("local"));
    }

    #[test]
    fn the_overlay_shadows_the_entry_text() {
        let dir = scratch("overlay-entry");
        let entry = write(&dir, "main.zrk", "fn on_disk() { return 1; }\n");

        let mut overlay = HashMap::new();
        overlay.insert(entry.clone(), "fn in_memory() { return 2; }\n".to_string());

        let mut sink = DiagnosticSink::new();
        let krate = load_with_overlay(&entry, &overlay, &mut sink).unwrap();

        let text = krate.sources.entry().text();
        assert!(text.contains("in_memory"));
        assert!(!text.contains("on_disk"));
        assert!(!sink.has_errors());
    }

    #[test]
    fn imports_still_resolve_from_disk() {
        let dir = scratch("overlay-import");
        write(&dir, "lib.zrk", "fn helper() { return 7; }\n");
        let entry = dir.join("main.zrk"); // never written: only in the overlay

        let mut overlay = HashMap::new();
        overlay.insert(
            entry.clone(),
            "import { helper } from \"./lib\";\nfn main() { helper(); }\n".to_string(),
        );

        let mut sink = DiagnosticSink::new();
        let krate = load_with_overlay(&entry, &overlay, &mut sink).unwrap();

        assert_eq!(krate.units.len(), 2);
        assert!(!sink.has_errors());
    }

    #[test]
    fn rejects_parent_traversal_outside_standalone_root() {
        let dir = scratch("outside-parent");
        let source_dir = dir.join("src");
        fs::create_dir_all(&source_dir).unwrap();
        write(&dir, "outside.zrk", "fn secret() { }\n");
        let entry = write(
            &source_dir,
            "main.zrk",
            "import { secret } from \"../outside\";\n",
        );

        let mut sink = DiagnosticSink::new();
        let krate = load(&entry, &mut sink).unwrap();

        assert_eq!(krate.units.len(), 1, "outside source must not be loaded");
        assert!(sink.has_errors());
        assert!(
            sink.diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == "E0512")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_outside_standalone_root() {
        use std::os::unix::fs::symlink;

        let dir = scratch("outside-symlink");
        let source_dir = dir.join("src");
        fs::create_dir_all(&source_dir).unwrap();
        write(&dir, "outside.zrk", "fn secret() { }\n");
        symlink(dir.join("outside.zrk"), source_dir.join("escape.zrk")).unwrap();
        let entry = write(
            &source_dir,
            "main.zrk",
            "import { secret } from \"./escape\";\n",
        );

        let mut sink = DiagnosticSink::new();
        let krate = load(&entry, &mut sink).unwrap();

        assert_eq!(krate.units.len(), 1, "symlink target must not be loaded");
        assert!(sink.has_errors());
        assert!(
            sink.diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == "E0512")
        );
    }

    #[cfg(unix)]
    #[test]
    fn in_root_symlink_aliases_share_one_source_identity() {
        use std::os::unix::fs::symlink;

        let dir = scratch("in-root-symlink");
        write(&dir, "lib.zrk", "fn helper() { }\n");
        symlink(dir.join("lib.zrk"), dir.join("alias.zrk")).unwrap();
        let entry = write(
            &dir,
            "main.zrk",
            "import { helper } from \"./lib\";\nimport { helper } from \"./alias\";\n",
        );

        let mut sink = DiagnosticSink::new();
        let krate = load(&entry, &mut sink).unwrap();

        assert_eq!(krate.units.len(), 2, "the aliased source loads only once");
        assert!(!sink.has_errors());
    }

    #[test]
    fn package_root_reexports_and_declared_local_dependencies_are_loaded_once() {
        let dir = scratch("project-packages");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("dep/src")).unwrap();
        write(
            &dir,
            ".zkinit",
            "project { name: \"app\"; version: \"0.1.0\"; type: application; entry: \"src/main.zrk\"; }\ndependencies { acme.web: { from: \"path:dep\"; } }",
        );
        write(
            &dir.join("src"),
            "main.zrk",
            "import { Api } from acme.web;\nfn main(): Void { }",
        );
        write(
            &dir.join("dep"),
            ".zkinit",
            "project { name: \"acme.web\"; version: \"1.0.0\"; type: package; }",
        );
        write(
            &dir.join("dep/src"),
            "package.zrk",
            "package acme.web;\nshare mod api from \"./api\";",
        );
        write(&dir.join("dep/src"), "api.zrk", "share class Api { }");
        let dependency_root = dir.join("dep");
        let dependency_manifest = project::read_manifest(&dependency_root.join(".zkinit")).unwrap();
        let hash = project::package_integrity_hash(&dependency_root, &dependency_manifest).unwrap();
        let package = project::LockedPackage {
            name: "acme.web".into(),
            version: "1.0.0".into(),
            source: "local".into(),
            locator: "path:dep".into(),
            revision: None,
            hash: format!("sha256:{hash}"),
            package_root: "src/package.zrk".into(),
        };
        write(
            &dir,
            "zirk.lock",
            &project::format_lockfile(&project::Lockfile {
                packages: [(package.name.clone(), package)].into(),
            }),
        );
        let mut sink = DiagnosticSink::new();
        let krate = load_project(&dir, &mut sink).unwrap();
        assert_eq!(krate.units.len(), 3);
        assert!(!sink.has_errors());
    }

    #[test]
    fn explicit_project_entry_keeps_manifest_lock_dependency_global_and_requirement_context() {
        let dir = scratch("explicit-project-entry-context");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("vendor/runtime/src")).unwrap();
        write(
            &dir,
            ".zkinit",
            "project { name: \"app\"; version: \"0.1.0\"; type: application; entry: \"src/main.zrk\"; }\ndependencies { acme.runtime: { from: \"path:vendor/runtime\"; } }\nglobals { counter: UInt64; }\nrequires { filesystem.read; }",
        );
        write(&dir, "src/main.zrk", "mut main_entry = true;\n");
        write(
            &dir,
            "src/alternate.zrk",
            "import { Helper } from acme.runtime;\nmut explicit_entry = true;\n",
        );
        write(
            &dir,
            "vendor/runtime/.zkinit",
            "project { name: \"acme.runtime\"; version: \"1.0.0\"; type: package; }",
        );
        write(
            &dir,
            "vendor/runtime/src/package.zrk",
            "package acme.runtime;\nshare class Helper { }\n",
        );

        let dependency_root = dir.join("vendor/runtime");
        let dependency_manifest = project::read_manifest(&dependency_root.join(".zkinit")).unwrap();
        let hash = project::package_integrity_hash(&dependency_root, &dependency_manifest).unwrap();
        let package = project::LockedPackage {
            name: "acme.runtime".into(),
            version: "1.0.0".into(),
            source: "local".into(),
            locator: "path:vendor/runtime".into(),
            revision: None,
            hash: format!("sha256:{hash}"),
            package_root: "src/package.zrk".into(),
        };
        write(
            &dir,
            "zirk.lock",
            &project::format_lockfile(&project::Lockfile {
                packages: [(package.name.clone(), package)].into(),
            }),
        );

        let explicit = dir.join("src/alternate.zrk");
        let mut sink = DiagnosticSink::new();
        let krate = load_project_entry(&explicit, &explicit, &mut sink)
            .expect("the explicit file loads with its nearest project context");
        assert_eq!(
            Path::new(krate.sources.entry().name())
                .canonicalize()
                .unwrap(),
            explicit.canonicalize().unwrap()
        );
        assert!(krate.module_roots.packages.contains_key("acme.runtime"));
        let project_context = krate.project.as_ref().expect("project context is retained");
        assert!(
            project_context
                .manifest
                .dependencies
                .contains_key("acme.runtime")
        );
        assert!(project_context.manifest.globals.contains_key("counter"));
        assert!(
            project_context
                .manifest
                .requires
                .contains(&project::Capability::FilesystemRead)
        );
        assert!(
            !sink.has_errors(),
            "{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );

        let mut frontend_sink = DiagnosticSink::new();
        let frontend = crate::frontend::run_frontend(&explicit, &mut frontend_sink)
            .expect("the explicit entry is checked against the application globals");
        assert!(frontend.checked.project_globals.contains_key("counter"));
        assert!(frontend.module_roots.packages.contains_key("acme.runtime"));
        assert!(
            !frontend_sink.has_errors(),
            "{}",
            frontend_sink.render(zirk_diagnostics::RenderStyle::Human)
        );
    }

    #[test]
    fn development_loader_adds_root_dev_packages_without_inheriting_package_dev_deps() {
        let dir = scratch("development-packages");
        write(
            &dir,
            ".zkinit",
            "project { name: \"app\"; version: \"0.1.0\"; type: application; entry: \"src/main.zrk\"; }\ndependencies { acme.runtime: { from: \"path:vendor/runtime\"; } }\ndev_dependencies { acme.testkit: { from: \"path:dev/testkit\"; } }",
        );
        fs::create_dir_all(dir.join("src")).unwrap();
        write(&dir, "src/main.zrk", "");
        for (path, name) in [
            ("vendor/runtime", "acme.runtime"),
            ("vendor/hidden", "acme.hidden"),
            ("dev/testkit", "acme.testkit"),
        ] {
            fs::create_dir_all(dir.join(path).join("src")).unwrap();
            write(
                &dir,
                &format!("{path}/.zkinit"),
                &format!("project {{ name: \"{name}\"; version: \"1.0.0\"; type: package; }}"),
            );
            write(&dir, &format!("{path}/src/package.zrk"), "");
        }
        write(
            &dir,
            "vendor/runtime/.zkinit",
            "project { name: \"acme.runtime\"; version: \"1.0.0\"; type: package; }\ndev_dependencies { acme.hidden: { from: \"path:../hidden\"; } }",
        );

        let mut locked = std::collections::BTreeMap::new();
        for (name, relative, locator) in [
            ("acme.runtime", "vendor/runtime", "path:vendor/runtime"),
            ("acme.testkit", "dev/testkit", "path:dev/testkit"),
        ] {
            let package_root = dir.join(relative);
            let manifest = project::read_manifest(&package_root.join(".zkinit")).unwrap();
            let hash = project::package_integrity_hash(&package_root, &manifest).unwrap();
            let package = project::LockedPackage {
                name: name.into(),
                version: "1.0.0".into(),
                source: "local".into(),
                locator: locator.into(),
                revision: None,
                hash: format!("sha256:{hash}"),
                package_root: manifest.entry.to_string_lossy().into_owned(),
            };
            locked.insert(name.into(), package);
        }
        write(
            &dir,
            "zirk.lock",
            &project::format_lockfile(&project::Lockfile { packages: locked }),
        );

        let mut runtime_sink = DiagnosticSink::new();
        let runtime = load_project(&dir, &mut runtime_sink).unwrap();
        assert!(runtime.module_roots.packages.contains_key("acme.runtime"));
        assert!(!runtime.module_roots.packages.contains_key("acme.testkit"));
        assert!(!runtime.module_roots.packages.contains_key("acme.hidden"));

        let mut development_sink = DiagnosticSink::new();
        let development = load_project_for_development(&dir, &mut development_sink).unwrap();
        assert!(
            development
                .module_roots
                .packages
                .contains_key("acme.testkit")
        );
        assert!(
            !development
                .module_roots
                .packages
                .contains_key("acme.hidden")
        );
        assert!(!runtime_sink.has_errors());
        assert!(!development_sink.has_errors());
    }

    #[test]
    fn file_initializers_follow_depth_first_import_order_and_visit_diamonds_once() {
        let dir = scratch("initializer-order");
        write(
            &dir,
            "main.zrk",
            "import { a } from \"./a\";\nimport { b } from \"./b\";\nreport(\"main\");",
        );
        write(&dir, "a.zrk", "import { c } from \"./c\";\nreport(\"a\");");
        write(&dir, "b.zrk", "import { c } from \"./c\";\nreport(\"b\");");
        write(&dir, "c.zrk", "report(\"c\");");

        let mut sink = DiagnosticSink::new();
        let krate = load(&dir.join("main.zrk"), &mut sink).unwrap();
        let order: Vec<_> = krate
            .units
            .iter()
            .map(|unit| {
                Path::new(krate.sources.file(unit.program.span.file).name())
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(order, ["c.zrk", "a.zrk", "b.zrk", "main.zrk"]);
        assert!(krate.units.iter().all(|unit| unit.program.body.len() == 1));
        assert!(!sink.has_errors());
    }

    #[test]
    fn application_manifest_context_is_retained_with_the_loaded_graph() {
        let dir = scratch("application-context");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join(".zkinit"),
            "project { name: \"demo.globals\"; version: \"0.1.0\"; type: application; }\nglobals { count: UInt64; }\n",
        )
        .unwrap();
        write(&dir.join("src"), "main.zrk", "mut ready = true;\n");

        let mut sink = DiagnosticSink::new();
        let krate = load_project(&dir, &mut sink).unwrap();
        let project = krate.project.expect("project context is attached");
        assert_eq!(project.manifest.name, "demo.globals");
        assert_eq!(project.manifest.globals["count"].type_name, "UInt64");
        assert_eq!(project.root, absolute_normalize(&dir));
        assert!(!sink.has_errors());

        let mut frontend_sink = DiagnosticSink::new();
        let frontend = crate::frontend::run_frontend(&dir, &mut frontend_sink)
            .expect("the project frontend resolves its declared global");
        let global = frontend.checked.project_globals.get("count").unwrap();
        assert_eq!(global.ty, zirk_sema::Type::from_name("UInt64").unwrap());
        assert!(!global.readonly);
        assert!(!frontend_sink.has_errors());
    }

    #[test]
    fn global_from_file_is_loaded_for_custom_type_resolution_without_module_startup() {
        let dir = scratch("global-from-source");
        fs::create_dir_all(dir.join("src")).unwrap();
        write(
            &dir,
            ".zkinit",
            "project { name: \"demo.settings\"; version: \"0.1.0\"; type: application; }\nglobals { settings: { type: Settings; from: \"src/settings.zrk\"; } }",
        );
        write(&dir, "src/main.zrk", "mut ready = true;\n");
        write(&dir, "src/settings.zrk", "share class Settings { }\n");

        let mut sink = DiagnosticSink::new();
        let frontend = crate::frontend::run_frontend(&dir, &mut sink).unwrap_or_else(|| {
            panic!(
                "the manifest's exact custom-type source is loaded:\n{}",
                sink.render(zirk_diagnostics::RenderStyle::Human)
            )
        });
        let global = &frontend.checked.project_globals["settings"];
        let class_id = match global.ty.base {
            zirk_sema::Base::Class(id) => id,
            other => panic!("expected the custom class type, got {other:?}"),
        };
        assert_eq!(frontend.checked.classes[class_id as usize].name, "Settings");
        assert!(
            frontend
                .sources
                .files()
                .any(|(_, file)| file.name().ends_with("src/settings.zrk"))
        );
        assert!(!frontend.program.functions.iter().any(|function| {
            function.name.name.starts_with("$zirk.module.")
                && frontend.sources.files().any(|(file, source)| {
                    source.name().ends_with("src/settings.zrk")
                        && function.name.name == format!("$zirk.module.{}", file.0)
                })
        }));
        assert!(
            !sink.has_errors(),
            "{}",
            sink.render(zirk_diagnostics::RenderStyle::Human)
        );

        write(&dir, "src/settings.zrk", "share class Other { }\n");
        let manifest = project::read_manifest(&dir.join(".zkinit")).unwrap();
        let mismatch = project::validate_global_type_sources(&dir, &manifest).unwrap_err();
        assert!(mismatch.to_string().contains("does not declare that type"));

        write(
            &dir,
            "src/settings.zrk",
            "share class Settings { }\nmut accidental_side_effect = true;\n",
        );
        let manifest = project::read_manifest(&dir.join(".zkinit")).unwrap();
        let executable = project::validate_global_type_sources(&dir, &manifest).unwrap_err();
        assert!(
            executable
                .to_string()
                .contains("must not contain executable file-scope statements")
        );

        write(
            &dir,
            ".zkinit",
            "project { name: \"demo.settings\"; version: \"0.1.0\"; type: application; }\nglobals { settings: { type: Settings; from: \"src/missing.zrk\"; } }",
        );
        let manifest = project::read_manifest(&dir.join(".zkinit")).unwrap();
        let missing = project::validate_global_type_sources(&dir, &manifest).unwrap_err();
        assert!(missing.to_string().contains("src/missing.zrk"));
    }

    #[test]
    fn explicit_project_global_default_is_type_checked_and_retained_as_initializer() {
        let dir = scratch("global-default");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join(".zkinit"),
            "project { name: \"demo.defaults\"; version: \"0.1.0\"; type: application; }\nglobals { count: { type: UInt64; default: 0; } }\n",
        )
        .unwrap();
        write(&dir.join("src"), "main.zrk", "");

        let mut sink = DiagnosticSink::new();
        let frontend = crate::frontend::run_frontend(&dir, &mut sink)
            .expect("a type-correct explicit global default checks");
        let global = &frontend.checked.project_globals["count"];
        assert_eq!(
            global.default_function.as_deref(),
            Some("$zirk.global.default.0")
        );
        assert!(
            frontend
                .program
                .functions
                .iter()
                .any(|function| function.name.name == "$zirk.global.default.0")
        );

        fs::write(
            dir.join(".zkinit"),
            "project { name: \"demo.defaults\"; version: \"0.1.0\"; type: application; }\nglobals { count: { type: UInt64; default: \"wrong\"; } }\n",
        )
        .unwrap();
        let mut invalid_sink = DiagnosticSink::new();
        assert!(crate::frontend::run_frontend(&dir, &mut invalid_sink).is_none());
        let diagnostic = invalid_sink.render(zirk_diagnostics::RenderStyle::Human);
        assert!(
            diagnostic.contains("returned type does not match"),
            "{diagnostic}"
        );
        assert!(
            diagnostic.contains(".zkinit/globals/0-count.zrk"),
            "{diagnostic}"
        );
    }

    #[test]
    fn executable_module_cycles_report_the_complete_cycle() {
        let dir = scratch("initializer-cycle");
        write(
            &dir,
            "main.zrk",
            "import { a } from \"./a\";\nreport(\"main\");",
        );
        write(&dir, "a.zrk", "import { b } from \"./b\";\nreport(\"a\");");
        write(&dir, "b.zrk", "import { a } from \"./a\";\nreport(\"b\");");

        let mut sink = DiagnosticSink::new();
        let krate = load(&dir.join("main.zrk"), &mut sink).unwrap();
        assert!(sink.has_errors());
        let diagnostic = sink
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == codes::IMPORT_CYCLE)
            .unwrap();
        assert!(diagnostic.message.contains("a.zrk"));
        assert!(diagnostic.message.contains("b.zrk"));
        assert_eq!(krate.units.len(), 3);
    }

    #[test]
    fn declaration_only_import_cycles_remain_valid() {
        let dir = scratch("declaration-cycle");
        write(&dir, "main.zrk", "import { A } from \"./a\";");
        write(&dir, "a.zrk", "import { B } from \"./b\";\nclass A { }");
        write(&dir, "b.zrk", "import { A } from \"./a\";\nclass B { }");

        let mut sink = DiagnosticSink::new();
        let krate = load(&dir.join("main.zrk"), &mut sink).unwrap();
        assert_eq!(krate.units.len(), 3);
        assert!(!sink.has_errors());
    }
}
