//! # zirk-diagnostics
//!
//! **Responsibility:** define the shape of a compiler diagnostic and render it,
//! both for humans and for tooling.
//!
//! Implements the contract of `ZIRK_COMPILER_SPEC.md` section 8: every
//! diagnostic carries a severity, a stable code, a location, a cause and —when
//! a clear fix exists— actionable help.
//!
//! **Boundary:** this crate does not know Zirk syntax, does not read files and
//! does not decide when to emit a diagnostic. It only represents and formats
//! one. Whoever detects the error is responsible for building it and for
//! supplying the source snippet if it has one.
//!
//! It is the one cross-cutting dependency the workspace allows: any pipeline
//! stage may depend on it, regardless of its position.

mod color;
mod render;
mod source;

pub use color::Color;
pub use render::RenderStyle;
pub use source::{SourceFile, Span};

/// Severity of a diagnostic.
///
/// Warnings never alter program semantics; they only inform. Promoting them to
/// errors is a configuration decision, not one made at the emission site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    /// Label used in both the human-readable and the structured output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// Stable code of a diagnostic.
///
/// A published code is never reused for a semantically different error: tooling,
/// documentation and suppressions depend on that stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(&'static str);

impl Code {
    pub const fn new(code: &'static str) -> Self {
        Self(code)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Location of a diagnostic in the source.
///
/// `line` and `column` are one-based, as any editor expects. `column` counts
/// Unicode characters, not bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl Location {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

/// Source snippet accompanying the diagnostic.
///
/// It is optional on purpose: a diagnostic emitted without access to the source
/// is still valid and must render while preserving the rest of its information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    /// Full text of the reported line, without its trailing newline.
    pub line_text: String,
    /// How many characters the marker spans. Always at least 1.
    pub width: u32,
    /// Localized explanation printed next to the marker.
    pub label: Option<String>,
}

impl Snippet {
    pub fn new(line_text: impl Into<String>, width: u32) -> Self {
        Self {
            line_text: line_text.into(),
            width: width.max(1),
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Code,
    /// Precise description, shown in the header.
    pub message: String,
    pub location: Option<Location>,
    pub snippet: Option<Snippet>,
    /// Semantic reason for the error.
    pub cause: Option<String>,
    /// Concrete fix. Omitted when there is no clear fix: generic help with no
    /// actionable value is worse than none.
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    pub fn warning(code: Code, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }

    pub fn new(severity: Severity, code: Code, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            location: None,
            snippet: None,
            cause: None,
            help: None,
        }
    }

    pub fn at(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_snippet(mut self, snippet: Snippet) -> Self {
        self.snippet = Some(snippet);
        self
    }

    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Renders in the human-readable format of `COMPILER_SPEC` section 8.
    ///
    /// Without colour: it is the form tooling and tests read, and the one that
    /// keeps the output deterministic when it is piped.
    pub fn render(&self) -> String {
        render::human(self, Color::Never)
    }

    /// Renders in the human-readable format, with colour.
    pub fn render_colored(&self, color: Color) -> String {
        render::human(self, color)
    }

    /// Renders as a JSON object for consumption by tooling.
    pub fn render_json(&self) -> String {
        render::json(self)
    }

    /// Moves the diagnostic to the heap so it can be returned in a
    /// [`DiagnosticResult`].
    pub fn boxed(self) -> Box<Self> {
        Box::new(self)
    }
}

/// Result of an operation that can fail with a diagnostic.
///
/// The error goes on the heap on purpose: `Diagnostic` is around 200 bytes and
/// this alias appears in the return type of nearly every compiler operation.
/// Without the `Box`, the success path pays that size on every call.
pub type DiagnosticResult<T> = Result<T, Box<Diagnostic>>;

/// Accumulates diagnostics and applies the severity policy.
///
/// This is the only place where a warning can become an error: emission sites
/// declare the natural severity of the finding and know nothing of the policy.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticSink {
    diagnostics: Vec<Diagnostic>,
    warnings_as_errors: bool,
}

impl DiagnosticSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Promotes warnings to errors, per `--warnings-as-errors`.
    pub fn warnings_as_errors(mut self, enabled: bool) -> Self {
        self.warnings_as_errors = enabled;
        self
    }

    pub fn emit(&mut self, mut diagnostic: Diagnostic) {
        if self.warnings_as_errors && diagnostic.severity == Severity::Warning {
            diagnostic.severity = Severity::Error;
        }
        self.diagnostics.push(diagnostic);
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// If there is at least one error, compilation cannot continue.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Renders every accumulated diagnostic, without colour.
    pub fn render(&self, style: RenderStyle) -> String {
        render::sink(&self.diagnostics, style, Color::Never)
    }

    /// Renders every accumulated diagnostic, with colour.
    ///
    /// Colour never reaches the structured form: what a tool consumes must be
    /// parseable, and an escape sequence inside a JSON string is not.
    pub fn render_colored(&self, style: RenderStyle, color: Color) -> String {
        let color = match style {
            RenderStyle::Human => color,
            RenderStyle::Json => Color::Never,
        };
        render::sink(&self.diagnostics, style, color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_as_errors_promotes_the_severity() {
        let mut sink = DiagnosticSink::new().warnings_as_errors(true);
        sink.emit(Diagnostic::warning(Code::new("W0001"), "unused symbol"));

        assert_eq!(sink.diagnostics()[0].severity, Severity::Error);
        assert!(sink.has_errors());
    }

    #[test]
    fn without_the_flag_a_warning_does_not_stop_compilation() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::warning(Code::new("W0001"), "unused symbol"));

        assert_eq!(sink.diagnostics()[0].severity, Severity::Warning);
        assert!(!sink.has_errors());
    }

    #[test]
    fn errors_always_stop_compilation() {
        let mut sink = DiagnosticSink::new();
        sink.emit(Diagnostic::error(Code::new("E0001"), "incompatible type"));

        assert!(sink.has_errors());
    }

    #[test]
    fn a_snippet_has_a_marker_of_at_least_one_character() {
        assert_eq!(Snippet::new("texto", 0).width, 1);
    }
}
