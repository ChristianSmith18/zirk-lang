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

use crate::color::Style;
use crate::{Color, Diagnostic, Severity};
use std::fmt::Write as _;

/// Requested output form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderStyle {
    /// Human-readable format.
    Human,
    /// Structured format for tooling (`--json`).
    Json,
}

pub(crate) fn human(diagnostic: &Diagnostic, color: Color) -> String {
    let mut out = String::new();

    let severity_style = match diagnostic.severity {
        Severity::Error => Style::Error,
        Severity::Warning => Style::Warning,
    };

    // error[E0403]: incompatible types
    let header = format!(
        "{}[{}]: {}",
        diagnostic.severity.as_str(),
        diagnostic.code,
        diagnostic.message
    );
    let _ = writeln!(out, "{}", color.paint(severity_style, &header));

    // The width of the left gutter depends on how many digits the line has.
    let gutter = diagnostic
        .location
        .as_ref()
        .map(|l| l.line.to_string().len())
        .unwrap_or(0);
    let pad = " ".repeat(gutter);
    let bar = color.paint(Style::Gutter, "|");

    if let Some(location) = &diagnostic.location {
        let where_ = format!("{}:{}:{}", location.file, location.line, location.column);
        let _ = writeln!(out, "{pad} {}", color.paint(Style::Location, &where_));

        // The snippet is optional: without source, the diagnostic keeps its
        // header, location, cause and help.
        if let Some(snippet) = &diagnostic.snippet {
            let _ = writeln!(out, "{pad} {bar}");

            let number = color.paint(Style::Gutter, &location.line.to_string());
            let line = highlight(&snippet.line_text, location.column, snippet.width, color);
            let _ = writeln!(out, "{number} {bar} {line}");

            let offset = " ".repeat(location.column.saturating_sub(1) as usize);
            let marker = color.paint(Style::Marker, &"^".repeat(snippet.width as usize));
            match &snippet.label {
                Some(label) => {
                    let label = color.paint(Style::Marker, label);
                    let _ = writeln!(out, "{pad} {bar} {offset}{marker} {label}");
                }
                None => {
                    let _ = writeln!(out, "{pad} {bar} {offset}{marker}");
                }
            }
            let _ = writeln!(out, "{pad} {bar}");
        }
    }

    if let Some(cause) = &diagnostic.cause {
        let _ = writeln!(
            out,
            "{pad} {} {cause}",
            color.paint(Style::Cause, "= cause:")
        );
    }
    if let Some(help) = &diagnostic.help {
        let _ = writeln!(out, "{pad} {} {help}", color.paint(Style::Help, "= help:"));
    }

    out
}

