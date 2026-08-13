//! Tests del léxico.
//!
//! `ZIRK_SPEC_FINAL.md` sección 8 exige que cada regla del lenguaje tenga al
//! menos un caso válido y uno inválido. Los tests están agrupados por regla
//! para que esa correspondencia sea verificable a simple vista.

use zirk_diagnostics::{DiagnosticSink, SourceFile};
use zirk_lexer::{Keyword, TokenKind, codes, tokenize};

/// Tokeniza esperando que no haya errores.
fn tokens(fuente: &str) -> Vec<TokenKind> {
    let source = SourceFile::new("test.zrk", fuente);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);

    assert!(
        !sink.has_errors(),
        "no se esperaban errores léxicos:\n{}",
        sink.render(zirk_diagnostics::RenderStyle::Human)
    );

    tokens.into_iter().map(|t| t.kind).collect()
}

/// Tokeniza esperando error, y devuelve los diagnósticos renderizados.
fn errores(fuente: &str) -> String {
    let source = SourceFile::new("test.zrk", fuente);
    let mut sink = DiagnosticSink::new();
    tokenize(&source, &mut sink);

    assert!(
        sink.has_errors(),
        "se esperaba un error léxico y no hubo ninguno"
    );
    sink.render(zirk_diagnostics::RenderStyle::Human)
}

// --- Tokenización general ---------------------------------------------------

#[test]
fn valido_programa_minimo() {
    use TokenKind::*;
    assert_eq!(
        tokens("fn main(): Void { }"),
        vec![
            Keyword(zirk_lexer::Keyword::Fn),
            Identifier("main".into()),
            LParen,
            RParen,
            Colon,
            Identifier("Void".into()),
            LBrace,
            RBrace,
            Eof,
        ]
    );
}

#[test]
fn valido_toda_secuencia_termina_en_eof() {
    assert_eq!(tokens("").last(), Some(&TokenKind::Eof));
    assert_eq!(tokens("fn").last(), Some(&TokenKind::Eof));
}

#[test]
fn invalido_caracter_no_reconocido() {
    let salida = errores("fn main() @ { }");
    assert!(salida.contains(codes::CARACTER_NO_RECONOCIDO.as_str()));
    assert!(salida.contains('@'));
    assert!(salida.contains("= cause:"));
}

// --- Ubicación --------------------------------------------------------------

#[test]
fn valido_cada_token_conserva_su_ubicacion() {
    let source = SourceFile::new("test.zrk", "fn main");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);

    assert_eq!(source.slice(tokens[0].span), "fn");
    assert_eq!(source.slice(tokens[1].span), "main");
}

#[test]
fn valido_la_columna_del_diagnostico_cuenta_caracteres() {
    // 'ñ' ocupa dos bytes: si la columna contara bytes, diría 8 en vez de 7.
    let salida = errores("mut añs = @;");
    assert!(salida.contains(":1:11"), "ubicación inesperada:\n{salida}");
}

// --- Literales enteros ------------------------------------------------------

#[test]
fn valido_entero_simple() {
    assert_eq!(tokens("42")[0], TokenKind::Integer(42));
}

#[test]
fn valido_entero_con_separadores() {
    assert_eq!(tokens("1_000_000")[0], TokenKind::Integer(1_000_000));
}

#[test]
fn invalido_separador_al_inicio_del_numero() {
    // `_1000` es un identificador, no un número: la regla se prueba con un
    // separador inicial dentro de un contexto numérico.
    let salida = errores("mut x = 1__000;");
    assert!(salida.contains(codes::SEPARADOR_INVALIDO.as_str()));
    assert!(salida.contains("= help:"));
}

#[test]
fn invalido_separador_al_final_del_numero() {
    let salida = errores("mut x = 1000_;");
    assert!(salida.contains(codes::SEPARADOR_INVALIDO.as_str()));
}

#[test]
fn invalido_entero_demasiado_grande() {
    let salida = errores("mut x = 999999999999999999999999999999999999999999;");
    assert!(salida.contains(codes::ENTERO_DEMASIADO_GRANDE.as_str()));
}

// --- Literales de cadena ----------------------------------------------------

