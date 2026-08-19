//! Lexical tests.
//!
//! `ZIRK_SPEC_FINAL.md` section 8 requires every rule of the language to have at
//! least one valid and one invalid case. Tests are grouped by rule so that
//! correspondence is verifiable at a glance.

use zirk_diagnostics::{DiagnosticSink, Phase, SourceFile};
use zirk_lexer::{DurationUnit, Keyword, NumberLit, StrPart, TokenKind, codes, tokenize};

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

/// An identifier token, which the operator tests need on both sides.
fn id(name: &str) -> TokenKind {
    TokenKind::Identifier(name.to_string())
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
    // The pipe belongs to the functional style, not to the objects of Phase 3.
    assert_eq!(TokenKind::PipeGt.phase(), Some(Phase::SEVEN_B));
    // Subset operators declare no pending phase.
    assert_eq!(TokenKind::Plus.phase(), None);
    assert_eq!(TokenKind::Eq.phase(), None);
    // Phase 2 retires these from the pending list.
    assert_eq!(TokenKind::PlusEq.phase(), None);
    assert_eq!(TokenKind::PlusPlus.phase(), None);
    assert_eq!(TokenKind::QuestionDot.phase(), None);
}

#[test]
fn valid_range_and_variadic_operators() {
    use TokenKind::*;
    assert_eq!(
        tokens(".. ..= ... ."),
        vec![DotDot, DotDotEq, DotDotDot, Dot, Eof]
    );
}

