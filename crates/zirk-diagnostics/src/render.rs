//! Diagnostic rendering.
//!
//! The human-readable format reproduces `ZIRK_COMPILER_SPEC.md` section 8:
//!
//! ```text
//! error[E1234]: precise description
//!   src/users.zrk:18:12
//!    |
//! 18 |     problematic expression
//!    |            ^ localized explanation
//!    |
//!    = cause: semantic reason
//!    = help: concrete action
//! ```
//!
//! The labels are in English. The spec illustrates them in Spanish because that
//! document is written in Spanish, but compiler output is addressed to whoever
//! uses Zirk. See `docs/decisions/ADR-006-language-of-the-codebase.md`.

use crate::Diagnostic;
use std::fmt::Write as _;

/// Requested output form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStyle {
    /// Human-readable format.
    Human,
    /// Structured format for tooling (`--json`).
    Json,
}

pub(crate) fn human(diagnostic: &Diagnostic) -> String {
    let mut out = String::new();

    // error[E1234]: precise description
    let _ = writeln!(
        out,
        "{}[{}]: {}",
        diagnostic.severity.as_str(),
        diagnostic.code,
        diagnostic.message
    );

    // The width of the left gutter depends on how many digits the line has.
    let gutter = diagnostic
        .location
        .as_ref()
        .map(|l| l.line.to_string().len())
        .unwrap_or(0);
    let pad = " ".repeat(gutter);

    if let Some(location) = &diagnostic.location {
        let _ = writeln!(
            out,
            "{pad} {}:{}:{}",
            location.file, location.line, location.column
        );

        // The snippet is optional: without source, the diagnostic keeps its
        // header, location, cause and help.
        if let Some(snippet) = &diagnostic.snippet {
            let _ = writeln!(out, "{pad} |");
            let _ = writeln!(out, "{} | {}", location.line, snippet.line_text);

            let offset = " ".repeat(location.column.saturating_sub(1) as usize);
            let marker = "^".repeat(snippet.width as usize);
            match &snippet.label {
                Some(label) => {
                    let _ = writeln!(out, "{pad} | {offset}{marker} {label}");
                }
                None => {
                    let _ = writeln!(out, "{pad} | {offset}{marker}");
                }
            }
            let _ = writeln!(out, "{pad} |");
        }
    }

    if let Some(cause) = &diagnostic.cause {
        let _ = writeln!(out, "{pad} = cause: {cause}");
    }
    if let Some(help) = &diagnostic.help {
        let _ = writeln!(out, "{pad} = help: {help}");
    }

    out
}

pub(crate) fn json(diagnostic: &Diagnostic) -> String {
    let mut out = String::from("{");

    let _ = write!(out, "\"severity\":\"{}\"", diagnostic.severity.as_str());
    let _ = write!(out, ",\"code\":\"{}\"", diagnostic.code);
    let _ = write!(out, ",\"message\":{}", quote(&diagnostic.message));

    match &diagnostic.location {
        Some(location) => {
            let _ = write!(
                out,
                ",\"location\":{{\"file\":{},\"line\":{},\"column\":{}}}",
                quote(&location.file),
                location.line,
                location.column
            );
        }
        None => out.push_str(",\"location\":null"),
    }

    match &diagnostic.cause {
        Some(cause) => {
            let _ = write!(out, ",\"cause\":{}", quote(cause));
        }
        None => out.push_str(",\"cause\":null"),
    }

    match &diagnostic.help {
        Some(help) => {
            let _ = write!(out, ",\"help\":{}", quote(help));
        }
        None => out.push_str(",\"help\":null"),
    }

    out.push('}');
    out
}

pub(crate) fn sink(diagnostics: &[Diagnostic], style: RenderStyle) -> String {
    // Diagnostics are sorted by location before rendering. Pipeline stages emit
    // in the order they work — all of the lexing first, then all of the
    // parsing — so without this an error on line 3 can appear before one on
    // line 2. Readers expect to walk their file from top to bottom.
    let mut sorted: Vec<&Diagnostic> = diagnostics.iter().collect();
    sorted.sort_by_key(|d| sort_key(d));

    match style {
        RenderStyle::Human => sorted.into_iter().map(human).collect::<Vec<_>>().join("\n"),
        RenderStyle::Json => {
            let items = sorted.into_iter().map(json).collect::<Vec<_>>().join(",");
            format!("[{items}]")
        }
    }
}

