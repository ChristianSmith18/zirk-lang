//! # zirk-lexer
//!
//! **Responsibility:** turn `.zrk` source text into a sequence of tokens with
//! their locations, and report the lexical errors it finds.
//!
//! **Boundary:** the lexer does not know the grammar. It does not decide
//! whether a sequence of tokens is valid, only whether each token is. The
//! optional semicolon of `ZIRK_LANGUAGE_SPEC.md` section 1 is the parser's
//! problem, not this crate's.
//!
//! `ZIRK_COMPILER_SPEC.md` section 2 describes it as incremental: it must be
//! able to re-scan only the region affected by an edit. That capability arrives
//! with incremental compilation, but the API must not close the door on it.
//!
//! # Error recovery
//!
//! The lexer **does not stop at the first error**. It emits the diagnostic,
//! moves on and keeps tokenizing, so one compilation reports every lexical
//! problem at once instead of one per run.

mod token;

pub use token::{
    DurationUnit, FLOAT_WIDTHS, Keyword, NumberLit, OLD_FLOAT_WIDTHS, StrPart, Token, TokenKind,
};

use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};

/// A comment retained by [`tokenize_with_trivia`]. Whitespace deliberately
/// remains implicit, so the normal lexer hot path and token contract stay
/// unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub text: String,
    pub span: Span,
}

/// Diagnostic codes of the lexer.
pub mod codes {
    use zirk_diagnostics::Code;

    /// A character that starts no valid token.
    pub const UNRECOGNIZED_CHARACTER: Code = Code::new("E0201");
    /// String literal without a closing quote.
    pub const UNTERMINATED_STRING: Code = Code::new("E0202");
    /// Unrecognized escape sequence inside a string.
    pub const UNKNOWN_ESCAPE: Code = Code::new("E0203");
    /// Block comment without a closing delimiter.
    pub const UNTERMINATED_COMMENT: Code = Code::new("E0204");
    /// `_` separator in an invalid position of a numeric literal.
    pub const INVALID_SEPARATOR: Code = Code::new("E0205");
    /// Alphabetic characters attached to a numeric literal.
    pub const INVALID_NUMERIC_SUFFIX: Code = Code::new("E0207");
    /// Integer literal exceeding the internal representation.
    pub const INTEGER_TOO_LARGE: Code = Code::new("E0206");
    /// Character literal without a closing quote.
    pub const UNTERMINATED_CHARACTER: Code = Code::new("E0208");
    /// Regex literal without a closing delimiter.
    pub const UNTERMINATED_REGEX: Code = Code::new("E0209");
    /// Interpolation inside a string that is never closed.
    pub const UNTERMINATED_INTERPOLATION: Code = Code::new("E0210");
    /// A `'''` content line that starts left of the literal's left edge.
    pub const STRING_CONTENT_LEFT_OF_EDGE: Code = Code::new("E0211");
    /// A closing `'''` that starts left of the literal's left edge.
    pub const STRING_CLOSING_LEFT_OF_EDGE: Code = Code::new("E0212");
    /// A closing `'''` that does not start its own line.
    pub const STRING_CLOSING_MID_LINE: Code = Code::new("E0213");
}

/// Turns a source file into tokens, accumulating any errors found.
pub fn tokenize(source: &SourceFile, sink: &mut DiagnosticSink) -> Vec<Token> {
    Lexer::new(source, sink).run()
}

/// Tokenizes normally and additionally returns comments in source order.
/// This additive API is intended for tools such as the formatter; callers
/// that do not need trivia should keep using [`tokenize`].
pub fn tokenize_with_trivia(
    source: &SourceFile,
    sink: &mut DiagnosticSink,
) -> (Vec<Token>, Vec<Comment>) {
    let tokens = tokenize(source, sink);
    let text = source.text();
    let mut comments = Vec::new();
    let mut offset = 0usize;
    while offset < text.len() {
        let rest = &text[offset..];
        let relative = match (rest.find("//"), rest.find("/*")) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => break,
        };
        let start = offset + relative;
        if tokens
            .iter()
            .any(|token| token.span.start as usize <= start && start < token.span.end as usize)
        {
            offset = start + 2;
            continue;
        }
        let line = text[start..].starts_with("//");
        let end = if line {
            text[start..].find('\n').map_or(text.len(), |n| start + n)
        } else if let Some(close) = text[start + 2..].find("*/") {
            start + 2 + close + 2
        } else {
            text.len()
        };
        comments.push(Comment {
            text: text[start..end].into(),
            span: source.span(start as u32, end as u32),
        });
        offset = end.max(start + 2);
    }
    (tokens, comments)
}

