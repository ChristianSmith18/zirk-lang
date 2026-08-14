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

use crate::codes;
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
    let mut loader = Loader {
        sources: SourceMap::new(),
        units: Vec::new(),
        by_path: HashMap::new(),
        sink,
    };

    loader.load(entry, None)?;

    Some(Crate {
        sources: loader.sources,
        units: loader.units,
    })
}

struct Loader<'a> {
    sources: SourceMap,
    units: Vec<Unit>,
    /// Files already loaded, so a diamond of imports reads each one once.
    by_path: HashMap<PathBuf, FileId>,
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

        let Ok(text) = std::fs::read_to_string(&path) else {
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

        let file = self
            .sources
            .add(SourceFile::new(path.display().to_string(), text));
        self.by_path.insert(path.clone(), file);

        let tokens = zirk_lexer::tokenize(self.sources.file(file), self.sink);
        let program = zirk_parser::parse(self.sources.file(file), &tokens, self.sink);

        // Imports are followed even when the file had errors: reporting every
        // file's problems in one run beats one file per invocation.
        let directory = path.parent().map(Path::to_path_buf).unwrap_or_default();

        for import in &program.imports {
            let ImportSource::Local {
                path: written,
                span,
            } = &import.source
            else {
                // Standard modules are not files of the crate.
                continue;
            };

            let target = directory.join(format!("{written}.zrk"));
            self.load(&target, Some((file, *span)));
        }

        self.units.push(Unit { program });

        Some(())
    }
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
