//! Tokens of the language.
//!
//! The keywords of the **whole language** are modelled, not only those of the
//! implemented subset. Recognizing them lets the parser tell a construct from a
//! later phase apart from a syntax error, so the diagnostic can say "not
//! implemented yet" instead of "unexpected token".

use zirk_diagnostics::{Phase, Span};

/// A token together with its location in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// A keyword of the language.
///
/// [`Keyword::in_subset`] tells the ones this phase implements apart from those
/// recognized only to produce a useful diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    // --- Phase 1 subset ---
    Fn,
    Mut,
    Inmut,
    If,
    Else,
    Return,
    True,
    False,

    // --- Phase 2: control flow, functions, match, nullability, modules ---
    For,
    In,
    While,
    Loop,
    Break,
    Continue,
    Match,
    Enum,
    Null,
    Share,
    Import,
    Use,

    // --- Whole language, later phases ---
    /// `do { } while cond;`
    Do,
    /// `yield` inside a `fn gen`.
    Yield,
    Interface,
    Trait,
    With,
    /// `transfer(expr)` — ownership transfer of a `TransferableResource`.
    Transfer,
    Try,
    Catch,
    Finally,
    /// `throw expr;` / `throw;` (rethrow, roadmap Phase 4b).
    Throw,
    /// `throws Type (| Type)*` on a function signature (roadmap Phase 4b).
    Throws,
    Class,
    Construct,
    This,
    /// `super(...)` for base construction, `super.method()` for inherited
    /// behavior.
    Super,
    Override,
    Record,
    Type,
    Public,
    Private,
    Protected,
    Abstract,
    Implements,
    Extends,
    From,
    As,
    Is,
    Unsafe,
    Task,
    Await,
    Parallel,
    Thread,
    Sync,
    Dec,
    Gen,
    Default,
    /// `static` members on classes: fields and methods accessed as
    /// `ClassName.member`, with no instance receiver.
    Static,
    /// `out T` on a generic parameter, restricting it to covariant output
    /// positions. `in` is shared with `for ... in`, so only `out` needs a
    /// keyword of its own.
    Out,
    /// `commit { }`, the irreversible-effect boundary nested inside `unsafe`
    /// (roadmap Phase 4e).
    Commit,
    /// `extern "C" fn name(...): T;`, a bodyless native declaration (roadmap
    /// Phase 4e, `ADR-015-declaracion-extern.md`).
    Extern,
    /// `Pin<T>` / `pin` (roadmap Phase 4e, `phase-4e-memory`): the type form
    /// is implemented as a built-in generic, while the statement form is
    /// reserved.
    Pin,
    /// `final` seals a class against `extends` or a method against
    /// `#override`.
    Final,
    /// `inner class` marks a nested class that captures a reference to its
    /// enclosing instance.
    Inner,
}

impl Keyword {
    /// Resolves a keyword from its text.
    ///
    /// It is not called `from_str` so it is not confused with
    /// `std::str::FromStr`, which returns `Result` and has other semantics.
    pub fn from_text(text: &str) -> Option<Self> {
        use Keyword::*;
        Some(match text {
            "fn" => Fn,
            "mut" => Mut,
            "inmut" => Inmut,
            "if" => If,
            "else" => Else,
            "return" => Return,
            "true" => True,
            "false" => False,
            "for" => For,
            "in" => In,
            "while" => While,
            "loop" => Loop,
            "break" => Break,
            "continue" => Continue,
            "match" => Match,
            "do" => Do,
            "yield" => Yield,
            "interface" => Interface,
            "trait" => Trait,
            "with" => With,
            "transfer" => Transfer,
            "try" => Try,
            "catch" => Catch,
            "finally" => Finally,
            "throw" => Throw,
            "throws" => Throws,
            "class" => Class,
            "construct" => Construct,
            "this" => This,
            "super" => Super,
            "override" => Override,
            "enum" => Enum,
            "record" => Record,
            "type" => Type,
            "share" => Share,
            "import" => Import,
            "use" => Use,
            "public" => Public,
            "private" => Private,
            "protected" => Protected,
            "abstract" => Abstract,
            "implements" => Implements,
            "extends" => Extends,
            "from" => From,
            "as" => As,
            "is" => Is,
            "unsafe" => Unsafe,
            "task" => Task,
            "await" => Await,
            "parallel" => Parallel,
            "thread" => Thread,
            "sync" => Sync,
            "dec" => Dec,
            "gen" => Gen,
            "null" => Null,
            "default" => Default,
            "static" => Static,
            "out" => Out,
            "commit" => Commit,
            "extern" => Extern,
            "Pin" => Pin,
            "final" => Final,
            "inner" => Inner,
            _ => return None,
        })
    }

