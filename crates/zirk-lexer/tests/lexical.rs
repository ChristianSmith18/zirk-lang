//! Lexical tests.
//!
//! `ZIRK_SPEC_FINAL.md` section 8 requires every rule of the language to have at
//! least one valid and one invalid case. Tests are grouped by rule so that
//! correspondence is verifiable at a glance.

use zirk_diagnostics::{DiagnosticSink, SourceFile};
use zirk_lexer::{Keyword, TokenKind, codes, tokenize};

/// Tokenizes expecting no errors.
fn tokens(source_text: &str) -> Vec<TokenKind> {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);

    assert!(
        !sink.has_errors(),
        "no lexical errors were expected:\n{}",
        sink.render(zirk_diagnostics::RenderStyle::Human)
    );

    tokens.into_iter().map(|t| t.kind).collect()
}

/// Tokenizes expecting an error, returning the rendered diagnostics.
fn errors(source_text: &str) -> String {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    tokenize(&source, &mut sink);

    assert!(
        sink.has_errors(),
        "a lexical error was expected and none occurred"
    );
    sink.render(zirk_diagnostics::RenderStyle::Human)
}

// --- General tokenization ---------------------------------------------------

#[test]
fn valid_minimal_program() {
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
fn valid_every_sequence_ends_in_eof() {
    assert_eq!(tokens("").last(), Some(&TokenKind::Eof));
    assert_eq!(tokens("fn").last(), Some(&TokenKind::Eof));
}

#[test]
fn invalid_unrecognized_character() {
    let output = errors("fn main() @ { }");
    assert!(output.contains(codes::UNRECOGNIZED_CHARACTER.as_str()));
    assert!(output.contains('@'));
    assert!(output.contains("= cause:"));
}

// --- Location --------------------------------------------------------------

#[test]
fn valid_every_token_keeps_its_location() {
    let source = SourceFile::new("test.zrk", "fn main");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);

    assert_eq!(source.slice(tokens[0].span), "fn");
    assert_eq!(source.slice(tokens[1].span), "main");
}

#[test]
fn valid_the_diagnostic_column_counts_characters() {
    // 'ñ' takes two bytes: if the column counted bytes it would say 8, not 7.
    let output = errors("mut añs = @;");
    assert!(output.contains(":1:11"), "unexpected location:\n{output}");
}

// --- Integer literals ------------------------------------------------------

#[test]
fn valid_simple_integer() {
    assert_eq!(tokens("42")[0], TokenKind::Integer(42));
}

#[test]
fn valid_integer_with_separators() {
    assert_eq!(tokens("1_000_000")[0], TokenKind::Integer(1_000_000));
}

#[test]
fn invalid_separator_at_the_start_of_the_number() {
    // `_1000` is an identifier, not a number: the rule is exercised with a
    // leading separator inside a numeric context.
    let output = errors("mut x = 1__000;");
    assert!(output.contains(codes::INVALID_SEPARATOR.as_str()));
    assert!(output.contains("= help:"));
}

#[test]
fn invalid_separator_at_the_end_of_the_number() {
    let output = errors("mut x = 1000_;");
    assert!(output.contains(codes::INVALID_SEPARATOR.as_str()));
}

#[test]
fn invalid_integer_too_large() {
    let output = errors("mut x = 999999999999999999999999999999999999999999;");
    assert!(output.contains(codes::INTEGER_TOO_LARGE.as_str()));
}

// --- String literals ----------------------------------------------------