#[cfg(test)]
mod trivia_tests {
    use super::*;

    #[test]
    fn trivia_mode_returns_comments_in_source_order() {
        let source = SourceFile::new("comments.zrk", "// first\nfn main(): Void { /* second */ }");
        let mut sink = DiagnosticSink::new();
        let (_, comments) = tokenize_with_trivia(&source, &mut sink);
        assert!(!sink.has_errors());
        assert_eq!(
            comments.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
            ["// first", "/* second */"]
        );
    }
}

struct Lexer<'a> {
    source: &'a SourceFile,
    sink: &'a mut DiagnosticSink,
    /// The text as a vector of characters with their byte offsets, so we can
    /// advance over Unicode characters without losing the byte position.
    ///
    /// It costs roughly eight times the size of the source in memory. That is
    /// acceptable for single files and will be revisited once incremental
    /// compilation exists, where the latency goal of `COMPILER_SPEC` section 1
    /// makes the cost relevant.
    chars: Vec<(u32, char)>,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a SourceFile, sink: &'a mut DiagnosticSink) -> Self {
        let chars = source
            .text()
            .char_indices()
            .map(|(offset, c)| (offset as u32, c))
            .collect();
        Self {
            source,
            sink,
            chars,
            pos: 0,
        }
    }

    // --- Navigation -------------------------------------------------------

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).map(|&(_, c)| c)
    }

    fn peek_at(&self, lookahead: usize) -> Option<char> {
        self.chars.get(self.pos + lookahead).map(|&(_, c)| c)
    }

    /// Byte offset of the current position, or the end of the text.
    fn offset(&self) -> u32 {
        self.chars
            .get(self.pos)
            .map(|&(o, _)| o)
            .unwrap_or(self.source.text().len() as u32)
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    /// Consumes the character if it matches the expected one.
    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // --- Diagnostics ------------------------------------------------------

    fn error(&mut self, code: Code, span: Span, message: impl Into<String>) -> PartialDiagnostic {
        PartialDiagnostic {
            diagnostic: Diagnostic::error(code, message)
                .at(self.source.location(span))
                .with_snippet(self.source.snippet(span)),
        }
    }

    fn emit(&mut self, partial: PartialDiagnostic) {
        self.sink.emit(partial.diagnostic);
    }

    // --- Main loop --------------------------------------------------------

    fn run(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            self.skip_trivia();
            let start = self.offset();

            let Some(c) = self.peek() else {
                tokens.push(Token::new(TokenKind::Eof, self.source.span(start, start)));
                break;
            };

            // `re'...'` before the word recognizer, or `re` would be an
            // identifier and the pattern a broken character literal.
            let kind = if c == 'r' && self.peek_at(1) == Some('e') && self.peek_at(2) == Some('\'')
            {
                self.regex()
            } else if c.is_alphabetic() || c == '_' {
                Some(self.word())
            } else if c.is_ascii_digit() {
                self.number()
            } else if c == '"' {
                self.string()
            } else if c == '\'' {
                // `'''` opens a multiline literal; anything shorter is a
                // character literal. The longest match wins, as always.
                if self.peek_at(1) == Some('\'') && self.peek_at(2) == Some('\'') {
                    self.triple_string()
                } else {
                    self.character()
                }
            } else {
                self.punctuation()
            };

            if let Some(kind) = kind {
                tokens.push(Token::new(kind, self.source.span(start, self.offset())));
            }
        }

        tokens
    }

    /// Skips whitespace and comments.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.pos += 1;
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    self.block_comment();
                }
                _ => return,
            }
        }
    }

    /// Block comment.
    ///
    /// **They do not nest**: `/* /* */` closes at the first `*/`, as in C. The
    /// spec does not define this, so the most widespread convention is chosen
    /// and pinned by a test so it cannot change by accident.
    fn block_comment(&mut self) {
        let start = self.offset();
        self.pos += 2; // `/*`

        loop {
            match self.peek() {
                None => {
                    let span = self.source.span(start, start + 2);
                    let d = self.error(
                        codes::UNTERMINATED_COMMENT,
                        span,
                        "unterminated block comment",
                    );
                    self.emit(
                        d.with_cause(
                            "the comment opens here and the file ends before it is closed",
                        )
                        .with_help("add `*/` where the comment should end"),
                    );
                    return;
                }
                Some('*') if self.peek_at(1) == Some('/') => {
                    self.pos += 2;
                    return;
                }
                Some(_) => {
                    self.pos += 1;
                }
            }
        }
    }

    // --- Recognizers ------------------------------------------------------

    /// Identifier or keyword.
    fn word(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        let text: String = self.chars[start..self.pos]
            .iter()
            .map(|&(_, c)| c)
            .collect();

        match Keyword::from_text(&text) {
            Some(k) => TokenKind::Keyword(k),
            None => TokenKind::Identifier(text),
        }
    }

    /// A numeric literal: integer, float or duration.
    ///
    /// The forms of `docs/handbook/11-reference/04-literals.md`: decimal,
    /// hexadecimal and binary integers, fractional and scientific floats with
    /// an optional width suffix, and durations with a unit suffix. `_` is a
    /// separator in all of them.
    fn number(&mut self) -> Option<TokenKind> {
        let start_offset = self.offset();

        // A base prefix decides everything that follows, so it is checked first.
        if self.peek() == Some('0')
            && let Some(radix) = self.peek_at(1).and_then(Radix::from_prefix)
        {
            self.pos += 2;
            return self.radix_integer(radix, start_offset);
        }

        let mut text = self.digits(10, start_offset)?;

        // A fraction needs a digit after the point. Without that check `0..10`
        // would read as `0.` followed by `.10`, and `1.abs()` as a malformed
        // literal instead of a method call.
        let fractional =
            self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit());
        if fractional {
            self.pos += 1;
            text.push('.');
            text.push_str(&self.digits(10, start_offset)?);
        }

        let exponent = self.exponent(start_offset)?;
        if let Some(exponent) = &exponent {
            text.push_str(exponent);
        }

        let is_float = fractional || exponent.is_some();
        self.finish_number(text, is_float, start_offset)
    }

    /// Digits of the given base, with `_` allowed between them.
    ///
    /// Returns `None` after reporting a malformed literal, so the caller stops
    /// rather than building a token from digits it already knows are wrong.
    fn digits(&mut self, radix: u32, start_offset: u32) -> Option<String> {
        let mut digits = String::new();
        let mut last_was_separator = false;

        while let Some(c) = self.peek() {
            if c.is_digit(radix) {
                digits.push(c);
                last_was_separator = false;
                self.pos += 1;
            } else if c == '_' {
                // Two in a row, or a leading one, are invalid.
                if last_was_separator || digits.is_empty() {
                    let at = self.source.span(self.offset(), self.offset() + 1);
                    self.malformed_separator(at);
                    return None;
                }
                last_was_separator = true;
                self.pos += 1;
            } else {
                break;
            }
        }

        if last_was_separator {
            let at = self.source.span(self.offset() - 1, self.offset());
            self.malformed_separator(at);
            return None;
        }

        if digits.is_empty() {
            let span = self
                .source
                .span(start_offset, self.offset().max(start_offset + 1));
            let d = self.error(codes::INVALID_SEPARATOR, span, "malformed numeric literal");
            self.emit(
                d.with_cause("the literal has no digits")
                    .with_help("write at least one digit"),
            );
            return None;
        }

        Some(digits)
    }

    fn malformed_separator(&mut self, at: Span) {
        let d = self.error(codes::INVALID_SEPARATOR, at, "malformed numeric literal");
        self.emit(
            d.with_cause("`_` may only appear between digits")
                .with_help("write the number as `1_000_000`"),
        );
    }

    /// The `e`/`E` exponent of a scientific literal, if one follows.
    ///
    /// Returns `Some(None)` when there is no exponent, and `None` when there
    /// was one but it was malformed.
    fn exponent(&mut self, start_offset: u32) -> Option<Option<String>> {
        if !matches!(self.peek(), Some('e' | 'E')) {
            return Some(None);
        }

        // `1e2` is scientific; `1em` is a number with a bad suffix. The sign
        // and at least one digit are what tell them apart.
        let sign = matches!(self.peek_at(1), Some('+' | '-'));
        let digit_at = if sign { 2 } else { 1 };
        if !self.peek_at(digit_at).is_some_and(|c| c.is_ascii_digit()) {
            return Some(None);
        }

        let mut text = String::from("e");
        self.pos += 1;
        if sign {
            text.push(self.advance().expect("the sign was peeked"));
        }
        let digits = self.digits(10, start_offset)?;
        text.push_str(&digits);

        Some(Some(text))
    }

    /// Reads the suffix, if any, and builds the token it implies.
    fn finish_number(
        &mut self,
        text: String,
        is_float: bool,
        start_offset: u32,
    ) -> Option<TokenKind> {
        let suffix_start = self.offset();
        while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            self.pos += 1;
        }
        let suffix: String = self
            .source
            .slice(self.source.span(suffix_start, self.offset()))
            .to_string();

        if suffix.is_empty() {
            return if is_float {
                Some(TokenKind::Float(NumberLit::new(text)))
            } else {
                self.integer_token(&text, 10, start_offset)
            };
        }

        // An `f`/`f16`/`f32`/`f64`/`f128` suffix marks a binary `Float`
        // literal — on a fractional or an integer-valued mantissa alike.
        if FLOAT_WIDTHS.contains(&suffix.as_str()) {
            return Some(TokenKind::Float(NumberLit::new(text).with_width(suffix)));
        }

        // The old `b`/`bN` spellings get a targeted migration diagnostic
        // rather than the generic invalid-suffix error.
        if OLD_FLOAT_WIDTHS.contains(&suffix.as_str()) {
            let span = self.source.span(suffix_start, self.offset());
            let d = self.error(
                codes::INVALID_NUMERIC_SUFFIX,
                span,
                format!("invalid suffix on numeric literal: `{suffix}`"),
            );
            self.emit(
                d.with_cause("the binary-float suffix is now `f`/`fN`")
                    .with_help(format!("write `{text}f` or `{text}fN` instead")),
            );
            return None;
        }

        if let Some(unit) = DurationUnit::from_text(&suffix) {
            return Some(TokenKind::Duration(NumberLit::new(text), unit));
        }

        let span = self.source.span(suffix_start, self.offset());
        let d = self.error(
            codes::INVALID_NUMERIC_SUFFIX,
            span,
            format!("invalid suffix on numeric literal: `{suffix}`"),
        );
        self.emit(
            d.with_cause("it is neither a binary-float suffix (`f32`) nor a duration unit (`ms`)")
                .with_help("separate the number from the identifier with a space or an operator"),
        );
        None
    }

    /// A literal written in a base other than ten.
    fn radix_integer(&mut self, radix: Radix, start_offset: u32) -> Option<TokenKind> {
        let digits = self.digits(radix.value(), start_offset)?;

        // A suffix on a based literal is always a mistake: widths belong to
        // floats and duration units to durations.
        if self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            let suffix_start = self.offset();
            while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                self.pos += 1;
            }
            let span = self.source.span(suffix_start, self.offset());
            let text = self.source.slice(span).to_string();
            let d = self.error(
                codes::INVALID_NUMERIC_SUFFIX,
                span,
                format!("invalid suffix on numeric literal: `{text}`"),
            );
            self.emit(d.with_cause(format!(
                "a {} literal only accepts its digits and the `_` separator",
                radix.name()
            )));
            return None;
        }

        self.integer_token(&digits, radix.value(), start_offset)
    }

    /// Builds the integer token, reporting a value the compiler cannot hold.
    fn integer_token(&mut self, digits: &str, radix: u32, start_offset: u32) -> Option<TokenKind> {
        match i128::from_str_radix(digits, radix) {
            Ok(value) => Some(TokenKind::Integer(value)),
            Err(_) => {
                let span = self.source.span(start_offset, self.offset());
                let d = self.error(
                    codes::INTEGER_TOO_LARGE,
                    span,
                    "integer literal is too large",
                );
                self.emit(d.with_cause(
                    "the value exceeds the largest integer the compiler can represent",
                ));
                None
            }
        }
    }

    /// String literal with escapes.
    fn string(&mut self) -> Option<TokenKind> {
        let start_offset = self.offset();
        self.pos += 1; // opening quote

        let mut value = String::new();
        let mut parts: Vec<StrPart> = Vec::new();

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = self.source.span(start_offset, start_offset + 1);
                    let d = self.error(
                        codes::UNTERMINATED_STRING,
                        span,
                        "unterminated string literal",
                    );
                    self.emit(
                        d.with_cause(
                            "the string opens here and is not closed before the end of the line",
                        )
                        .with_help("add the closing `\"` quote"),
                    );
                    return None;
                }
                Some('"') => {
                    self.pos += 1;
                    if parts.is_empty() {
                        return Some(TokenKind::Str(canonical(value)));
                    }
                    if !value.is_empty() {
                        parts.push(StrPart::Literal(canonical(value)));
                    }
                    return Some(TokenKind::InterpolatedStr(parts));
                }
                Some('{') => {
                    let part = self.interpolation()?;
                    if !value.is_empty() {
                        parts.push(StrPart::Literal(canonical(std::mem::take(&mut value))));
                    }
                    parts.push(part);
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.escape('"') {
                        Some(c) => value.push(c),
                        None => continue,
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// Triple-quoted multiline string literal `'''…'''`.
    ///
    /// The column of the opening `'''` is the literal's left edge: every
    /// non-empty content line must begin at or to the right of it, and
    /// exactly that many leading whitespace characters are stripped from
    /// each line. Empty or whitespace-only lines contribute a bare `\n` and
    /// carry no indentation requirement. The closing `'''` must sit on its
    /// own line at or right of the edge, and its line contributes nothing —
    /// its preceding newline included.
    ///
    /// The token shapes are the same `Str`/`InterpolatedStr` a `"…"` literal
    /// produces, so nothing downstream needs to know how the text was
    /// written.
    fn triple_string(&mut self) -> Option<TokenKind> {
        let start_offset = self.offset();

        // The edge is the opening delimiter's column, in characters (a tab
        // counts as one): the count of characters back to the last newline.
        let mut line_start = self.pos;
        while line_start > 0 && self.chars[line_start - 1].1 != '\n' {
            line_start -= 1;
        }
        let edge = self.pos - line_start;

        self.pos += 3; // opening `'''`

        // `''''''` is the empty literal — the only inline form.
        if self.peek() == Some('\'')
            && self.peek_at(1) == Some('\'')
            && self.peek_at(2) == Some('\'')
        {
            self.pos += 3;
            return Some(TokenKind::Str(String::new()));
        }

        // The newline right after the opening delimiter is not content, and
        // the line that follows is the first one the edge applies to.
        let saw_opening_newline = self.eat('\n');

        let mut value = String::new();
        let mut parts: Vec<StrPart> = Vec::new();
        // Whitespace at the start of the current line, held back until the
        // line proves to hold content — the edge is stripped from it then.
        let mut indent = String::new();
        let mut at_line_start = saw_opening_newline;
        let mut saw_newline = saw_opening_newline;
        let mut line_offset = self.offset();

        loop {
            match self.peek() {
                None => {
                    let span = self.source.span(start_offset, start_offset + 3);
                    let d = self.error(
                        codes::UNTERMINATED_STRING,
                        span,
                        "unterminated string literal",
                    );
                    self.emit(
                        d.with_cause("the `'''` literal opens here and is never closed")
                            .with_help("add the closing `'''` on its own line"),
                    );
                    return None;
                }
                Some('\n') => {
                    // The line ends here; if it held only whitespace it was
                    // empty, and empty lines are exempt from the edge rule.
                    self.pos += 1;
                    value.push('\n');
                    indent.clear();
                    at_line_start = true;
                    saw_newline = true;
                    line_offset = self.offset();
                }
                Some('\'') if self.peek_at(1) == Some('\'') && self.peek_at(2) == Some('\'') => {
                    // The first `'''` run closes.
                    if !saw_newline || !at_line_start {
                        let span = self.source.span(self.offset(), self.offset() + 3);
                        let d = self.error(
                            codes::STRING_CLOSING_MID_LINE,
                            span,
                            "the closing `'''` must be on its own line",
                        );
                        self.emit(
                            d.with_cause("other content shares its line")
                                .with_help("move the closing `'''` to the start of a line"),
                        );
                    } else if indent.chars().count() < edge {
                        let span = self.source.span(line_offset, self.offset());
                        let d = self.error(
                            codes::STRING_CLOSING_LEFT_OF_EDGE,
                            span,
                            "the closing `'''` escapes the literal's left edge",
                        );
                        self.emit(
                            d.with_cause(format!(
                                "it is indented {} characters, fewer than the {edge} of the opening `'''`",
                                indent.chars().count()
                            ))
                            .with_help(format!(
                                "indent the closing `'''` at least {edge} characters"
                            )),
                        );
                    } else {
                        // A proper closing line contributes nothing —
                        // including the newline that ended the last content
                        // line, which is always the last character pushed.
                        value.pop();
                    }
                    self.pos += 3;
                    if parts.is_empty() {
                        return Some(TokenKind::Str(canonical(value)));
                    }
                    if !value.is_empty() {
                        parts.push(StrPart::Literal(canonical(value)));
                    }
                    return Some(TokenKind::InterpolatedStr(parts));
                }
                Some(c @ (' ' | '\t')) if at_line_start => {
                    indent.push(c);
                    self.pos += 1;
                }
                Some('{') => {
                    if at_line_start {
                        self.commit_indent(&mut indent, edge, &mut value, line_offset);
                        at_line_start = false;
                    }
                    let part = self.interpolation()?;
                    if !value.is_empty() {
                        parts.push(StrPart::Literal(canonical(std::mem::take(&mut value))));
                    }
                    parts.push(part);
                }
                Some('\\') => {
                    if at_line_start {
                        self.commit_indent(&mut indent, edge, &mut value, line_offset);
                        at_line_start = false;
                    }
                    self.pos += 1;
                    match self.escape('\'') {
                        Some(c) => value.push(c),
                        None => continue,
                    }
                }
                Some(c) => {
                    if at_line_start {
                        self.commit_indent(&mut indent, edge, &mut value, line_offset);
                        at_line_start = false;
                    }
                    value.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// Commits a content line's leading whitespace under the left-edge rule:
    /// the first `edge` characters are stripped and the rest are content. A
    /// non-empty line indented less than the edge escapes the literal's
    /// boundary and is diagnosed; its text is kept so scanning can continue.
    fn commit_indent(
        &mut self,
        indent: &mut String,
        edge: usize,
        value: &mut String,
        line_offset: u32,
    ) {
        let width = indent.chars().count();
        if width >= edge {
            let rest: String = indent.chars().skip(edge).collect();
            value.push_str(&rest);
        } else {
            let end = self.offset();
            let span = if indent.is_empty() {
                let char_len = self.peek().map(|c| c.len_utf8() as u32).unwrap_or(1);
                self.source.span(end, end + char_len)
            } else {
                self.source.span(line_offset, end)
            };
            let d = self.error(
                codes::STRING_CONTENT_LEFT_OF_EDGE,
                span,
                "content escapes the literal's left edge",
            );
            self.emit(
                d.with_cause(format!(
                    "the line is indented {width} characters, fewer than the {edge} of the opening `'''`"
                ))
                .with_help(format!(
                    "indent the line at least {edge} characters, or run `zirk format`"
                )),
            );
            value.push_str(indent);
        }
        indent.clear();
    }

    /// One `{ expression }` inside a string literal.
    ///
    /// Braces are counted so an expression may contain its own — a lambda body
    /// or a nested literal — and the interpolation still closes where it
    /// should rather than at the first `}`.
    fn interpolation(&mut self) -> Option<StrPart> {
        let open_offset = self.offset();
        self.pos += 1; // `{`

        let text_start = self.offset();
        let mut depth = 1usize;

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = self.source.span(open_offset, open_offset + 1);
                    let d = self.error(
                        codes::UNTERMINATED_INTERPOLATION,
                        span,
                        "unterminated interpolation",
                    );
                    self.emit(
                        d.with_cause("the interpolation opens here and is never closed")
                            .with_help("add the closing `}`, or write `\\{` for a literal brace"),
                    );
                    return None;
                }
                Some('{') => {
                    depth += 1;
                    self.pos += 1;
                }
                Some('}') => {
                    depth -= 1;
                    if depth == 0 {
                        let span = self.source.span(text_start, self.offset());
                        let text = self.source.slice(span).to_string();
                        self.pos += 1; // `}`
                        return Some(StrPart::Expr { text, span });
                    }
                    self.pos += 1;
                }
                Some(_) => {
                    self.pos += 1;
                }
            }
        }
    }

    /// Resolves one escape sequence, having already consumed the backslash.
    ///
    /// `quote` is the delimiter of the literal being read, so `\"` works
    /// inside a string and `\'` inside a character literal.
    fn escape(&mut self, quote: char) -> Option<char> {
        let escape_start = self.offset() - 1;

        match self.advance() {
            Some('n') => Some('\n'),
            Some('t') => Some('\t'),
            Some('r') => Some('\r'),
            Some('0') => Some('\0'),
            Some('\\') => Some('\\'),
            // A literal brace, which is otherwise the opening of an
            // interpolation.
            Some('{') => Some('{'),
            Some('}') => Some('}'),
            Some(c) if c == quote => Some(c),
            Some(other) => {
                let span = self.source.span(escape_start, self.offset());
                let d = self.error(
                    codes::UNKNOWN_ESCAPE,
                    span,
                    format!("unknown escape sequence: `\\{other}`"),
                );
                self.emit(
                    d.with_cause("this is not an escape sequence of the language")
                        .with_help(format!(
                            "the valid sequences are \\n, \\t, \\r, \\0, \\{{, \\}}, \\{quote} and \\\\"
                        )),
                );
                // The character is kept so tokenizing can continue.
                Some(other)
            }
            None => None,
        }
    }

    /// Character literal.
    ///
    /// The full content between the quotes is kept, however many code points
    /// it holds: a `Char` is one Unicode **grapheme**, and a family emoji is
    /// one grapheme made of several code points. Deciding whether the content
    /// is exactly one grapheme needs Unicode segmentation, which is semantics
    /// and not lexing.
    fn character(&mut self) -> Option<TokenKind> {
        let start_offset = self.offset();
        self.pos += 1; // opening quote

        let mut value = String::new();

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = self.source.span(start_offset, start_offset + 1);
                    let d = self.error(
                        codes::UNTERMINATED_CHARACTER,
                        span,
                        "unterminated character literal",
                    );
                    self.emit(
                        d.with_cause(
                            "the literal opens here and is not closed before the end of the line",
                        )
                        .with_help("add the closing `'` quote"),
                    );
                    return None;
                }
                Some('\'') => {
                    self.pos += 1;
                    return Some(TokenKind::Char(canonical(value)));
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.escape('\'') {
                        Some(c) => value.push(c),
                        None => continue,
                    }
                }
                Some(c) => {
                    value.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// Regex literal `re'pattern'`.
    ///
    /// Escapes are **preserved**, not resolved: `\d` means something to the
    /// regex engine and nothing to the string escapes. Resolving them here
    /// would destroy the pattern before its own parser ever saw it.
    fn regex(&mut self) -> Option<TokenKind> {
        let start_offset = self.offset();
        self.pos += 3; // `re'`

        let mut pattern = String::new();

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = self.source.span(start_offset, start_offset + 3);
                    let d = self.error(
                        codes::UNTERMINATED_REGEX,
                        span,
                        "unterminated regex literal",
                    );
                    self.emit(
                        d.with_cause(
                            "the literal opens here and is not closed before the end of the line",
                        )
                        .with_help("add the closing `'` quote"),
                    );
                    return None;
                }
                // A doubled backslash collapses to one, so the pattern can
                // itself use regex escapes like `\d`.  Any other character
                // after a backslash is preserved literally.
                Some('\\') => {
                    self.pos += 1;
                    match self.peek() {
                        Some('\\') => {
                            pattern.push('\\');
                            self.pos += 1;
                        }
                        Some(c) => {
                            pattern.push('\\');
                            pattern.push(c);
                            self.pos += 1;
                        }
                        None => {}
                    }
                }
                Some('\'') => {
                    self.pos += 1;
                    return Some(TokenKind::Regex(pattern));
                }
                Some(c) => {
                    pattern.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// Operators, delimiters and punctuation.
    fn punctuation(&mut self) -> Option<TokenKind> {
        use TokenKind::*;

        let start = self.offset();
        let c = self.advance()?;

        let kind = match c {
            '+' if self.eat('=') => PlusEq,
            '+' if self.eat('+') => PlusPlus,
            '+' => Plus,
            '-' if self.eat('>') => Arrow,
            '-' if self.eat('=') => MinusEq,
            '-' if self.eat('-') => MinusMinus,
            '-' => Minus,
            // `**=` before `**` before `*=` before `*`.
            '*' if self.eat('*') => {
                if self.eat('=') {
                    StarStarEq
                } else {
                    StarStar
                }
            }
            '*' if self.eat('=') => StarEq,
            '*' => Star,
            '/' if self.eat('=') => SlashEq,
            '/' => Slash,
            '%' if self.eat('=') => PercentEq,
            '%' => Percent,
            '=' if self.eat('=') => Eq,
            '=' if self.eat('>') => FatArrow,
            '=' => Assign,
            '!' if self.eat('=') => NotEq,
            '!' => Not,
            '?' if self.eat('?') => QuestionQuestion,
            '?' if self.eat('.') => QuestionDot,
            '?' => Question,
            // `<<=` before `<<` before `<=` before `<`.
            '<' if self.eat('<') => {
                if self.eat('=') {
                    ShlEq
                } else {
                    Shl
                }
            }
            '<' if self.eat('=') => LtEq,
            '<' => Lt,
            '>' if self.eat('>') => {
                if self.eat('=') {
                    ShrEq
                } else {
                    Shr
                }
            }
            '>' if self.eat('=') => GtEq,
            '>' => Gt,
            // The doubled forms are the short-circuit logical operators; the
            // single ones are bitwise. Longest match keeps them apart.
            '&' if self.eat('&') => AndAnd,
            '&' if self.eat('=') => AmpEq,
            '&' => Amp,
            '|' if self.eat('|') => OrOr,
            '|' if self.eat('>') => PipeGt,
            '|' if self.eat('=') => PipeEq,
            '|' => Pipe,
            '^' if self.eat('=') => CaretEq,
            '^' => Caret,
            '~' => Tilde,
            '(' => LParen,
            ')' => RParen,
            '{' => LBrace,
            '}' => RBrace,
            '[' => LBracket,
            ']' => RBracket,
            ',' => Comma,
            '#' => Hash,
            ';' => Semicolon,
            ':' if self.eat(':') => ColonColon,
            ':' => Colon,
            // Longest match first: `...` before `..=` before `..` before `.`.
            '.' if self.eat('.') => {
                if self.eat('.') {
                    DotDotDot
                } else if self.eat('=') {
                    DotDotEq
                } else {
                    DotDot
                }
            }
            '.' => Dot,
            other => {
                let span = self.source.span(start, self.offset());
                let d = self.error(
                    codes::UNRECOGNIZED_CHARACTER,
                    span,
                    format!("unrecognized character: `{other}`"),
                );
                self.emit(d.with_cause("it does not start any token of the language"));
                return None;
            }
        };

        Some(kind)
    }
}

/// Puts literal text into Unicode canonical form (NFC).
///
/// `"hó"` can be written with one code point or with two, depending on the
/// editor, the keyboard and everything the text passed through on its way into
/// the file. They look identical on screen, and
/// `docs/decisions/ADR-011-identidad-e-igualdad-de-string.md` makes them equal.
///
/// Normalizing here — once, at compile time, on a machine in no hurry — is what
/// keeps that promise cheap: comparing two literals stays a byte comparison,
/// and the runtime never pays for normalization it can avoid.
fn canonical(text: String) -> String {
    // The overwhelmingly common case, and the one where allocating again would
    // be pure waste: ASCII has no equivalent forms to collapse.
    if text.is_ascii() {
        return text;
    }
    unicode_normalization::UnicodeNormalization::nfc(text.as_str()).collect()
}

/// A base an integer literal may be written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Radix {
    Hexadecimal,
    Binary,
}

impl Radix {
    /// The base the character after a leading `0` selects.
    fn from_prefix(c: char) -> Option<Self> {
        match c {
            'x' | 'X' => Some(Radix::Hexadecimal),
            'b' | 'B' => Some(Radix::Binary),
            _ => None,
        }
    }

    const fn value(self) -> u32 {
        match self {
            Radix::Hexadecimal => 16,
            Radix::Binary => 2,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Radix::Hexadecimal => "hexadecimal",
            Radix::Binary => "binary",
        }
    }
}

/// A diagnostic under construction, so cause and help can be chained without
/// repeating the location at every error site.
struct PartialDiagnostic {
    diagnostic: Diagnostic,
}

impl PartialDiagnostic {
    fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.with_cause(cause);
        self
    }

    fn with_help(mut self, help: impl Into<String>) -> Self {
        self.diagnostic = self.diagnostic.with_help(help);
        self
    }
}