    /// Canonical text of the keyword, for diagnostics.
    pub const fn as_str(self) -> &'static str {
        use Keyword::*;
        match self {
            Fn => "fn",
            Mut => "mut",
            Inmut => "inmut",
            If => "if",
            Else => "else",
            Return => "return",
            True => "true",
            False => "false",
            For => "for",
            In => "in",
            While => "while",
            Loop => "loop",
            Break => "break",
            Continue => "continue",
            Match => "match",
            Do => "do",
            Yield => "yield",
            Interface => "interface",
            Trait => "trait",
            With => "with",
            Transfer => "transfer",
            Try => "try",
            Catch => "catch",
            Finally => "finally",
            Throw => "throw",
            Throws => "throws",
            Class => "class",
            Construct => "construct",
            This => "this",
            Super => "super",
            Override => "override",
            Enum => "enum",
            Record => "record",
            Type => "type",
            Share => "share",
            Import => "import",
            Use => "use",
            Public => "public",
            Private => "private",
            Protected => "protected",
            Abstract => "abstract",
            Implements => "implements",
            Extends => "extends",
            From => "from",
            As => "as",
            Is => "is",
            Unsafe => "unsafe",
            Task => "task",
            Await => "await",
            Parallel => "parallel",
            Thread => "thread",
            Sync => "sync",
            Dec => "dec",
            Gen => "gen",
            Null => "null",
            Default => "default",
            Static => "static",
            Out => "out",
            Commit => "commit",
            Extern => "extern",
            Pin => "Pin",
            Final => "final",
            Inner => "inner",
        }
    }

    /// Whether the keyword belongs to the subset this phase implements.
    pub const fn in_subset(self) -> bool {
        self.phase().is_none()
    }

    /// Roadmap phase in which the construct arrives, used by the diagnostic.
    ///
    /// Returns `None` for keywords that are already implemented.
    pub const fn phase(self) -> Option<Phase> {
        use Keyword::*;
        Some(match self {
            // `match ... with` is implemented (roadmap Phase 4c: `Resource<E>`
            // and scoped acquisition), so `With` is no longer gated here —
            // `parse_match` consumes it directly. `unsafe`/`Pointer<T>`/
            // `commit`/`extern` are implemented as of Phase 4e
            // (`fase-4e-unsafe-pointer-extern`), so `Unsafe` is no longer
            // gated here either — `Commit`/`Extern` were never gated (they
            // ship directly in this change, per design D-nothing: there was
            // no earlier phase claiming them). `default` labels the
            // catch-all arm of a `try`, so it arrives with error handling
            // and not with the decorators it used to be filed under — but
            // `catch Throwable(e)` already means "catch everything" for this
            // phase's scope, so `default` stays gated until `match with`
            // needs its own catch-all arm.
            Default => Phase::FOUR,
            Task | Await | Parallel | Thread | Sync => Phase::FIVE,
            // Generators are the functional style of `LANGUAGE_SPEC` section 8,
            // which the roadmap places after the collections they iterate.
            Gen | Yield => Phase::SEVEN_B,
            Dec => Phase::TEN,
            _ => return None,
        })
    }
}

/// A piece of an interpolated string literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrPart {
    /// Literal text, with its escapes already resolved.
    Literal(String),
    /// An embedded expression, kept as the source text between its braces.
    ///
    /// It is not tokenized here: doing so would make the lexer recursive for
    /// no gain, since the parser has to run over these tokens anyway.
    Expr { text: String, span: Span },
}

/// A numeric literal as written, minus its separators.
///
/// The text is kept instead of a computed value on purpose. The width is
/// chosen by the checker, and `Float128` represents values a host `f64` would
/// round on the way in: converting here would decide, in the wrong layer, a
/// precision the target type may exceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberLit {
    /// Digits, decimal point and exponent as written, without `_`.
    pub text: String,
    /// Explicit width suffix, such as the `f32` of `1.5f32`.
    pub width: Option<String>,
}

impl NumberLit {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            width: None,
        }
    }

    pub fn with_width(mut self, width: impl Into<String>) -> Self {
        self.width = Some(width.into());
        self
    }
}

/// Unit of a duration literal.
///
/// Months and years are absent by design: they are calendar quantities whose
/// length depends on where they are applied, so they belong to `Period` and not
/// to an exact `Duration`. That is also why `m` is minutes here and never
/// months.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
    Weeks,
}