/// Sort key of a diagnostic: file, line and column.
///
/// Diagnostics without a location go last: they belong to no point in the file,
/// and putting them first would displace the ones that do.
fn sort_key(d: &Diagnostic) -> (bool, String, u32, u32) {
    match &d.location {
        Some(l) => (false, l.file.clone(), l.line, l.column),
        None => (true, String::new(), 0, 0),
    }
}

/// Serializes a string as a JSON literal, escaping what RFC 8259 requires.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use crate::{Code, Diagnostic, DiagnosticSink, Location, RenderStyle, Snippet};

    fn complete_diagnostic() -> Diagnostic {
        Diagnostic::error(Code::new("E1234"), "precise description")
            .at(Location::new("src/users.zrk", 18, 12))
            .with_snippet(
                Snippet::new("    problematic expression", 1).with_label("localized explanation"),
            )
            .with_cause("semantic reason")
            .with_help("concrete action")
    }

    #[test]
    fn human_format_reproduces_the_spec() {
        let expected = "\
error[E1234]: precise description
   src/users.zrk:18:12
   |
18 |     problematic expression
   |            ^ localized explanation
   |
   = cause: semantic reason
   = help: concrete action
";
        assert_eq!(complete_diagnostic().render(), expected);
    }

    #[test]
    fn without_a_snippet_the_rest_is_preserved() {
        let rendered = Diagnostic::error(Code::new("E0002"), "no source available")
            .at(Location::new("src/main.zrk", 3, 5))
            .with_cause("the file could not be read")
            .with_help("check the file permissions")
            .render();

        assert!(rendered.contains("error[E0002]: no source available"));
        assert!(rendered.contains("src/main.zrk:3:5"));
        assert!(rendered.contains("= cause: the file could not be read"));
        assert!(rendered.contains("= help: check the file permissions"));
        // Without a snippet there must be no source gutter and no marker.
        assert!(!rendered.contains('^'));
    }

    #[test]
    fn without_help_no_generic_one_is_invented() {
        let rendered = Diagnostic::error(Code::new("E0003"), "error with no clear fix")
            .with_cause("state is not representable")
            .render();

        assert!(!rendered.contains("help"));
    }

    #[test]
    fn the_marker_lines_up_with_the_column() {
        let rendered = Diagnostic::error(Code::new("E0004"), "unexpected token")
            .at(Location::new("a.zrk", 1, 5))
            .with_snippet(Snippet::new("abcdefgh", 3))
            .render();

        let marker_line = rendered
            .lines()
            .find(|l| l.contains('^'))
            .expect("there must be a marker line");

        // Gutter "1 | " (4 characters) plus 4 spaces for column 5.
        assert_eq!(marker_line, "  |     ^^^");
    }

    #[test]
    fn json_escapes_quotes_and_newlines() {
        let rendered =
            Diagnostic::error(Code::new("E0005"), "said \"hello\"\nand left").render_json();

        assert!(rendered.contains(r#""message":"said \"hello\"\nand left""#));
    }

    #[test]
    fn json_keeps_the_required_fields() {
        let rendered = complete_diagnostic().render_json();

        assert!(rendered.contains(r#""severity":"error""#));
        assert!(rendered.contains(r#""code":"E1234""#));
        assert!(rendered.contains(r#""line":18"#));
        assert!(rendered.contains(r#""column":12"#));
        assert!(rendered.contains(r#""cause":"semantic reason""#));
        assert!(rendered.contains(r#""help":"concrete action""#));
    }

    #[test]
    fn json_represents_missing_fields_as_null() {
        let rendered = Diagnostic::warning(Code::new("W0001"), "something").render_json();

        assert!(rendered.contains(r#""location":null"#));
        assert!(rendered.contains(r#""cause":null"#));
        assert!(rendered.contains(r#""help":null"#));
    }

    #[test]
    fn the_json_sink_produces_an_array() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error(Code::new("E0001"), "one"));
        sink.emit(Diagnostic::warning(Code::new("W0001"), "two"));

        let rendered = sink.render(RenderStyle::Json);
        assert!(rendered.starts_with('['));
        assert!(rendered.ends_with(']'));
        assert_eq!(rendered.matches(r#""severity""#).count(), 2);
    }

    #[test]
    fn diagnostics_are_rendered_in_location_order() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error(Code::new("E0002"), "second").at(Location::new("a.zrk", 9, 1)));
        sink.emit(Diagnostic::error(Code::new("E0001"), "first").at(Location::new("a.zrk", 2, 1)));

        let rendered = sink.render(RenderStyle::Human);
        assert!(rendered.find("first").unwrap() < rendered.find("second").unwrap());
    }
}
