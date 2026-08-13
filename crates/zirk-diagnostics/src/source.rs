//! Source handling: spans and their conversion into readable locations.
//!
//! This lives in this crate because it is what turns a compiler position into
//! the [`Location`](crate::Location) a diagnostic needs, and because every
//! stage of the pipeline needs it — the same reason `zirk-diagnostics` is the
//! one cross-cutting dependency that is allowed.

use crate::{Location, Snippet};

/// A byte range within a source file.
///
/// Byte offsets are stored rather than line/column because they are cheap to
/// produce and propagate; the conversion into readable coordinates happens only
/// when a diagnostic has to be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// An empty span at a position, useful to point at "something is missing
    /// here".
    pub const fn empty(at: u32) -> Self {
        Self { start: at, end: at }
    }

    /// A span covering from the start of this one to the end of the other.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

/// A source file together with its line index.
///
/// The index is computed once at construction: converting offsets into line and
/// column is a frequent operation on the error path, and walking the whole text
/// for every diagnostic does not scale.
#[derive(Debug, Clone)]
pub struct SourceFile {
    name: String,
    text: String,
    /// Byte offset where each line starts. Always begins with 0.
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0u32];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset as u32 + 1);
            }
        }
        Self {
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text covered by a span.
    pub fn slice(&self, span: Span) -> &str {
        let start = (span.start as usize).min(self.text.len());
        let end = (span.end as usize).min(self.text.len());
        // A malformed span must not abort the compiler in the middle of an
        // error report.
        if start > end {
            return "";
        }
        &self.text[start..end]
    }

    /// Zero-based index of the line containing an offset.
    fn line_index(&self, offset: u32) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next - 1,
        }
    }

    /// Converts an offset into a one-based line and column.
    ///
    /// The column counts Unicode characters, not bytes, as an editor expects.
    pub fn line_column(&self, offset: u32) -> (u32, u32) {
        let line = self.line_index(offset);
        let start = self.line_starts[line] as usize;
        let up_to = (offset as usize).min(self.text.len());
        let column = self.text[start..up_to].chars().count() as u32 + 1;
        (line as u32 + 1, column)
    }

    /// Text of a one-based line, without its trailing newline.
    pub fn line_text(&self, line: u32) -> &str {
        let index = (line.saturating_sub(1)) as usize;
        let Some(&start) = self.line_starts.get(index) else {
            return "";
        };
        let end = self
            .line_starts
            .get(index + 1)
            .map(|&s| s as usize)
            .unwrap_or(self.text.len());
        self.text[start as usize..end].trim_end_matches(['\n', '\r'])
    }

    /// Readable location of the start of a span.
    pub fn location(&self, span: Span) -> Location {
        let (line, column) = self.line_column(span.start);
        Location::new(self.name.clone(), line, column)
    }

    /// Source snippet for a span, clamped to its first line.
    ///
    /// A multi-line span is trimmed at the end of its starting line: the format
    /// in `COMPILER_SPEC` section 8 shows one line with a marker underneath, and
    /// a marker spanning several lines would have nowhere to be drawn.
    pub fn snippet(&self, span: Span) -> Snippet {
        let (line, column) = self.line_column(span.start);
        let text = self.line_text(line);

        let chars_to_end = text.chars().count() as u32 + 1 - column;
        let width = self
            .slice(span)
            .chars()
            .count()
            .max(1)
            .min(chars_to_end.max(1) as usize) as u32;

        Snippet::new(text, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> SourceFile {
        SourceFile::new(
            "test.zrk",
            "fn main(): Void {\n    stdout.println(\"hola\");\n}\n",
        )
    }

    #[test]
    fn first_position_is_line_one_column_one() {
        assert_eq!(file().line_column(0), (1, 1));
    }

    #[test]
    fn an_offset_converts_into_line_and_column() {
        let f = file();
        // The first byte of the second line comes after "fn main(): Void {\n".
        assert_eq!(f.line_column(18), (2, 1));
    }

    #[test]
    fn the_column_counts_characters_not_bytes() {
        let f = SourceFile::new("t.zrk", "mut café = 1;");
        // 'é' takes two bytes; '=' is at byte 10 but at column 10.
        let offset = f.text().find('=').unwrap() as u32;
        assert_eq!(offset, 10);
        assert_eq!(f.line_column(offset), (1, 10));
    }

    #[test]
    fn the_text_of_a_line_is_recovered() {
        assert_eq!(file().line_text(2), "    stdout.println(\"hola\");");
    }

    #[test]
    fn a_missing_line_does_not_abort() {
        assert_eq!(file().line_text(99), "");
    }

    #[test]
    fn a_span_slices_the_matching_text() {
        let f = SourceFile::new("t.zrk", "fn main");
        assert_eq!(f.slice(Span::new(3, 7)), "main");
    }

    #[test]
    fn an_out_of_range_span_does_not_abort() {
        let f = SourceFile::new("t.zrk", "abc");
        assert_eq!(f.slice(Span::new(1, 999)), "bc");
    }

    #[test]
    fn the_marker_does_not_run_past_the_end_of_the_line() {
        let f = SourceFile::new("t.zrk", "ab\ncd");
        // A span starting on the first line and extending beyond it.
        let s = f.snippet(Span::new(1, 5));
        assert_eq!(s.line_text, "ab");
        assert!(s.width <= 2);
    }

    #[test]
    fn two_spans_combine_covering_both() {
        assert_eq!(Span::new(2, 5).to(Span::new(8, 10)), Span::new(2, 10));
    }
}
