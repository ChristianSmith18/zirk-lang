//! The deliberately small first slice of `zirk format`.
//!
//! It validates the source before touching disk and only rewrites the layout
//! of triple-quoted strings. Keeping the transformation here, rather than in
//! the editor extension, makes the command the canonical formatter owner.

use std::fs;
use std::path::Path;

use zirk_diagnostics::{DiagnosticSink, SourceFile};
use zirk_lexer::TokenKind;

use crate::frontend;

pub enum Outcome {
    Unchanged,
    Changed,
    Invalid(String),
}

pub fn format_path(
    path: &Path,
    check: bool,
    json: bool,
    color: zirk_diagnostics::Color,
) -> Outcome {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Outcome::Invalid(format!("could not read {}: {error}\n", path.display()));
        }
    };

    let source = SourceFile::new(path.display().to_string(), text.clone());
    let mut sink = DiagnosticSink::new();
    let tokens = zirk_lexer::tokenize(&source, &mut sink);
    let _ = zirk_parser::parse(&source, &tokens, &mut sink);
    if sink.has_errors() {
        return Outcome::Invalid(frontend::render(&sink, json, color));
    }

    let formatted = format_text(&text, &tokens);
    if formatted == text {
        return Outcome::Unchanged;
    }
    if check {
        return Outcome::Changed;
    }
    match fs::write(path, formatted) {
        Ok(()) => Outcome::Changed,
        Err(error) => Outcome::Invalid(format!("could not write {}: {error}\n", path.display())),
    }
}

/// Formats source held in memory — the entry point the language server uses,
/// where the text lives in the editor rather than on disk.
///
/// `name` only labels diagnostics. Returns the formatted text, or the
/// rendered diagnostics when the source does not parse (an unformattable
/// document is never rewritten halfway).
pub fn format_source(name: &str, text: &str) -> Result<String, String> {
    let source = SourceFile::new(name.to_string(), text.to_string());
    let mut sink = DiagnosticSink::new();
    let tokens = zirk_lexer::tokenize(&source, &mut sink);
    let _ = zirk_parser::parse(&source, &tokens, &mut sink);
    if sink.has_errors() {
        return Err(frontend::render(
            &sink,
            false,
            zirk_diagnostics::Color::Never,
        ));
    }
    Ok(format_text(text, &tokens))
}

fn format_text(text: &str, tokens: &[zirk_lexer::Token]) -> String {
    let mut literals: Vec<(usize, usize)> = tokens
        .iter()
        .filter_map(|token| match token.kind {
            TokenKind::Str(_) | TokenKind::InterpolatedStr(_)
                if text[token.span.start as usize..].starts_with("'''") =>
            {
                Some((token.span.start as usize, token.span.end as usize))
            }
            _ => None,
        })
        .collect();
    literals.sort_unstable_by_key(|(start, _)| *start);

    let mut output = text.to_string();
    for (start, end) in literals.into_iter().rev() {
        let replacement = canonical_literal(&output, start, end);
        let line_start = output[..start].rfind('\n').map_or(0, |offset| offset + 1);
        let replace_start = if output[line_start..start].trim().is_empty() {
            line_start
        } else {
            output[line_start..start]
                .trim_end_matches([' ', '\t'])
                .len()
                + line_start
        };
        output.replace_range(replace_start..end, &replacement);
    }
    output
}

fn canonical_literal(text: &str, start: usize, end: usize) -> String {
    let open_line_start = text[..start].rfind('\n').map_or(0, |offset| offset + 1);
    let open_prefix = &text[open_line_start..start];
    let opening_is_own_line = open_prefix.trim().is_empty();
    let introducer_line_start = if opening_is_own_line {
        text[..open_line_start.saturating_sub(1)]
            .rfind('\n')
            .map_or(0, |offset| offset + 1)
    } else {
        open_line_start
    };
    let introducer_indent: String = text[introducer_line_start..]
        .chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .collect();
    let indent_unit = if introducer_indent.contains('\t') {
        "\t"
    } else {
        "    "
    };
    let edge = format!("{introducer_indent}{indent_unit}");
    let old_edge = open_prefix.chars().count();

    let body = &text[start + 3..end - 3];
    // The final line is the indentation before the closing delimiter; neither
    // it nor its preceding newline belong to the literal value.
    let body = body.rsplit_once('\n').map_or(body, |(content, _)| content);
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
        .unwrap_or(body);
    let body = body
        .strip_suffix("\r\n")
        .or_else(|| body.strip_suffix('\n'))
        .unwrap_or(body);
    let content = body
        .split('\n')
        .map(|line| canonical_content_line(line.trim_end_matches('\r'), old_edge, &edge))
        .collect::<Vec<_>>()
        .join("\n");

    let opening = if opening_is_own_line {
        edge.clone()
    } else {
        format!("\n{edge}")
    };
    if content.is_empty() {
        format!("{opening}'''\n{edge}'''")
    } else {
        format!("{opening}'''\n{content}\n{edge}'''")
    }
}

fn canonical_content_line(line: &str, old_edge: usize, edge: &str) -> String {
    if line.trim().is_empty() {
        return String::new();
    }
    let rest: String = line.chars().skip(old_edge).collect();
    format!("{edge}{rest}")
}

#[cfg(test)]
mod tests {
    use super::format_text;
    use zirk_diagnostics::{DiagnosticSink, SourceFile};

    fn format(source: &str) -> String {
        let file = SourceFile::new("test.zrk", source);
        let mut sink = DiagnosticSink::new();
        let tokens = zirk_lexer::tokenize(&file, &mut sink);
        assert!(!sink.has_errors());
        format_text(source, &tokens)
    }

    #[test]
    fn moves_an_inline_literal_below_its_introducer() {
        assert_eq!(
            format(
                "fn main(): Void {\n    inmut text = '''\n                 hello\n                 ''';\n}\n"
            ),
            "fn main(): Void {\n    inmut text =\n        '''\n        hello\n        ''';\n}\n"
        );
    }

    #[test]
    fn formatting_is_idempotent() {
        let canonical =
            "fn main(): Void {\n    inmut text =\n        '''\n        hello\n        ''';\n}\n";
        assert_eq!(format(&format(canonical)), canonical);
    }

    #[test]
    fn format_source_formats_in_memory_text() {
        let formatted = super::format_source(
            "mem.zrk",
            "fn main(): Void {\n    inmut text = '''\n                 hello\n                 ''';\n}\n",
        )
        .unwrap();
        assert_eq!(
            formatted,
            "fn main(): Void {\n    inmut text =\n        '''\n        hello\n        ''';\n}\n"
        );
    }

    #[test]
    fn format_source_rejects_unparseable_text() {
        assert!(super::format_source("mem.zrk", "fn broken( {").is_err());
    }
}