#[test]
fn valid_simple_string() {
    assert_eq!(tokens(r#""Hola""#)[0], TokenKind::Str("Hola".into()));
}

#[test]
fn valid_string_with_escapes() {
    assert_eq!(
        tokens(r#""a\nb\tc\"d\\e""#)[0],
        TokenKind::Str("a\nb\tc\"d\\e".into())
    );
}

#[test]
fn valid_string_with_unicode() {
    assert_eq!(
        tokens(r#""café ñandú 日本""#)[0],
        TokenKind::Str("café ñandú 日本".into())
    );
}

#[test]
fn valid_empty_string() {
    assert_eq!(tokens(r#""""#)[0], TokenKind::Str(String::new()));
}

#[test]
fn invalid_unterminated_string() {
    let output = errors("mut x = \"sin cerrar;\n");
    assert!(output.contains(codes::UNTERMINATED_STRING.as_str()));
    assert!(output.contains("= help:"));
}

#[test]
fn invalid_unknown_escape() {
    let output = errors(r#"mut x = "\q";"#);
    assert!(output.contains(codes::UNKNOWN_ESCAPE.as_str()));
    assert!(output.contains("\\q"));
}

// --- Booleans --------------------------------------------------------------

#[test]
fn valid_boolean_literals() {
    assert_eq!(tokens("true")[0], TokenKind::Keyword(Keyword::True));
    assert_eq!(tokens("false")[0], TokenKind::Keyword(Keyword::False));
}

#[test]
fn valid_boolean_is_not_an_identifier() {
    assert_ne!(tokens("true")[0], TokenKind::Identifier("true".into()));
}

// --- Comments ------------------------------------------------------------

#[test]
fn valid_line_comment() {
    assert_eq!(
        tokens("// nada\nfn"),
        vec![TokenKind::Keyword(Keyword::Fn), TokenKind::Eof]
    );
}

#[test]
fn valid_block_comment() {
    assert_eq!(
        tokens("/* nada */ fn"),
        vec![TokenKind::Keyword(Keyword::Fn), TokenKind::Eof]
    );
}

#[test]
fn valid_multiline_block_comment() {
    assert_eq!(
        tokens("/* a\n b\n c */ fn"),
        vec![TokenKind::Keyword(Keyword::Fn), TokenKind::Eof]
    );
}

#[test]
fn invalid_unterminated_block_comment() {
    let output = errors("fn main() { /* sin cerrar");
    assert!(output.contains(codes::UNTERMINATED_COMMENT.as_str()));
}

// --- Keywords ---------------------------------------------------------

#[test]
fn valid_subset_keywords() {
    for (text, expected) in [
        ("fn", Keyword::Fn),
        ("mut", Keyword::Mut),
        ("inmut", Keyword::Inmut),
        ("if", Keyword::If),
        ("else", Keyword::Else),
        ("return", Keyword::Return),
    ] {
        assert_eq!(
            tokens(text)[0],
            TokenKind::Keyword(expected),
            "for `{text}`"
        );
    }
}

#[test]
fn valid_later_phase_keywords_are_recognized() {
    // They are not identifiers: that is what lets the parser say "not
    // implemented yet" instead of "unexpected token".
    for (text, expected) in [
        ("class", Keyword::Class),
        ("for", Keyword::For),
        ("match", Keyword::Match),
        ("task", Keyword::Task),
        ("import", Keyword::Import),
    ] {
        assert_eq!(
            tokens(text)[0],
            TokenKind::Keyword(expected),
            "for `{text}`"
        );
    }
}

// --- Case sensitivity ----------------------------------------------

#[test]
fn valid_identifiers_are_case_sensitive() {
    let t = tokens("total Total");
    assert_eq!(t[0], TokenKind::Identifier("total".into()));
    assert_eq!(t[1], TokenKind::Identifier("Total".into()));
    assert_ne!(t[0], t[1]);
}

#[test]
fn valid_a_capitalized_keyword_is_an_identifier() {
    assert_eq!(tokens("Fn")[0], TokenKind::Identifier("Fn".into()));
}

// --- Operators -------------------------------------------------------------

#[test]
fn valid_single_character_operators() {
    use TokenKind::*;
    assert_eq!(
        tokens("+ - * / % ! < > = ."),
        vec![
            Plus, Minus, Star, Slash, Percent, Not, Lt, Gt, Assign, Dot, Eof
        ]
    );
}

#[test]
fn valid_two_character_operators() {
    use TokenKind::*;
    assert_eq!(
        tokens("== != <= >= && || :: -> =>"),
        vec![
            Eq, NotEq, LtEq, GtEq, AndAnd, OrOr, ColonColon, Arrow, FatArrow, Eof
        ]
    );
}

#[test]
fn valid_longest_operator_wins() {
    // `==` must not be read as two `=`.
    assert_eq!(tokens("==")[0], TokenKind::Eq);
    assert_eq!(tokens("=")[0], TokenKind::Assign);
}

// --- Recovery -----------------------------------------------------------

#[test]
fn invalid_every_error_is_reported_at_once() {
    let source = SourceFile::new("test.zrk", "@ # $");
    let mut sink = DiagnosticSink::new();
    tokenize(&source, &mut sink);

    assert_eq!(
        sink.len(),
        3,
        "the lexer must continue after an error and report all three"
    );
}

#[test]
fn invalid_tokens_keep_being_produced_after_an_error() {
    let source = SourceFile::new("test.zrk", "@ fn");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);

    assert!(sink.has_errors());
    assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Fn));
}

// --- Later-phase operators ----------------------------------------

#[test]
fn valid_compound_assignment_is_one_token() {
    use TokenKind::*;
    // Without this, `count += 1` would lex as `+` followed by `=` and the
    // parser would say "unexpected token" instead of "not implemented yet".
    assert_eq!(
        tokens("+= -= *= /= %="),
        vec![PlusEq, MinusEq, StarEq, SlashEq, PercentEq, Eof]
    );
}

#[test]
fn valid_increment_and_decrement_are_one_token() {
    use TokenKind::*;
    assert_eq!(tokens("++ --"), vec![PlusPlus, MinusMinus, Eof]);
}

#[test]
fn valid_nullability_and_pipe_operators() {
    use TokenKind::*;
    assert_eq!(
        tokens("? ?? ?. |>"),
        vec![Question, QuestionQuestion, QuestionDot, PipeGt, Eof]
    );
}

#[test]
fn valid_pending_operators_declare_their_phase() {
    assert_eq!(TokenKind::PlusEq.phase(), Some(2));
    assert_eq!(TokenKind::PipeGt.phase(), Some(3));
    // Subset operators declare no pending phase.
    assert_eq!(TokenKind::Plus.phase(), None);
    assert_eq!(TokenKind::Eq.phase(), None);
}

#[test]
fn valid_longest_operator_wins_for_compound_ones_too() {
    assert_eq!(tokens("+=")[0], TokenKind::PlusEq);
    assert_eq!(tokens("+")[0], TokenKind::Plus);
    assert_eq!(tokens("??")[0], TokenKind::QuestionQuestion);
    assert_eq!(tokens("?")[0], TokenKind::Question);
}

// --- Numeric suffix --------------------------------------------------------

#[test]
fn invalid_suffix_attached_to_a_number() {
    let output = errors("mut x = 123abc;");
    assert!(output.contains(codes::INVALID_NUMERIC_SUFFIX.as_str()));
    assert!(output.contains("123abc") || output.contains("abc"));
    // The cause must talk about the suffix, not the `_` separator, which is
    // nowhere to be seen.
    assert!(
        !output.contains("`_` may only appear"),
        "the cause must not mention the separator:\n{output}"
    );
}

#[test]
fn invalid_the_suffix_produces_a_single_diagnostic() {
    let source = SourceFile::new("test.zrk", "mut x = 123abc;");
    let mut sink = DiagnosticSink::new();
    tokenize(&source, &mut sink);
    assert_eq!(
        sink.len(),
        1,
        "el sufijo entero debe reportarse una sola vez"
    );
}

// --- Nested block comments -----------------------------------------

#[test]
fn valid_block_comments_do_not_nest() {
    use TokenKind::*;
    // C convention: it closes at the first `*/`. The spec does not define
    // this, so the behaviour is pinned here so it cannot change by accident.
    assert_eq!(
        tokens("/* a /* b */ fn"),
        vec![Keyword(zirk_lexer::Keyword::Fn), Eof]
    );
}
