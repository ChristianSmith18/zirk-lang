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
    /// The file the offsets belong to.
    ///
    /// Offsets stay local to their file rather than living in one global
    /// address space: editing a file must not shift the spans of every file
    /// after it. See `docs/decisions/ADR-010-ubicaciones-multiarchivo.md`.
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

/// Identifies a file within a [`SourceMap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u32);

impl FileId {
    /// The first file, which is the entry point of a compilation.
    pub const ENTRY: FileId = FileId(0);
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self::in_file(FileId::ENTRY, start, end)
    }

    pub const fn in_file(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// An empty span at a position, useful to point at "something is missing
    /// here".
    pub const fn empty(at: u32) -> Self {
        Self::in_file(FileId::ENTRY, at, at)
    }

    pub const fn empty_in(file: FileId, at: u32) -> Self {
        Self::in_file(file, at, at)
    }

    /// A span covering from the start of this one to the end of the other.
    ///
    /// Both must belong to the same file: a range spanning two files names no
    /// text, and making that impossible to write is half the point of carrying
    /// the file at all (ADR-010).
    pub fn to(self, other: Span) -> Span {
        debug_assert_eq!(
            self.file, other.file,
            "spans from different files cannot be combined"
        );
        Span::in_file(
            self.file,
            self.start.min(other.start),
            self.end.max(other.end),
        )
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
    /// Which file this is within its [`SourceMap`].
    ///
    /// The file carries its own id so every stage that already holds the file
    /// can build spans for it without threading a second parameter alongside.
    id: FileId,
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
            // A file built on its own is the entry point until a `SourceMap`
            // says otherwise, which keeps single-file use unchanged.
            id: FileId::ENTRY,
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn id(&self) -> FileId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// A span within this file.
    pub fn span(&self, start: u32, end: u32) -> Span {
        Span::in_file(self.id, start, end)
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

/// The files of one compilation.
///
/// It owns every [`SourceFile`] and resolves a [`Span`] against the one its
/// [`FileId`] names, so no stage has to carry the right file alongside the
/// span. See `docs/decisions/ADR-010-ubicaciones-multiarchivo.md`.
#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file and returns the id that names it.
    ///
    /// The first file added is the entry point, and gets [`FileId::ENTRY`].
    pub fn add(&mut self, mut file: SourceFile) -> FileId {
        let id = FileId(self.files.len() as u32);
        file.id = id;
        self.files.push(file);
        id
    }

    pub fn file(&self, id: FileId) -> &SourceFile {
        &self.files[id.0 as usize]
    }

    pub fn files(&self) -> impl Iterator<Item = (FileId, &SourceFile)> {
        self.files
            .iter()
            .enumerate()
            .map(|(index, file)| (FileId(index as u32), file))
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Looks a file up by the name it was added with.
    pub fn find(&self, name: &str) -> Option<FileId> {
        self.files
            .iter()
            .position(|f| f.name() == name)
            .map(|index| FileId(index as u32))
    }

    pub fn location(&self, span: Span) -> Location {
        self.file(span.file).location(span)
    }

    pub fn snippet(&self, span: Span) -> Snippet {
        self.file(span.file).snippet(span)
    }

    pub fn slice(&self, span: Span) -> &str {
        self.file(span.file).slice(span)
    }
}

#[cfg(test)]
mod source_map_tests {
    use super::*;

    #[test]
    fn the_first_file_added_is_the_entry_point() {
        let mut map = SourceMap::new();
        let id = map.add(SourceFile::new("main.zrk", "fn main(): Void { }"));
        assert_eq!(id, FileId::ENTRY);
    }

    #[test]
    fn a_span_resolves_against_its_own_file() {
        let mut map = SourceMap::new();
        let first = map.add(SourceFile::new("a.zrk", "alpha"));
        let second = map.add(SourceFile::new("b.zrk", "beta"));

        // The same offsets in different files name different text: that is
        // exactly what the file id is for.
        assert_eq!(map.slice(Span::in_file(first, 0, 5)), "alpha");
        assert_eq!(map.slice(Span::in_file(second, 0, 4)), "beta");
    }

    #[test]
    fn a_location_names_the_file_it_belongs_to() {
        let mut map = SourceMap::new();
        map.add(SourceFile::new("a.zrk", "one\ntwo"));
        let second = map.add(SourceFile::new("b.zrk", "uno\ndos"));

        let location = map.location(Span::in_file(second, 4, 7));
        assert_eq!(location.file, "b.zrk");
        assert_eq!(location.line, 2);
    }

    #[test]
    fn a_file_can_be_found_by_name() {
        let mut map = SourceMap::new();
        map.add(SourceFile::new("a.zrk", ""));
        let second = map.add(SourceFile::new("b.zrk", ""));

        assert_eq!(map.find("b.zrk"), Some(second));
        assert_eq!(map.find("c.zrk"), None);
    }
}