impl DurationUnit {
    pub fn from_text(text: &str) -> Option<Self> {
        use DurationUnit::*;
        Some(match text {
            "ns" => Nanoseconds,
            "us" => Microseconds,
            "ms" => Milliseconds,
            "s" => Seconds,
            "m" => Minutes,
            "h" => Hours,
            "d" => Days,
            "w" => Weeks,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        use DurationUnit::*;
        match self {
            Nanoseconds => "ns",
            Microseconds => "us",
            Milliseconds => "ms",
            Seconds => "s",
            Minutes => "m",
            Hours => "h",
            Days => "d",
            Weeks => "w",
        }
    }
}

/// Suffixes that make a fractional literal an IEEE 754 binary `BinaryFloat`.
///
/// A suffix-less fractional literal is the exact base-ten `Float` instead.
/// `b` alone means `BinaryFloat64`.
pub const FLOAT_WIDTHS: &[&str] = &["b", "b16", "b32", "b64", "b128"];

/// Class of token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),

    /// Integer literal, already normalized without separators.
    Integer(i128),
    /// Fractional or scientific literal.
    Float(NumberLit),
    /// A number with a duration unit attached, such as `250ms`.
    Duration(NumberLit, DurationUnit),
    /// String literal with escapes already resolved.
    Str(String),
    /// String literal containing `{ expression }` interpolation.
    ///
    /// Kept as parts rather than as text so the embedded expressions retain
    /// their own spans. Re-scanning the string later would work, but the
    /// offsets into the file — and with them every diagnostic pointing inside
    /// an interpolation — would already be lost.
    InterpolatedStr(Vec<StrPart>),
    /// Character literal, holding the full Unicode content between the quotes.
    ///
    /// Whether it is exactly one grapheme is not decided here: that is the
    /// semantics of `Char`, and the lexer does not know Unicode segmentation.
    Char(String),
    /// Regex literal `re'pattern'`, holding the pattern with escapes intact.
    Regex(String),

    // --- Operators ---
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AndAnd,
    OrOr,
    Not,

    // --- Whole-language operators, later phases ---------------------------
    // Recognized so the parser can say "not implemented yet" instead of
    // "unexpected token", exactly as with keywords. Without these, `count += 1`
    // would read as `+` followed by `=`.
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    PlusPlus,
    MinusMinus,
    Question,
    QuestionQuestion,
    QuestionDot,
    PipeGt,
    /// `**`, exponentiation. Without it, `2 ** 3` would read as two
    /// multiplications and fail with a message about the wrong thing.
    StarStar,
    StarStarEq,

    // --- Bitwise and shift operators, levels 7 to 10 of the operator table --
    /// `&`. Distinct from `&&`, which is the short-circuit conjunction.
    Amp,
    AmpEq,
    /// `|`. Distinct from `||` and from the pipe `|>`.
    Pipe,
    PipeEq,
    Caret,
    CaretEq,
    /// `~`, the integer complement. It has no compound form.
    Tilde,
    Shl,
    ShlEq,
    /// `>>`.
    ///
    /// Emitted as one token, which is what makes `a >> b` unambiguous. The
    /// cost lands on generics: `Box<Box<Int32>>` ends in this token, so the
    /// type parser of Phase 3 has to split it back into two `>`. That is the
    /// same trade every language with both features makes.
    Shr,
    ShrEq,

    // --- Delimiters and punctuation ---
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    ColonColon,
    Dot,
    /// `..`, exclusive range.
    DotDot,
    /// `..=`, inclusive range.
    DotDotEq,
    /// `...`, variadic parameter marker.
    DotDotDot,
    Arrow,
    FatArrow,
    /// `#`, introducing a member marker such as `#override`.
    Hash,

    /// End of file. Always the last token.
    Eof,
}

impl TokenKind {
    /// Roadmap phase in which the operator arrives, used by the diagnostic.
    ///
    /// Returns `None` for operators the subset already implements.
    pub const fn phase(&self) -> Option<Phase> {
        use TokenKind::*;
        Some(match self {
            // The pipe belongs to the functional style, not to the objects of
            // Phase 3 it used to be filed under.
            PipeGt => Phase::SEVEN_B,
            // `**` / `**=` are part of the implemented subset (Phase 3b,
            // `exponentiation-operator`): the parser desugars `a ** b` to
            // `a.pow(b)` over the numeric families, and a negative literal
            // exponent widens an integer base to exact `Float`.
            _ => return None,
        })
    }

    /// Description used in diagnostics.
    pub fn description(&self) -> String {
        use TokenKind::*;
        match self {
            Identifier(name) => format!("identifier `{name}`"),
            Keyword(k) => format!("keyword `{}`", k.as_str()),
            Integer(v) => format!("integer literal `{v}`"),
            Float(lit) => format!("float literal `{}`", lit.text),
            Duration(lit, unit) => {
                format!("duration literal `{}{}`", lit.text, unit.as_str())
            }
            Str(_) => "a string literal".to_string(),
            Char(c) => format!("character literal `'{c}'`"),
            Regex(_) => "a regex literal".to_string(),
            Eof => "end of file".to_string(),
            other => format!("`{}`", other.symbol()),
        }
    }