#[test]
fn valid_range_is_not_confused_with_member_access() {
    use TokenKind::*;
    assert_eq!(tokens("0..10"), vec![Integer(0), DotDot, Integer(10), Eof]);
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

// --- Exponentiation, bitwise and shifts -------------------------------------

#[test]
fn valid_exponentiation_is_one_token_not_two_multiplications() {
    use TokenKind::*;
    assert_eq!(tokens("a ** b"), vec![id("a"), StarStar, id("b"), Eof]);
    assert_eq!(tokens("a **= b"), vec![id("a"), StarStarEq, id("b"), Eof]);
    // The single form is unaffected.
    assert_eq!(tokens("a * b"), vec![id("a"), Star, id("b"), Eof]);
    assert_eq!(tokens("a *= b"), vec![id("a"), StarEq, id("b"), Eof]);
}

#[test]
fn valid_bitwise_operators_are_distinct_from_the_logical_ones() {
    use TokenKind::*;
    // Longest match keeps `&&` and `&` apart, and `||`, `|>` and `|`.
    assert_eq!(tokens("a && b"), vec![id("a"), AndAnd, id("b"), Eof]);
    assert_eq!(tokens("a & b"), vec![id("a"), Amp, id("b"), Eof]);
    assert_eq!(tokens("a || b"), vec![id("a"), OrOr, id("b"), Eof]);
    assert_eq!(tokens("a |> b"), vec![id("a"), PipeGt, id("b"), Eof]);
    assert_eq!(tokens("a | b"), vec![id("a"), Pipe, id("b"), Eof]);
    assert_eq!(tokens("a ^ b"), vec![id("a"), Caret, id("b"), Eof]);
    assert_eq!(tokens("~a"), vec![Tilde, id("a"), Eof]);
}

#[test]
fn valid_shifts_and_their_compound_forms() {
    use TokenKind::*;
    assert_eq!(tokens("a << b"), vec![id("a"), Shl, id("b"), Eof]);
    assert_eq!(tokens("a >> b"), vec![id("a"), Shr, id("b"), Eof]);
    assert_eq!(tokens("a <<= b"), vec![id("a"), ShlEq, id("b"), Eof]);
    assert_eq!(tokens("a >>= b"), vec![id("a"), ShrEq, id("b"), Eof]);
    assert_eq!(tokens("a &= b"), vec![id("a"), AmpEq, id("b"), Eof]);
    assert_eq!(tokens("a |= b"), vec![id("a"), PipeEq, id("b"), Eof]);
    assert_eq!(tokens("a ^= b"), vec![id("a"), CaretEq, id("b"), Eof]);
    // Comparison is unaffected.
    assert_eq!(tokens("a < b"), vec![id("a"), Lt, id("b"), Eof]);
    assert_eq!(tokens("a >= b"), vec![id("a"), GtEq, id("b"), Eof]);
}

#[test]
fn valid_power_operator_declares_its_phase() {
    // `**` needs `Float` to define what a negative exponent means (`2 ** -1`
    // is the mathematical result converted back), so it stays gated until
    // `Float` itself lands — unlike bitwise/shift, which only ever needed
    // `Int32`, already available.
    use TokenKind::*;
    for kind in [StarStar, StarStarEq] {
        assert_eq!(
            kind.phase(),
            Some(Phase::THREE_B),
            "`{}` should announce its phase",
            kind.symbol()
        );
    }
}

#[test]
fn valid_bitwise_and_shift_operators_no_longer_declare_a_phase() {
    // Roadmap task 3b/4.4: bitwise and shift work over `Int32` now.
    use TokenKind::*;
    for kind in [Amp, Pipe, Caret, Tilde, Shl, Shr] {
        assert_eq!(
            kind.phase(),
            None,
            "`{}` should not be gated",
            kind.symbol()
        );
    }
}

#[test]
fn valid_words_of_the_whole_language_are_keywords_not_identifiers() {
    use Keyword::*;
    for (text, keyword) in [
        ("do", Do),
        ("yield", Yield),
        ("interface", Interface),
        ("trait", Trait),
    ] {
        assert_eq!(
            tokens(text),
            vec![TokenKind::Keyword(keyword), TokenKind::Eof],
            "`{text}` must not lex as an identifier"
        );
    }
}

#[test]
fn valid_contextual_words_stay_identifiers() {
    // Reserving these would break `mut value = 1;` and `match r { Ok(value) =>
    // ... }`, which are ordinary Zirk. They are recognized by position.
    for text in ["value", "strict"] {
        assert_eq!(
            tokens(text),
            vec![id(text), TokenKind::Eof],
            "`{text}` must stay an identifier"
        );
    }
}

// --- Numeric literals -------------------------------------------------------

#[test]
fn valid_fractional_literal_is_one_token_not_three() {
    use TokenKind::*;
    // The worst failure this change fixes: `1.5` used to lex as `1`, `.`, `5`
    // in silence, so the language could not even say "not yet".
    assert_eq!(tokens("1.5"), vec![Float(NumberLit::new("1.5")), Eof]);
    assert_eq!(tokens("0.125"), vec![Float(NumberLit::new("0.125")), Eof]);
}

#[test]
fn valid_scientific_notation() {
    use TokenKind::*;
    assert_eq!(tokens("1e2"), vec![Float(NumberLit::new("1e2")), Eof]);
    assert_eq!(
        tokens("6.02e23"),
        vec![Float(NumberLit::new("6.02e23")), Eof]
    );
    assert_eq!(tokens("1e-9"), vec![Float(NumberLit::new("1e-9")), Eof]);
    assert_eq!(tokens("1E+3"), vec![Float(NumberLit::new("1e+3")), Eof]);
}

#[test]
fn valid_float_width_suffix() {
    use TokenKind::*;
    assert_eq!(
        tokens("1.5f32"),
        vec![Float(NumberLit::new("1.5").with_width("f32")), Eof]
    );
}

#[test]
fn valid_range_is_not_read_as_a_fraction() {
    use TokenKind::*;
    // `0..10` must stay a range: a fraction needs a digit right after the dot.
    assert_eq!(tokens("0..10"), vec![Integer(0), DotDot, Integer(10), Eof]);
    assert_eq!(
        tokens("0..=10"),
        vec![Integer(0), DotDotEq, Integer(10), Eof]
    );
}

#[test]
fn valid_member_access_on_an_integer_is_not_a_fraction() {
    use TokenKind::*;
    assert_eq!(
        tokens("1.abs()"),
        vec![Integer(1), Dot, id("abs"), LParen, RParen, Eof]
    );
}

#[test]
fn valid_hexadecimal_and_binary_literals() {
    use TokenKind::*;
    assert_eq!(tokens("0xff"), vec![Integer(255), Eof]);
    assert_eq!(tokens("0XFF"), vec![Integer(255), Eof]);
    assert_eq!(tokens("0b1010"), vec![Integer(10), Eof]);
    assert_eq!(tokens("0xff_ff"), vec![Integer(65535), Eof]);
}

#[test]
fn valid_duration_literals_carry_their_unit() {
    use DurationUnit::*;
    use TokenKind::*;
    for (source_text, unit) in [
        ("10ns", Nanoseconds),
        ("5us", Microseconds),
        ("250ms", Milliseconds),
        ("30s", Seconds),
        ("15m", Minutes),
        ("2h", Hours),
        ("3d", Days),
        ("1w", Weeks),
    ] {
        let digits: String = source_text
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        assert_eq!(
            tokens(source_text),
            vec![Duration(NumberLit::new(digits), unit), Eof],
            "`{source_text}` must carry its unit"
        );
    }
}

#[test]
fn valid_minutes_are_not_months() {
    use DurationUnit::*;
    use TokenKind::*;
    // A calendar month is a `Period`, whose length depends on where it lands.
    assert_eq!(
        tokens("1m"),
        vec![Duration(NumberLit::new("1"), Minutes), Eof]
    );
}

#[test]
fn invalid_numeric_suffix_is_still_diagnosed() {
    let output = errors("123abc");
    assert!(
        output.contains(codes::INVALID_NUMERIC_SUFFIX.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_digit_for_the_base() {
    // `2` is not a binary digit, so it is read as a suffix.
    let output = errors("0b12");
    assert!(
        output.contains(codes::INVALID_NUMERIC_SUFFIX.as_str()),
        "{output}"
    );
}

// --- Character, regex and interpolation -------------------------------------

#[test]
fn valid_character_literal() {
    use TokenKind::*;
    assert_eq!(tokens("'a'"), vec![Char("a".into()), Eof]);
    assert_eq!(tokens("'é'"), vec![Char("é".into()), Eof]);
}

#[test]
fn valid_character_literal_keeps_every_code_point_of_a_grapheme() {
    use TokenKind::*;
    // One family emoji is one grapheme built from several code points. The
    // lexer keeps all of them; whether it is exactly one grapheme is decided
    // by the semantics of `Char`.
    let family = "👨‍👩‍👧‍👦";
    assert!(family.chars().count() > 1);
    assert_eq!(
        tokens(&format!("'{family}'")),
        vec![Char(family.into()), Eof]
    );
}

#[test]
fn valid_escaped_quote_in_a_character_literal() {
    use TokenKind::*;
    assert_eq!(tokens(r"'\''"), vec![Char("'".into()), Eof]);
    assert_eq!(tokens(r"'\n'"), vec![Char("\n".into()), Eof]);
}

#[test]
fn invalid_unterminated_character_points_at_its_opening() {
    let output = errors("'a");
    assert!(
        output.contains(codes::UNTERMINATED_CHARACTER.as_str()),
        "{output}"
    );
    assert!(output.contains("= help:"));
}

#[test]
fn valid_regex_literal_keeps_its_escapes() {
    use TokenKind::*;
    // `\d` means something to the regex engine and nothing to string escapes:
    // resolving it here would destroy the pattern.
    assert_eq!(tokens(r"re'^[0-9]+$'"), vec![Regex("^[0-9]+$".into()), Eof]);
    assert_eq!(tokens(r"re'\d+'"), vec![Regex(r"\d+".into()), Eof]);
}

#[test]
fn invalid_unterminated_regex_points_at_its_opening() {
    let output = errors("re'^[0-9]");
    assert!(
        output.contains(codes::UNTERMINATED_REGEX.as_str()),
        "{output}"
    );
}

#[test]
fn valid_interpolated_string_separates_its_parts() {
    let TokenKind::InterpolatedStr(parts) = &tokens("\"value={value}\"")[0] else {
        panic!("expected an interpolated string");
    };

    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], StrPart::Literal("value=".into()));
    let StrPart::Expr { text, .. } = &parts[1] else {
        panic!("the second part is the embedded expression");
    };
    assert_eq!(text, "value");
    // The braces are not part of the text.
    assert!(!text.contains('{'));
}

#[test]
fn valid_interpolation_closes_on_its_matching_brace() {
    let TokenKind::InterpolatedStr(parts) = &tokens("\"{ f({ x }) }\"")[0] else {
        panic!("expected an interpolated string");
    };

    let StrPart::Expr { text, .. } = &parts[0] else {
        panic!("the whole literal is one interpolation");
    };
    assert_eq!(text.trim(), "f({ x })");
}

#[test]
fn valid_string_without_braces_is_not_interpolated() {
    use TokenKind::*;
    assert_eq!(tokens("\"hola\""), vec![Str("hola".into()), Eof]);
}

#[test]
fn valid_escaped_brace_is_literal_text() {
    use TokenKind::*;
    assert_eq!(tokens(r#""a \{ b""#), vec![Str("a { b".into()), Eof]);
}

#[test]
fn invalid_unterminated_interpolation_points_at_its_opening() {
    let output = errors("\"value={value\"");
    assert!(
        output.contains(codes::UNTERMINATED_INTERPOLATION.as_str()),
        "{output}"
    );
}

// --- Canonical literals -----------------------------------------------------

#[test]
fn valid_string_literals_reach_the_runtime_in_canonical_form() {
    // The same text written composed and decomposed. Which one an editor
    // produces depends on the operating system and the keyboard, and they are
    // indistinguishable on screen — so `ADR-011` makes them equal, and
    // normalizing here is what keeps that promise cheap.
    let composed = "\"h\u{f3}\"";
    let decomposed = "\"ho\u{301}\"";

    assert_ne!(
        composed, decomposed,
        "the two sources must really differ in bytes"
    );
    assert_eq!(
        tokens(composed),
        tokens(decomposed),
        "both spellings must produce the same literal"
    );
}

#[test]
fn valid_character_literals_are_canonical_too() {
    assert_eq!(tokens("'\u{f3}'"), tokens("'o\u{301}'"));
}

#[test]
fn valid_ascii_literals_are_untouched() {
    assert_eq!(
        tokens("\"plain ascii\"")[0],
        TokenKind::Str("plain ascii".into())
    );
}