/// Emphasizes the fragment of the source line the diagnostic points at.
///
/// The line keeps its own reading and only the marked span is bold, so the eye
/// lands on the exact spot without the surrounding code losing legibility.
///
/// Columns and widths count Unicode characters, so the split is done over
/// characters and not bytes: cutting `ñ` in half would corrupt the output.
fn highlight(line: &str, column: u32, width: u32, color: Color) -> String {
    if !color.enabled() {
        return line.to_string();
    }

    let start = column.saturating_sub(1) as usize;
    let chars: Vec<char> = line.chars().collect();

    // A location past the end of the line is not an error worth aborting on:
    // the line is emitted as it is.
    if start >= chars.len() {
        return line.to_string();
    }

    let end = (start + width as usize).min(chars.len());

    let before: String = chars[..start].iter().collect();
    let marked: String = chars[start..end].iter().collect();
    let after: String = chars[end..].iter().collect();

    format!("{before}{}{after}", color.paint(Style::Marked, &marked))
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

pub(crate) fn sink(diagnostics: &[Diagnostic], style: RenderStyle, color: Color) -> String {
    // Diagnostics are sorted by location before rendering. Pipeline stages emit
    // in the order they work — all of the lexing first, then all of the
    // parsing — so without this an error on line 3 can appear before one on
    // line 2. Readers expect to walk their file from top to bottom.
    let mut sorted: Vec<&Diagnostic> = diagnostics.iter().collect();
    sorted.sort_by_key(|d| sort_key(d));

    match style {
        RenderStyle::Human => sorted
            .into_iter()
            .map(|d| human(d, color))
            .collect::<Vec<_>>()
            .join("\n"),
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
    use crate::{Code, Color, Diagnostic, DiagnosticSink, Location, RenderStyle, Snippet};

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

    // --- Colour -------------------------------------------------------------

    #[test]
    fn without_colour_the_output_carries_no_escape_sequences() {
        // This is the property that keeps the output deterministic when piped,
        // as `COMPILER_SPEC` section 9 requires.
        let plain = complete_diagnostic().render();
        assert!(!plain.contains('\x1b'), "{plain:?}");
    }

    #[test]
    fn colour_does_not_alter_the_text() {
        // Stripping the escape sequences must give exactly the plain rendering:
        // colour adds emphasis, it does not change what is said.
        let plain = complete_diagnostic().render();
        let colored = complete_diagnostic().render_colored(Color::Ansi);
        assert_eq!(strip(&colored), plain);
    }

    #[test]
    fn the_error_the_cause_and_the_help_are_distinguishable() {
        let colored = complete_diagnostic().render_colored(Color::Ansi);

        let line_of = |needle: &str| {
            colored
                .lines()
                .find(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("no line with `{needle}`"))
                .to_string()
        };

        let error = line_of("E1234");
        let cause = line_of("cause:");
        let help = line_of("help:");

        for line in [&error, &cause, &help] {
            assert!(line.contains('\x1b'), "uncoloured: {line:?}");
        }
        assert_ne!(prefix_of(&error), prefix_of(&cause));
        assert_ne!(prefix_of(&cause), prefix_of(&help));
    }

    #[test]
    fn the_marked_fragment_is_emphasized() {
        let colored = complete_diagnostic().render_colored(Color::Ansi);

        // The marker is one character wide at column 12, which lands inside
        // `problematic`. The word is therefore split by the emphasis, and
        // looking for it whole would not find it — which is exactly the point:
        // the bold covers the marked span and nothing more.
        let source_line = colored
            .lines()
            .find(|l| l.contains("expression"))
            .expect("the source line");

        assert!(
            source_line.contains("\x1b[1m"),
            "the marked fragment must be bold: {source_line:?}"
        );
        assert_eq!(
            strip(source_line)
                .trim_start_matches(['1', '8', ' ', '|'])
                .trim(),
            "problematic expression",
            "stripping the emphasis must give the original line back"
        );
    }

    #[test]
    fn a_marker_past_the_end_of_the_line_does_not_abort() {
        // A malformed location must not take the compiler down while it is
        // already reporting an error.
        let rendered = Diagnostic::error(Code::new("E0001"), "beyond")
            .at(Location::new("a.zrk", 1, 99))
            .with_snippet(Snippet::new("short", 3))
            .render_colored(Color::Ansi);

        assert!(rendered.contains("short"));
    }

    #[test]
    fn the_emphasis_respects_unicode() {
        // Splitting by bytes would cut `ñ` in half and corrupt the output.
        let rendered = Diagnostic::error(Code::new("E0001"), "unicode")
            .at(Location::new("a.zrk", 1, 5))
            .with_snippet(Snippet::new("mut ñandú = 1;", 5))
            .render_colored(Color::Ansi);

        assert!(rendered.contains("ñandú"), "{rendered:?}");
        assert!(strip(&rendered).contains("ñandú"));
    }

    #[test]
    fn the_structured_form_never_carries_colour() {
        // What tooling consumes must be parseable, and an escape sequence
        // inside a JSON string is not.
        let mut sink = DiagnosticSink::new();
        sink.emit(complete_diagnostic());

        let json = sink.render_colored(RenderStyle::Json, Color::Ansi);
        assert!(!json.contains('\x1b'), "{json:?}");
    }

    /// Removes the ANSI escape sequences from a text.
    fn strip(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();

        while let Some(c) = chars.next() {
            if c != '\x1b' {
                out.push(c);
                continue;
            }
            // A sequence runs until its final letter, `m` for the ones used here.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        }
        out
    }

    /// The escape sequence that opens a line, to compare roles.
    fn prefix_of(line: &str) -> String {
        line.chars()
            .skip_while(|c| *c != '\x1b')
            .take_while(|c| *c != 'm')
            .collect()
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
