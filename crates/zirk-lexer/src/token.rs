//! Tokens of the language.
//!
//! The keywords of the **whole language** are modelled, not only those of the
//! implemented subset. Recognizing them lets the parser tell a construct from a
//! later phase apart from a syntax error, so the diagnostic can say "not
//! implemented yet" instead of "unexpected token".

use zirk_diagnostics::Span;

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

    // --- Whole language, later phases ---
    For,
    While,
    Loop,
    Break,
    Continue,
    Match,
    With,
    Try,
    Catch,
    Finally,
    Class,
    Construct,
    This,
    Enum,
    Record,
    Type,
    Share,
    Import,
    Use,
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
    Null,
    Default,
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
            "while" => While,
            "loop" => Loop,
            "break" => Break,
            "continue" => Continue,
            "match" => Match,
            "with" => With,
            "try" => Try,
            "catch" => Catch,
            "finally" => Finally,
            "class" => Class,
            "construct" => Construct,
            "this" => This,
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
            While => "while",
            Loop => "loop",
            Break => "break",
            Continue => "continue",
            Match => "match",
            With => "with",
            Try => "try",
            Catch => "catch",
            Finally => "finally",
            Class => "class",
            Construct => "construct",
            This => "this",
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
        }
    }

    /// Whether the keyword belongs to the subset this phase implements.
    pub const fn in_subset(self) -> bool {
        use Keyword::*;
        matches!(self, Fn | Mut | Inmut | If | Else | Return | True | False)
    }

    /// Roadmap phase in which the construct arrives, used by the diagnostic.
    ///
    /// Returns `None` for keywords that are already implemented.
    pub const fn phase(self) -> Option<u8> {
        use Keyword::*;
        Some(match self {
            For | While | Loop | Break | Continue | Match | Share | Import | Use => 2,
            Class | Construct | This | Enum | Record | Type | Public | Private | Protected
            | Abstract | Implements | Extends | From | As | Is => 3,
            Try | Catch | Finally | With | Unsafe | Null => 4,
            Task | Await | Parallel | Thread | Sync => 5,
            Dec | Gen | Default => 10,
            _ => return None,
        })
    }
}

/// Class of token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),

    /// Integer literal, already normalized without separators.
    Integer(i128),
    /// String literal with escapes already resolved.
    Str(String),

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
    Arrow,
    FatArrow,

    /// End of file. Always the last token.
    Eof,
}

impl TokenKind {
    /// Roadmap phase in which the operator arrives, used by the diagnostic.
    ///
    /// Returns `None` for operators the subset already implements.
    pub const fn phase(&self) -> Option<u8> {
        use TokenKind::*;
        Some(match self {
            PlusEq | MinusEq | StarEq | SlashEq | PercentEq | PlusPlus | MinusMinus => 2,
            Question | QuestionQuestion | QuestionDot => 2,
            PipeGt => 3,
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
            Str(_) => "a string literal".to_string(),
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
            Arrow => "->",
            FatArrow => "=>",
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
    fn later_phase_keywords_declare_their_phase() {
        assert!(!Keyword::Class.in_subset());
        assert_eq!(Keyword::Class.phase(), Some(3));
        assert_eq!(Keyword::For.phase(), Some(2));
        assert_eq!(Keyword::Task.phase(), Some(5));
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
