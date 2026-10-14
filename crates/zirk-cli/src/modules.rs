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
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use zirk_ast::{ImportSource, Program};
use zirk_diagnostics::{Diagnostic, DiagnosticSink, FileId, SourceFile, SourceMap};

/// The parsed files of one crate.
pub struct Crate {
    pub sources: SourceMap,
    /// One parsed program per file, in the order the files were loaded.
    pub units: Vec<Unit>,
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
    let manifest_path = project::discover(start)?;
    let manifest = match project::read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            sink.emit(Diagnostic::error(codes::INVALID_PROJECT, error.to_string()));
            return None;
        }
    };
    let root = manifest_path.parent().unwrap_or_else(|| Path::new("."));
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
    if let Err(error) = project::validate_lockfile(root, &manifest, lock.as_ref()) {
        sink.emit(Diagnostic::error(codes::INVALID_PROJECT, error.to_string()));
        return None;
    }
    let empty_lock = project::Lockfile {
        packages: std::collections::BTreeMap::new(),
    };
    load_with_project(
        &root.join(&manifest.entry),
        &manifest,
        lock.as_ref().unwrap_or(&empty_lock),
        root,
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
    let overlay: HashMap<PathBuf, String> = overlay
        .iter()
        .map(|(path, text)| (normalize(path), text.clone()))
        .collect();

    let mut loader = Loader {
        sources: SourceMap::new(),
        units: Vec::new(),
        by_path: HashMap::new(),
        overlay: &overlay,
        package_roots: standard_package_roots(),
        sink,
    };

    loader.load(entry, None)?;

    Some(Crate {
        sources: loader.sources,
        units: loader.units,
    })
}

fn load_with_project(
    entry: &Path,
    manifest: &ProjectManifest,
    lock: &project::Lockfile,
    root: &Path,
    sink: &mut DiagnosticSink,
) -> Option<Crate> {
    let overlay = HashMap::new();
    let package_roots: HashMap<String, PathBuf> =
        match project::resolve_package_roots(root, manifest, lock) {
            Ok(roots) => roots.into_iter().collect(),
            Err(error) => {
                sink.emit(Diagnostic::error(
                    codes::PACKAGE_RESOLUTION,
                    error.to_string(),
                ));
                return None;
            }
        };
    let mut loader = Loader {
        sources: SourceMap::new(),
        units: Vec::new(),
        by_path: HashMap::new(),
        overlay: &overlay,
        package_roots: {
            let mut roots = standard_package_roots();
            roots.extend(package_roots);
            roots
        },
        sink,
    };
    loader.load(entry, None)?;
    Some(Crate {
        sources: loader.sources,
        units: loader.units,
    })
}

/// Compiler-owned package roots are ordinary Zirk source trees bundled with
/// the toolchain. They enter the same graph as a user package, but their
/// logical identity is fixed and their trusted host bindings remain governed
/// by `zirk_sema::stdlib` rather than source syntax.
fn standard_package_roots() -> HashMap<String, PathBuf> {
    HashMap::from([(
        "std.io".into(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std/io/package.zrk"),
    )])
}

/// Bundled source stays available to the browser/WASM frontend without a
/// filesystem read. Native loading uses the same text, so the package graph is
/// identical across targets.
fn standard_source(path: &Path) -> Option<&'static str> {
    let directory =
        normalize(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/std/io"));
    match path
        .strip_prefix(&directory)
        .ok()?
        .to_string_lossy()
        .as_ref()
    {
        "package.zrk" => Some(include_str!("../../../stdlib/std/io/package.zrk")),
        "stdin.zrk" => Some(include_str!("../../../stdlib/std/io/stdin.zrk")),
        "stdout.zrk" => Some(include_str!("../../../stdlib/std/io/stdout.zrk")),
        "stderr.zrk" => Some(include_str!("../../../stdlib/std/io/stderr.zrk")),
        _ => None,
    }
}

struct Loader<'a> {
    sources: SourceMap,
    units: Vec<Unit>,
    /// Files already loaded, so a diamond of imports reads each one once.
    by_path: HashMap<PathBuf, FileId>,
    /// In-memory text that shadows the filesystem, keyed by normalized path.
    overlay: &'a HashMap<PathBuf, String>,
    /// Logical package roots resolved from the project manifest. Non-local
    /// transports intentionally do not become filesystem imports here.
    package_roots: HashMap<String, PathBuf>,
    sink: &'a mut DiagnosticSink,
}

impl Loader<'_> {
    /// Loads one file and, depth first, everything it imports.
    ///
    /// `from` is the span of the `import` that asked for it, so a missing file
    /// is reported where it was requested rather than at the top of nowhere.
    fn load(&mut self, path: &Path, from: Option<(FileId, zirk_diagnostics::Span)>) -> Option<()> {
        let path = normalize(path);

        if self.by_path.contains_key(&path) {
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
                path.starts_with(root)
                    .then(|| (name.clone(), root.to_path_buf()))
            })
            .unwrap_or_else(|| ("local".into(), directory_for(&path)));
        let canonical_module_path = path
            .strip_prefix(&package_root)
            .unwrap_or(&path)
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
        self.by_path.insert(path.clone(), file);

        let tokens = zirk_lexer::tokenize(self.sources.file(file), self.sink);
        let mut program = zirk_parser::parse(self.sources.file(file), &tokens, self.sink);

        // Imports are followed even when the file had errors: reporting every
        // file's problems in one run beats one file per invocation.
        let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();

        for import in &program.imports {
            match &import.source {
                ImportSource::Local {
                    path: written,
                    span,
                } => {
                    self.load(
                        &directory.join(format!("{written}.zrk")),
                        Some((file, *span)),
                    );
                }
                ImportSource::Package { path, span } => {
                    let package = path.split("::").next().unwrap_or(path).to_string();
                    if zirk_sema::stdlib::standard_module(path).is_some() {
                        if let Some(root) = self.package_roots.get(&package).cloned() {
                            self.load(&root, Some((file, *span)));
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
                        self.load(&root, Some((file, *span)));
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
            self.load(&target, Some((file, reexport.span)));
            let target = normalize(&target);
            if let Some(target_file) = self.by_path.get(&target)
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

        self.units.push(Unit { program });

        Some(())
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

fn directory_for(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// Removes `.` and `..` without touching the filesystem.
///
/// `canonicalize` is not used because it fails when the file does not exist,
/// and a missing import deserves its own diagnostic rather than a read error
/// with no location.
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();

    for component in path.components() {
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
    fn package_root_reexports_and_declared_local_dependencies_are_loaded_once() {
        let dir = scratch("project-packages");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("dep/src")).unwrap();
        write(
            &dir,
            ".zkinit",
            "project \"app\"\nkind application\nentry \"src/main.zrk\"\ndependency acme.web ^1 local \"dep\"",
        );
        write(
            &dir,
            "zirk.lock",
            "acme.web 1.0 local sha256:fixture src/package.zrk",
        );
        write(
            &dir.join("src"),
            "main.zrk",
            "import { Api } from acme.web;\nfn main(): Void { }",
        );
        write(
            &dir.join("dep"),
            ".zkinit",
            "project \"acme.web\"\nkind package",
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
        write(
            &dir,
            "zirk.lock",
            &format!("acme.web 1.0 local sha256:{hash} src/package.zrk"),
        );
        let mut sink = DiagnosticSink::new();
        let krate = load_project(&dir, &mut sink).unwrap();
        assert_eq!(krate.units.len(), 3);
        assert!(!sink.has_errors());
    }
}
