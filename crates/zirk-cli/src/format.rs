//! CLI I/O adapter for the shared, pure `zirk-format` engine.

use std::fs;
use std::path::Path;

pub enum Outcome {
    Unchanged,
    Changed,
    Invalid(String),
}

/// Result of formatting one already-resolved source path. The CLI owns target
/// discovery and I/O policy; this helper keeps every target independent.
pub fn format_path_with_diff(path: &Path, check: bool, diff: bool) -> Result<Outcome, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}\n", path.display()))?;
    let formatted = zirk_format::format(&path.display().to_string(), &text)?;
    if formatted == text {
        return Ok(Outcome::Unchanged);
    }
    if diff {
        print_simple_diff(path, &text, &formatted);
        return Ok(Outcome::Changed);
    }
    if check {
        return Ok(Outcome::Changed);
    }
    fs::write(path, formatted)
        .map_err(|error| format!("could not write {}: {error}\n", path.display()))?;
    Ok(Outcome::Changed)
}

fn print_simple_diff(path: &Path, before: &str, after: &str) {
    println!("--- {}", path.display());
    println!("+++ {}", path.display());
    for line in before.lines() {
        println!("-{line}");
    }
    for line in after.lines() {
        println!("+{line}");
    }
}

pub fn format_path(
    path: &Path,
    check: bool,
    _json: bool,
    _color: zirk_diagnostics::Color,
) -> Outcome {
    format_path_with_diff(path, check, false).unwrap_or_else(Outcome::Invalid)
}

/// Formats source held in memory — the entry point the language server uses,
/// where the text lives in the editor rather than on disk.
///
/// `name` only labels diagnostics. Returns the formatted text, or the
/// rendered diagnostics when the source does not parse (an unformattable
/// document is never rewritten halfway).
pub fn format_source(name: &str, text: &str) -> Result<String, String> {
    zirk_format::format(name, text)
}
pub fn format_text(text: &str, tokens: &[zirk_lexer::Token]) -> String {
    zirk_format::format_text(text, tokens)
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