#[test]
fn valido_cadena_simple() {
    assert_eq!(tokens(r#""Hola""#)[0], TokenKind::Str("Hola".into()));
}

#[test]
fn valido_cadena_con_escapes() {
    assert_eq!(
        tokens(r#""a\nb\tc\"d\\e""#)[0],
        TokenKind::Str("a\nb\tc\"d\\e".into())
    );
}

#[test]
fn valido_cadena_con_unicode() {
    assert_eq!(
        tokens(r#""café ñandú 日本""#)[0],
        TokenKind::Str("café ñandú 日本".into())
    );
}

#[test]
fn valido_cadena_vacia() {
    assert_eq!(tokens(r#""""#)[0], TokenKind::Str(String::new()));
}

#[test]
fn invalido_cadena_sin_cerrar() {
    let salida = errores("mut x = \"sin cerrar;\n");
    assert!(salida.contains(codes::CADENA_SIN_CERRAR.as_str()));
    assert!(salida.contains("= help:"));
}

#[test]
fn invalido_escape_desconocido() {
    let salida = errores(r#"mut x = "\q";"#);
    assert!(salida.contains(codes::ESCAPE_DESCONOCIDO.as_str()));
    assert!(salida.contains("\\q"));
}

// --- Booleanos --------------------------------------------------------------

#[test]
fn valido_literales_booleanos() {
    assert_eq!(tokens("true")[0], TokenKind::Keyword(Keyword::True));
    assert_eq!(tokens("false")[0], TokenKind::Keyword(Keyword::False));
}

#[test]
fn valido_booleano_no_es_identificador() {
    assert_ne!(tokens("true")[0], TokenKind::Identifier("true".into()));
}

// --- Comentarios ------------------------------------------------------------

#[test]
fn valido_comentario_de_linea() {
    assert_eq!(
        tokens("// nada\nfn"),
        vec![TokenKind::Keyword(Keyword::Fn), TokenKind::Eof]
    );
}

#[test]
fn valido_comentario_de_bloque() {
    assert_eq!(
        tokens("/* nada */ fn"),
        vec![TokenKind::Keyword(Keyword::Fn), TokenKind::Eof]
    );
}

#[test]
fn valido_comentario_de_bloque_multilinea() {
    assert_eq!(
        tokens("/* a\n b\n c */ fn"),
        vec![TokenKind::Keyword(Keyword::Fn), TokenKind::Eof]
    );
}

#[test]
fn invalido_comentario_de_bloque_sin_cerrar() {
    let salida = errores("fn main() { /* sin cerrar");
    assert!(salida.contains(codes::COMENTARIO_SIN_CERRAR.as_str()));
}

// --- Palabras clave ---------------------------------------------------------

#[test]
fn valido_palabras_clave_del_subset() {
    for (texto, esperada) in [
        ("fn", Keyword::Fn),
        ("mut", Keyword::Mut),
        ("inmut", Keyword::Inmut),
        ("if", Keyword::If),
        ("else", Keyword::Else),
        ("return", Keyword::Return),
    ] {
        assert_eq!(
            tokens(texto)[0],
            TokenKind::Keyword(esperada),
            "para `{texto}`"
        );
    }
}

#[test]
fn valido_palabras_clave_de_fases_posteriores_se_reconocen() {
    // No son identificadores: es lo que permite al parser decir "todavía no
    // está implementado" en vez de "token inesperado".
    for (texto, esperada) in [
        ("class", Keyword::Class),
        ("for", Keyword::For),
        ("match", Keyword::Match),
        ("task", Keyword::Task),
        ("import", Keyword::Import),
    ] {
        assert_eq!(
            tokens(texto)[0],
            TokenKind::Keyword(esperada),
            "para `{texto}`"
        );
    }
}

// --- Sensibilidad a mayúsculas ----------------------------------------------

#[test]
fn valido_identificadores_distinguen_capitalizacion() {
    let t = tokens("total Total");
    assert_eq!(t[0], TokenKind::Identifier("total".into()));
    assert_eq!(t[1], TokenKind::Identifier("Total".into()));
    assert_ne!(t[0], t[1]);
}

#[test]
fn valido_una_palabra_clave_en_mayuscula_es_identificador() {
    assert_eq!(tokens("Fn")[0], TokenKind::Identifier("Fn".into()));
}

// --- Operadores -------------------------------------------------------------

#[test]
fn valido_operadores_de_un_caracter() {
    use TokenKind::*;
    assert_eq!(
        tokens("+ - * / % ! < > = ."),
        vec![
            Plus, Minus, Star, Slash, Percent, Not, Lt, Gt, Assign, Dot, Eof
        ]
    );
}

#[test]
fn valido_operadores_de_dos_caracteres() {
    use TokenKind::*;
    assert_eq!(
        tokens("== != <= >= && || :: -> =>"),
        vec![
            Eq, NotEq, LtEq, GtEq, AndAnd, OrOr, ColonColon, Arrow, FatArrow, Eof
        ]
    );
}

#[test]
fn valido_el_operador_mas_largo_gana() {
    // `==` no debe leerse como dos `=`.
    assert_eq!(tokens("==")[0], TokenKind::Eq);
    assert_eq!(tokens("=")[0], TokenKind::Assign);
}

// --- Recuperación -----------------------------------------------------------

#[test]
fn invalido_se_reportan_todos_los_errores_de_una_vez() {
    let source = SourceFile::new("test.zrk", "@ # $");
    let mut sink = DiagnosticSink::new();
    tokenize(&source, &mut sink);

    assert_eq!(
        sink.len(),
        3,
        "el lexer debe seguir tras un error y reportar los tres"
    );
}

#[test]
fn invalido_tras_un_error_se_siguen_produciendo_tokens() {
    let source = SourceFile::new("test.zrk", "@ fn");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);

    assert!(sink.has_errors());
    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Fn));
}

// --- Operadores de fases posteriores ----------------------------------------

#[test]
fn valido_asignacion_compuesta_se_reconoce_como_un_token() {
    use TokenKind::*;
    // Sin esto, `count += 1` lexearía como `+` seguido de `=` y el parser
    // diría "token inesperado" en vez de "todavía no está implementado".
    assert_eq!(
        tokens("+= -= *= /= %="),
        vec![PlusEq, MinusEq, StarEq, SlashEq, PercentEq, Eof]
    );
}

#[test]
fn valido_incremento_y_decremento_son_un_token() {
    use TokenKind::*;
    assert_eq!(tokens("++ --"), vec![PlusPlus, MinusMinus, Eof]);
}

#[test]
fn valido_operadores_de_nullability_y_pipe() {
    use TokenKind::*;
    assert_eq!(
        tokens("? ?? ?. |>"),
        vec![Question, QuestionQuestion, QuestionDot, PipeGt, Eof]
    );
}

#[test]
fn valido_los_operadores_pendientes_declaran_su_fase() {
    assert_eq!(TokenKind::PlusEq.fase(), Some(2));
    assert_eq!(TokenKind::PipeGt.fase(), Some(3));
    // Los del subset no declaran fase pendiente.
    assert_eq!(TokenKind::Plus.fase(), None);
    assert_eq!(TokenKind::Eq.fase(), None);
}

#[test]
fn valido_el_operador_mas_largo_gana_tambien_en_los_compuestos() {
    assert_eq!(tokens("+=")[0], TokenKind::PlusEq);
    assert_eq!(tokens("+")[0], TokenKind::Plus);
    assert_eq!(tokens("??")[0], TokenKind::QuestionQuestion);
    assert_eq!(tokens("?")[0], TokenKind::Question);
}

// --- Sufijo numérico --------------------------------------------------------

#[test]
fn invalido_sufijo_pegado_a_un_numero() {
    let salida = errores("mut x = 123abc;");
    assert!(salida.contains(codes::SUFIJO_NUMERICO_INVALIDO.as_str()));
    assert!(salida.contains("123abc") || salida.contains("abc"));
    // La causa debe hablar del sufijo, no del separador `_`, que no aparece.
    assert!(
        !salida.contains("`_` may only appear"),
        "the cause must not mention the separator:\n{salida}"
    );
}

#[test]
fn invalido_el_sufijo_produce_un_solo_diagnostico() {
    let source = SourceFile::new("test.zrk", "mut x = 123abc;");
    let mut sink = DiagnosticSink::new();
    tokenize(&source, &mut sink);
    assert_eq!(
        sink.len(),
        1,
        "el sufijo entero debe reportarse una sola vez"
    );
}

// --- Comentarios de bloque anidados -----------------------------------------

#[test]
fn valido_los_comentarios_de_bloque_no_anidan() {
    use TokenKind::*;
    // Convención de C: cierra en el primer `*/`. El spec no lo define, así que
    // el comportamiento queda fijado acá para que no cambie por accidente.
    assert_eq!(
        tokens("/* a /* b */ fn"),
        vec![Keyword(zirk_lexer::Keyword::Fn), Eof]
    );
}
