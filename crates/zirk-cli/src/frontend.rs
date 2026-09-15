//! The frontend driver: loading, parsing, and checking without lowering.
//!
//! This module is independent of `zirk-ir`, `zirk-codegen-llvm`, the linker,
//! and the runtime. It is the shared half of both `zirk` and `zirk-check`.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use zirk_diagnostics::{Color, DiagnosticSink, RenderStyle, SourceMap};

/// Result of a successful frontend run.
pub struct FrontendResult {
    pub program: zirk_ast::Program,
    pub checked: zirk_sema::CheckedProgram,
    pub sources: SourceMap,
}

/// Loads, parses, resolves, and checks the program rooted at `path`.
///
/// On failure it returns `None` and `sink` contains the diagnostics that
/// explain why. On success it returns the merged program and the checked tree.
pub fn run_frontend(path: &Path, sink: &mut DiagnosticSink) -> Option<FrontendResult> {
    let loaded = crate::modules::load(path, sink)?;
    if sink.has_errors() {
        // Loading recovered enough to continue, but the errors already make the
        // program invalid. Returning None keeps the contract simple: a Some
        // result is one that a backend pass may attempt to lower.
        return None;
    }

    let program = merge(&loaded);
    let checked = zirk_sema::check(&loaded.sources, &program, sink);
    if sink.has_errors() {
        return None;
    }

    Some(FrontendResult {
        program,
        checked,
        sources: loaded.sources,
    })
}

/// Like [`run_frontend`], but keeps the checked program when the checker
/// reported errors: `zirk check --types` still renders the entries it
/// managed to resolve (spec `zirk-check`, "Type map on a file with
/// errors").
pub fn run_frontend_lenient(path: &Path, sink: &mut DiagnosticSink) -> Option<FrontendResult> {
    let loaded = crate::modules::load(path, sink)?;
    if sink.has_errors() {
        return None;
    }

    let program = merge(&loaded);
    let checked = zirk_sema::check(&loaded.sources, &program, sink);
    Some(FrontendResult {
        program,
        checked,
        sources: loaded.sources,
    })
}

/// Renders the checker's type map as structured JSON — sibling to the
/// diagnostics, on stdout (spec `zirk-check`, "Type-map output mode").
pub fn render_type_map(checked: &zirk_sema::CheckedProgram, sources: &SourceMap) -> String {
    let mut out = String::from("{\"types\":[");
    for (index, entry) in checked.type_map.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let file = sources.file(entry.span.file).name();
        out.push_str(&format!(
            "{{\"file\":\"{}\",\"start\":{},\"end\":{},\"kind\":\"{}\"",
            json_escape(file),
            entry.span.start,
            entry.span.end,
            entry.kind.as_str(),
        ));
        if let Some(name) = &entry.name {
            out.push_str(&format!(",\"name\":\"{}\"", json_escape(name)));
        }
        out.push_str(&format!(
            ",\"type\":\"{}\",\"inferred\":{}}}",
            json_escape(&entry.text),
            entry.inferred,
        ));
    }
    out.push_str("]}");
    out
}

/// Escapes a string for embedding in the JSON `render_type_map` writes —
/// quotes, backslashes and control characters only; the type map's own
/// field names are fixed ASCII.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Merges the files of a crate into the program the checker sees.
fn merge(loaded: &crate::modules::Crate) -> zirk_ast::Program {
    let mut program = zirk_ast::Program {
        imports: Vec::new(),
        uses: Vec::new(),
        enums: Vec::new(),
        classes: Vec::new(),
        contracts: Vec::new(),
        functions: Vec::new(),
        type_aliases: Vec::new(),
        externs: Vec::new(),
        span: loaded.sources.entry().span(0, 0),
    };

    for unit in &loaded.units {
        program.imports.extend(unit.program.imports.iter().cloned());
        program.uses.extend(unit.program.uses.iter().cloned());
        program.enums.extend(unit.program.enums.iter().cloned());
        program.classes.extend(unit.program.classes.iter().cloned());
        program
            .contracts
            .extend(unit.program.contracts.iter().cloned());
        program
            .functions
            .extend(unit.program.functions.iter().cloned());
        program
            .type_aliases
            .extend(unit.program.type_aliases.iter().cloned());
        program.externs.extend(unit.program.externs.iter().cloned());
    }

    program
}

/// Renders the accumulated diagnostics.
pub fn render(sink: &DiagnosticSink, json: bool, color: Color) -> String {
    let style = if json {
        RenderStyle::Json
    } else {
        RenderStyle::Human
    };
    sink.render_colored(style, color)
}

/// Resolves a source argument to the path the compiler reads.
///
/// If the argument already ends with `.zrk`, it is used verbatim. Otherwise,
/// `.zrk` is appended. This lets `zirk run main` resolve to `main.zrk` without
/// any ambiguity.
pub fn resolve_source_path(arg: &str) -> PathBuf {
    if arg.ends_with(".zrk") {
        PathBuf::from(arg)
    } else {
        PathBuf::from(format!("{arg}.zrk"))
    }
}

/// Decides whether the diagnostics carry colour.
///
/// `ZIRK_COMPILER_SPEC.md` section 9 requires deterministic output, so colour
/// only appears when the destination is a terminal a person is reading. Piping
/// the output must yield exactly the same bytes as an uncoloured run.
pub fn resolve_color(requested: Option<&str>) -> Color {
    match requested {
        Some("always") => return Color::Ansi,
        Some("never") => return Color::Never,
        _ => {}
    }

    if std::env::var_os("NO_COLOR").is_some() {
        return Color::Never;
    }

    // Standard error and not output: that is where diagnostics go.
    if std::io::stderr().is_terminal() {
        Color::Ansi
    } else {
        Color::Never
    }
}
