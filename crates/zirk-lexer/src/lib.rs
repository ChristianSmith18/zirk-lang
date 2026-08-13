//! # zirk-lexer
//!
//! **Responsabilidad:** convertir texto fuente `.zrk` en una secuencia de tokens
//! con su ubicación, y reportar los errores léxicos que encuentre.
//!
//! **Límite:** el lexer no conoce la gramática. No decide si una secuencia de
//! tokens es válida, solo si cada token lo es. El punto y coma opcional de
//! `ZIRK_LANGUAGE_SPEC.md` sección 1 es un problema del parser, no de acá.
//!
//! `ZIRK_COMPILER_SPEC.md` sección 2 lo describe como incremental: debe poder
//! reanalizar solo la región afectada por una edición. Esa capacidad llega
//! cuando exista compilación incremental, pero la API no debe cerrarse a ella.
//!
//! # Recuperación de errores
//!
//! El lexer **no se detiene en el primer error**. Emite el diagnóstico, avanza
//! y sigue tokenizando, de modo que una compilación reporte todos los problemas
//! léxicos de una vez en lugar de uno por ejecución.

mod token;

pub use token::{Keyword, Token, TokenKind};

use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};

/// Códigos de diagnóstico del lexer.
pub mod codes {
    use zirk_diagnostics::Code;

    /// Carácter que no inicia ningún token válido.
    pub const CARACTER_NO_RECONOCIDO: Code = Code::new("E0201");
    /// Literal de cadena sin comilla de cierre.
    pub const CADENA_SIN_CERRAR: Code = Code::new("E0202");
    /// Secuencia de escape no reconocida dentro de una cadena.
    pub const ESCAPE_DESCONOCIDO: Code = Code::new("E0203");
    /// Comentario de bloque sin cierre.
    pub const COMENTARIO_SIN_CERRAR: Code = Code::new("E0204");
    /// Separador `_` en una posición inválida de un literal numérico.
    pub const SEPARADOR_INVALIDO: Code = Code::new("E0205");
    /// Caracteres alfabéticos pegados a un literal numérico.
    pub const SUFIJO_NUMERICO_INVALIDO: Code = Code::new("E0207");
    /// Literal entero que excede la representación interna.
    pub const ENTERO_DEMASIADO_GRANDE: Code = Code::new("E0206");
}

/// Convierte un archivo fuente en tokens, acumulando los errores encontrados.
pub fn tokenize(source: &SourceFile, sink: &mut DiagnosticSink) -> Vec<Token> {
    Lexer::new(source, sink).run()
}