    /// Textual symbol of punctuation and operator tokens.
    pub fn symbol(&self) -> &'static str {
        use TokenKind::*;
        match self {
            Plus => "+",
            Minus => "-",
            Star => "*",
            Slash => "/",
            Percent => "%",
            Assign => "=",
            Eq => "==",
            NotEq => "!=",
            Lt => "<",
            LtEq => "<=",
            Gt => ">",
            GtEq => ">=",
            AndAnd => "&&",
            OrOr => "||",
            Not => "!",
            PlusEq => "+=",
            MinusEq => "-=",
            StarEq => "*=",
            SlashEq => "/=",
            PercentEq => "%=",
            PlusPlus => "++",
            MinusMinus => "--",
            Question => "?",
            QuestionQuestion => "??",
            QuestionDot => "?.",
            PipeGt => "|>",
            StarStar => "**",
            StarStarEq => "**=",
            Amp => "&",
            AmpEq => "&=",
            Pipe => "|",
            PipeEq => "|=",
            Caret => "^",
            CaretEq => "^=",
            Tilde => "~",
            Shl => "<<",
            ShlEq => "<<=",
            Shr => ">>",
            ShrEq => ">>=",
            LParen => "(",
            RParen => ")",
            LBrace => "{",
            RBrace => "}",
            LBracket => "[",
            RBracket => "]",
            Comma => ",",
            Semicolon => ";",
            Colon => ":",
            ColonColon => "::",
            Dot => ".",
            DotDot => "..",
            DotDotEq => "..=",
            DotDotDot => "...",
            Arrow => "->",
            FatArrow => "=>",
            Hash => "#",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_keywords_declare_no_pending_phase() {
        assert!(Keyword::Fn.in_subset());
        assert_eq!(Keyword::Fn.phase(), None);
    }

    #[test]
    fn implemented_keywords_declare_no_phase() {
        // Phase 3's full vocabulary: objects, records, type
        // aliases, contracts and casts.
        for k in [
            Keyword::Class,
            Keyword::Construct,
            Keyword::This,
            Keyword::Record,
            Keyword::Type,
            Keyword::Public,
            Keyword::Private,
            Keyword::Protected,
            Keyword::Abstract,
            Keyword::Implements,
            Keyword::Extends,
            Keyword::From,
            Keyword::As,
            Keyword::Is,
            Keyword::Interface,
            Keyword::Trait,
        ] {
            assert!(k.in_subset(), "`{}` is implemented", k.as_str());
        }
    }

    #[test]
    fn later_phase_keywords_declare_their_phase() {
        assert!(!Keyword::Task.in_subset());
        assert_eq!(Keyword::Task.phase(), Some(Phase::FIVE));
    }

    #[test]
    fn unsafe_commit_and_extern_are_in_the_subset() {
        for k in [Keyword::Unsafe, Keyword::Commit, Keyword::Extern] {
            assert!(k.in_subset(), "`{}` should be implemented", k.as_str());
            assert_eq!(k.phase(), None, "`{}`", k.as_str());
        }
    }

    #[test]
    fn commit_and_extern_resolve_from_text() {
        assert_eq!(Keyword::from_text("commit"), Some(Keyword::Commit));
        assert_eq!(Keyword::from_text("extern"), Some(Keyword::Extern));
        assert_eq!(Keyword::Commit.as_str(), "commit");
        assert_eq!(Keyword::Extern.as_str(), "extern");
    }

    #[test]
    fn phase_two_keywords_are_in_the_subset() {
        for k in [
            Keyword::For,
            Keyword::In,
            Keyword::While,
            Keyword::Loop,
            Keyword::Break,
            Keyword::Continue,
            Keyword::Match,
            Keyword::Enum,
            Keyword::Null,
            Keyword::Share,
            Keyword::Import,
            Keyword::Use,
        ] {
            assert!(k.in_subset(), "`{}` should be implemented", k.as_str());
            assert_eq!(k.phase(), None, "`{}`", k.as_str());
        }
    }

    #[test]
    fn range_and_variadic_tokens_carry_their_symbol() {
        assert_eq!(TokenKind::DotDot.symbol(), "..");
        assert_eq!(TokenKind::DotDotEq.symbol(), "..=");
        assert_eq!(TokenKind::DotDotDot.symbol(), "...");
    }

    #[test]
    fn text_resolves_to_the_keyword() {
        assert_eq!(Keyword::from_text("fn"), Some(Keyword::Fn));
        assert_eq!(Keyword::from_text("inmut"), Some(Keyword::Inmut));
    }

    #[test]
    fn an_ordinary_identifier_is_not_a_keyword() {
        assert_eq!(Keyword::from_text("total"), None);
        assert_eq!(Keyword::from_text("Fn"), None);
    }

    #[test]
    fn the_canonical_text_round_trips() {
        for text in ["fn", "class", "parallel", "inmut", "default"] {
            let k = Keyword::from_text(text).expect("known keyword");
            assert_eq!(k.as_str(), text);
        }
    }
}
