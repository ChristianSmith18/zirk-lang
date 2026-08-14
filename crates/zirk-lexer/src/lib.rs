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

pub use token::{Keyword, Token, TokenKind};

use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};

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
}

/// Turns a source file into tokens, accumulating any errors found.
pub fn tokenize(source: &SourceFile, sink: &mut DiagnosticSink) -> Vec<Token> {
    Lexer::new(source, sink).run()
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
                tokens.push(Token::new(TokenKind::Eof, Span::empty(start)));
                break;
            };

            let kind = if c.is_alphabetic() || c == '_' {
                Some(self.word())
            } else if c.is_ascii_digit() {
                self.number()
            } else if c == '"' {
                self.string()
            } else {
                self.punctuation()
            };

            if let Some(kind) = kind {
                tokens.push(Token::new(kind, Span::new(start, self.offset())));
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
                    let span = Span::new(start, start + 2);
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

    /// Decimal integer literal, allowing `_` as a separator.
    fn number(&mut self) -> Option<TokenKind> {
        let start_offset = self.offset();
        let mut digits = String::new();
        let mut last_was_separator = false;
        let mut invalid_separator: Option<Span> = None;
        let mut invalid_suffix: Option<Span> = None;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                digits.push(c);
                last_was_separator = false;
                self.pos += 1;
            } else if c == '_' {
                // Two separators in a row, or a leading one, are invalid.
                if last_was_separator || digits.is_empty() {
                    invalid_separator.get_or_insert(Span::new(self.offset(), self.offset() + 1));
                }
                last_was_separator = true;
                self.pos += 1;
            } else if c.is_alphanumeric() {
                // `123abc`: the whole suffix is consumed so we do not fail
                // again on every character, and it is reported as its own
                // problem — the separator cause does not apply here.
                let suffix_start = self.offset();
                while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    self.pos += 1;
                }
                invalid_suffix.get_or_insert(Span::new(suffix_start, self.offset()));
            } else {
                break;
            }
        }

        // A trailing separator is not valid either.
        if last_was_separator {
            invalid_separator.get_or_insert(Span::new(self.offset() - 1, self.offset()));
        }

        let span = Span::new(start_offset, self.offset());

        if let Some(suffix_span) = invalid_suffix {
            let text: String = self.source.slice(suffix_span).to_string();
            let d = self.error(
                codes::INVALID_NUMERIC_SUFFIX,
                suffix_span,
                format!("invalid suffix on numeric literal: `{text}`"),
            );
            self.emit(
                d.with_cause("an integer literal only accepts digits and the `_` separator")
                    .with_help(
                        "separate the number from the identifier with a space or an operator",
                    ),
            );
            return None;
        }

        if let Some(invalid_span) = invalid_separator {
            let d = self.error(
                codes::INVALID_SEPARATOR,
                invalid_span,
                "malformed numeric literal",
            );
            self.emit(
                d.with_cause("`_` may only appear between digits")
                    .with_help("write the number as `1_000_000`"),
            );
            return None;
        }

        match digits.parse::<i128>() {
            Ok(value) => Some(TokenKind::Integer(value)),
            Err(_) => {
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

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = Span::new(start_offset, start_offset + 1);
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
                    return Some(TokenKind::Str(value));
                }
                Some('\\') => {
                    let escape_start = self.offset();
                    self.pos += 1;
                    match self.advance() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('r') => value.push('\r'),
                        Some('0') => value.push('\0'),
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some(other) => {
                            let span = Span::new(escape_start, self.offset());
                            let d = self.error(
                                codes::UNKNOWN_ESCAPE,
                                span,
                                format!("unknown escape sequence: `\\{other}`"),
                            );
                            self.emit(
                                d.with_cause("this is not an escape sequence of the language")
                                    .with_help(
                                        "the valid sequences are \\n, \\t, \\r, \\0, \\\" and \\\\",
                                    ),
                            );
                            // The character is kept so tokenizing can continue.
                            value.push(other);
                        }
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
            '<' if self.eat('=') => LtEq,
            '<' => Lt,
            '>' if self.eat('=') => GtEq,
            '>' => Gt,
            '&' if self.eat('&') => AndAnd,
            '|' if self.eat('|') => OrOr,
            '|' if self.eat('>') => PipeGt,
            '(' => LParen,
            ')' => RParen,
            '{' => LBrace,
            '}' => RBrace,
            '[' => LBracket,
            ']' => RBracket,
            ',' => Comma,
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
                let span = Span::new(start, self.offset());
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