struct Lexer<'a> {
    source: &'a SourceFile,
    sink: &'a mut DiagnosticSink,
    /// Texto como vector de caracteres con su offset de byte, para poder
    /// avanzar por caracteres Unicode sin perder la posición en bytes.
    ///
    /// Cuesta unas ocho veces el tamaño del source en memoria. Es aceptable
    /// para archivos únicos y se revisará cuando exista compilación
    /// incremental, donde el objetivo de latencia de `COMPILER_SPEC` sección 1
    /// vuelve el costo relevante.
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

    // --- Navegación -------------------------------------------------------

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).map(|&(_, c)| c)
    }

    fn peek_at(&self, adelanto: usize) -> Option<char> {
        self.chars.get(self.pos + adelanto).map(|&(_, c)| c)
    }

    /// Offset de byte de la posición actual, o el final del texto.
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

    /// Consume el carácter si coincide con el esperado.
    fn eat(&mut self, esperado: char) -> bool {
        if self.peek() == Some(esperado) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    // --- Diagnósticos -----------------------------------------------------

    fn error(&mut self, code: Code, span: Span, mensaje: impl Into<String>) -> DiagnosticoParcial {
        DiagnosticoParcial {
            diagnostico: Diagnostic::error(code, mensaje)
                .at(self.source.location(span))
                .with_snippet(self.source.snippet(span)),
        }
    }

    fn emitir(&mut self, parcial: DiagnosticoParcial) {
        self.sink.emit(parcial.diagnostico);
    }

    // --- Bucle principal --------------------------------------------------

    fn run(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        loop {
            self.skip_trivia();
            let inicio = self.offset();

            let Some(c) = self.peek() else {
                tokens.push(Token::new(TokenKind::Eof, Span::empty(inicio)));
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
                tokens.push(Token::new(kind, Span::new(inicio, self.offset())));
            }
        }

        tokens
    }

    /// Descarta espacios y comentarios.
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

    /// Comentario de bloque.
    ///
    /// **No anidan**: `/* /* */` cierra en el primer `*/`, como en C. El spec
    /// no lo define, así que se elige la convención más difundida y se fija con
    /// un test para que no cambie por accidente.
    fn block_comment(&mut self) {
        let inicio = self.offset();
        self.pos += 2; // `/*`

        loop {
            match self.peek() {
                None => {
                    let span = Span::new(inicio, inicio + 2);
                    let d = self.error(
                        codes::COMENTARIO_SIN_CERRAR,
                        span,
                        "unterminated block comment",
                    );
                    self.emitir(
                        d.con_causa("the comment opens here and the file ends before it is closed")
                            .con_ayuda("add `*/` where the comment should end"),
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

    // --- Reconocedores ----------------------------------------------------

    /// Identificador o palabra clave.
    fn word(&mut self) -> TokenKind {
        let inicio = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }

        let texto: String = self.chars[inicio..self.pos]
            .iter()
            .map(|&(_, c)| c)
            .collect();

        match Keyword::from_text(&texto) {
            Some(k) => TokenKind::Keyword(k),
            None => TokenKind::Identifier(texto),
        }
    }

    /// Literal entero decimal, admitiendo `_` como separador.
    fn number(&mut self) -> Option<TokenKind> {
        let inicio_offset = self.offset();
        let mut digitos = String::new();
        let mut ultimo_fue_separador = false;
        let mut separador_invalido: Option<Span> = None;
        let mut sufijo_invalido: Option<Span> = None;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                digitos.push(c);
                ultimo_fue_separador = false;
                self.pos += 1;
            } else if c == '_' {
                // Dos separadores seguidos, o uno inicial, no son válidos.
                if ultimo_fue_separador || digitos.is_empty() {
                    separador_invalido.get_or_insert(Span::new(self.offset(), self.offset() + 1));
                }
                ultimo_fue_separador = true;
                self.pos += 1;
            } else if c.is_alphanumeric() {
                // `123abc`: se consume el sufijo entero para no volver a fallar
                // en cada carácter, y se reporta como problema propio — la causa
                // del separador no aplica acá.
                let inicio_sufijo = self.offset();
                while self.peek().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    self.pos += 1;
                }
                sufijo_invalido.get_or_insert(Span::new(inicio_sufijo, self.offset()));
            } else {
                break;
            }
        }

        // Un separador final tampoco es válido.
        if ultimo_fue_separador {
            separador_invalido.get_or_insert(Span::new(self.offset() - 1, self.offset()));
        }

        let span = Span::new(inicio_offset, self.offset());

        if let Some(span_sufijo) = sufijo_invalido {
            let texto: String = self.source.slice(span_sufijo).to_string();
            let d = self.error(
                codes::SUFIJO_NUMERICO_INVALIDO,
                span_sufijo,
                format!("invalid suffix on numeric literal: `{texto}`"),
            );
            self.emitir(
                d.con_causa("an integer literal only accepts digits and the `_` separator")
                    .con_ayuda(
                        "separate the number from the identifier with a space or an operator",
                    ),
            );
            return None;
        }

        if let Some(span_invalido) = separador_invalido {
            let d = self.error(
                codes::SEPARADOR_INVALIDO,
                span_invalido,
                "malformed numeric literal",
            );
            self.emitir(
                d.con_causa("`_` may only appear between digits")
                    .con_ayuda("write the number as `1_000_000`"),
            );
            return None;
        }

        match digitos.parse::<i128>() {
            Ok(valor) => Some(TokenKind::Integer(valor)),
            Err(_) => {
                let d = self.error(
                    codes::ENTERO_DEMASIADO_GRANDE,
                    span,
                    "integer literal is too large",
                );
                self.emitir(
                    d.con_causa("the value exceeds the largest integer the compiler can represent"),
                );
                None
            }
        }
    }

    /// Literal de cadena con escapes.
    fn string(&mut self) -> Option<TokenKind> {
        let inicio_offset = self.offset();
        self.pos += 1; // comilla de apertura

        let mut valor = String::new();

        loop {
            match self.peek() {
                None | Some('\n') => {
                    let span = Span::new(inicio_offset, inicio_offset + 1);
                    let d = self.error(
                        codes::CADENA_SIN_CERRAR,
                        span,
                        "unterminated string literal",
                    );
                    self.emitir(
                        d.con_causa(
                            "the string opens here and is not closed before the end of the line",
                        )
                        .con_ayuda("add the closing `\"` quote"),
                    );
                    return None;
                }
                Some('"') => {
                    self.pos += 1;
                    return Some(TokenKind::Str(valor));
                }
                Some('\\') => {
                    let inicio_escape = self.offset();
                    self.pos += 1;
                    match self.advance() {
                        Some('n') => valor.push('\n'),
                        Some('t') => valor.push('\t'),
                        Some('r') => valor.push('\r'),
                        Some('0') => valor.push('\0'),
                        Some('"') => valor.push('"'),
                        Some('\\') => valor.push('\\'),
                        Some(otro) => {
                            let span = Span::new(inicio_escape, self.offset());
                            let d = self.error(
                                codes::ESCAPE_DESCONOCIDO,
                                span,
                                format!("unknown escape sequence: `\\{otro}`"),
                            );
                            self.emitir(
                                d.con_causa("this is not an escape sequence of the language")
                                    .con_ayuda(
                                        "the valid sequences are \\n, \\t, \\r, \\0, \\\" and \\\\",
                                    ),
                            );
                            // Se conserva el carácter para seguir tokenizando.
                            valor.push(otro);
                        }
                        None => continue,
                    }
                }
                Some(c) => {
                    valor.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    /// Operadores, delimitadores y puntuación.
    fn punctuation(&mut self) -> Option<TokenKind> {
        use TokenKind::*;

        let inicio = self.offset();
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
            '.' => Dot,
            otro => {
                let span = Span::new(inicio, self.offset());
                let d = self.error(
                    codes::CARACTER_NO_RECONOCIDO,
                    span,
                    format!("unrecognized character: `{otro}`"),
                );
                self.emitir(d.con_causa("it does not start any token of the language"));
                return None;
            }
        };

        Some(kind)
    }
}

/// Diagnóstico en construcción, para encadenar causa y ayuda sin repetir la
/// ubicación en cada sitio de error.
struct DiagnosticoParcial {
    diagnostico: Diagnostic,
}

impl DiagnosticoParcial {
    fn con_causa(mut self, causa: impl Into<String>) -> Self {
        self.diagnostico = self.diagnostico.with_cause(causa);
        self
    }

    fn con_ayuda(mut self, ayuda: impl Into<String>) -> Self {
        self.diagnostico = self.diagnostico.with_help(ayuda);
        self
    }
}
