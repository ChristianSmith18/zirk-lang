//! Tokens del lenguaje.
//!
//! Se modelan las palabras clave del **lenguaje completo**, no solo las del
//! subset implementado. Reconocerlas permite que el parser distinga una
//! construcción de una fase posterior de un error de sintaxis, y que el
//! diagnóstico diga "todavía no está implementado" en vez de "token
//! inesperado".

use zirk_diagnostics::Span;

/// Un token con su ubicación en el source.
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

/// Palabra clave del lenguaje.
///
/// El campo `en_subset` distingue las que esta fase implementa de las que solo
/// se reconocen para producir un diagnóstico útil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    // --- Subset de la Fase 1 ---
    Fn,
    Mut,
    Inmut,
    If,
    Else,
    Return,
    True,
    False,

    // --- Lenguaje completo, fases posteriores ---
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
    /// Resuelve una palabra clave a partir de su texto.
    ///
    /// No se llama `from_str` para no confundirse con `std::str::FromStr`,
    /// que devuelve `Result` y tiene otra semántica.
    pub fn from_text(texto: &str) -> Option<Self> {
        use Keyword::*;
        Some(match texto {
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

    /// Texto canónico de la palabra clave, para diagnósticos.
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

    /// Si la palabra clave pertenece al subset que esta fase implementa.
    pub const fn en_subset(self) -> bool {
        use Keyword::*;
        matches!(self, Fn | Mut | Inmut | If | Else | Return | True | False)
    }

    /// Fase del roadmap en la que llega la construcción, para el diagnóstico.
    ///
    /// Devuelve `None` para las palabras clave que ya están implementadas.
    pub const fn fase(self) -> Option<u8> {
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

/// Clase de token producida por el lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),

    /// Literal entero ya normalizado, sin separadores.
    Integer(i128),
    /// Literal de cadena con los escapes ya resueltos.
    Str(String),

    // --- Operadores ---
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

    // --- Operadores del lenguaje completo, fases posteriores ---------------
    // Se reconocen para que el parser pueda decir "todavia no esta
    // implementado" en vez de "token inesperado", igual que con las palabras
    // clave. Sin esto, `count += 1` se leeria como `+` seguido de `=`.
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

    // --- Delimitadores y puntuación ---
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

    /// Fin de archivo. Siempre es el último token.
    Eof,
}

impl TokenKind {
    /// Fase del roadmap en la que llega el operador, para el diagnóstico.
    ///
    /// Devuelve `None` para los operadores que el subset ya implementa.
    pub const fn fase(&self) -> Option<u8> {
        use TokenKind::*;
        Some(match self {
            PlusEq | MinusEq | StarEq | SlashEq | PercentEq | PlusPlus | MinusMinus => 2,
            Question | QuestionQuestion | QuestionDot => 2,
            PipeGt => 3,
            _ => return None,
        })
    }

    /// Descripción para diagnósticos.
    pub fn descripcion(&self) -> String {
        use TokenKind::*;
        match self {
            Identifier(nombre) => format!("identificador `{nombre}`"),
            Keyword(k) => format!("palabra clave `{}`", k.as_str()),
            Integer(v) => format!("literal entero `{v}`"),
            Str(_) => "literal de cadena".to_string(),
            Eof => "fin de archivo".to_string(),
            otro => format!("`{}`", otro.simbolo()),
        }
    }

    /// Símbolo textual de los tokens de puntuación y operadores.
    pub fn simbolo(&self) -> &'static str {
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
    fn las_palabras_clave_del_subset_no_declaran_fase_pendiente() {
        assert!(Keyword::Fn.en_subset());
        assert_eq!(Keyword::Fn.fase(), None);
    }

    #[test]
    fn las_palabras_clave_de_fases_posteriores_declaran_su_fase() {
        assert!(!Keyword::Class.en_subset());
        assert_eq!(Keyword::Class.fase(), Some(3));
        assert_eq!(Keyword::For.fase(), Some(2));
        assert_eq!(Keyword::Task.fase(), Some(5));
    }

    #[test]
    fn el_texto_resuelve_a_la_palabra_clave() {
        assert_eq!(Keyword::from_text("fn"), Some(Keyword::Fn));
        assert_eq!(Keyword::from_text("inmut"), Some(Keyword::Inmut));
    }

    #[test]
    fn un_identificador_cualquiera_no_es_palabra_clave() {
        assert_eq!(Keyword::from_text("total"), None);
        assert_eq!(Keyword::from_text("Fn"), None);
    }

    #[test]
    fn el_texto_canonico_es_estable_en_ambos_sentidos() {
        for texto in ["fn", "class", "parallel", "inmut", "default"] {
            let k = Keyword::from_text(texto).expect("palabra clave conocida");
            assert_eq!(k.as_str(), texto);
        }
    }
}
