//! Grammar tests.
//!
//! `ZIRK_SPEC_FINAL.md` section 8 requires one valid and one invalid case per
//! rule. Tests are grouped by rule so that correspondence stays verifiable.

use zirk_ast::*;
use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile};
use zirk_lexer::tokenize;
use zirk_parser::{codes, parse};

/// Parses expecting no errors.
fn program(source_text: &str) -> Program {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let parsed = parse(&source, &tokens, &mut sink);

    assert!(
        !sink.has_errors(),
        "no errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
    parsed
}

/// Parses expecting an error, returning the rendered diagnostics.
fn errors(source_text: &str) -> String {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    parse(&source, &tokens, &mut sink);

    assert!(sink.has_errors(), "an error was expected and none occurred");
    sink.render(RenderStyle::Human)
}

/// Parses the body of `main`, for statement-focused tests.
fn statements(body: &str) -> Vec<Stmt> {
    let p = program(&format!("fn main(): Void {{ {body} }}"));
    p.functions.into_iter().next().unwrap().body.statements
}

/// Parses an expression and returns it.
fn expression(expr: &str) -> Expr {
    match statements(&format!("mut x = {expr};")).remove(0) {
        Stmt::Let(l) => l.init.expect("the declaration must have an initializer"),
        other => panic!("expected a declaration, got {other:?}"),
    }
}

/// Debug rendering of statements with every `Span { ... }` removed.
///
/// Lets two trees built from sources of different length be compared for
/// shape, which is what "the same tree" means when the only difference in the
/// source is punctuation that carries no meaning.
fn without_spans(statements: &[Stmt]) -> String {
    let rendered = format!("{statements:?}");
    let mut out = String::with_capacity(rendered.len());
    let mut rest = rendered.as_str();

    while let Some(at) = rest.find("Span {") {
        out.push_str(&rest[..at]);
        // Skip to the matching brace. Spans contain no nested braces of their
        // own, so the first `}` closes it.
        let after = &rest[at..];
        let close = after.find('}').expect("a span is closed");
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Textual rendering of an expression with explicit parentheses.
///
/// Makes the shape of the tree verifiable without depending on its internal
/// structure: a precedence test reads `(1 + (2 * 3))` at a glance.
fn shape(e: &Expr) -> String {
    match e {
        Expr::Int(i) => i.value.to_string(),
        Expr::Float(f) => match &f.width {
            Some(width) => format!("{}{width}", f.text),
            None => f.text.clone(),
        },
        Expr::Str(s) => format!("{:?}", s.value),
        Expr::Bool(b) => b.value.to_string(),
        Expr::Path(i) => i.name.clone(),
        Expr::Unary(u) => format!("({}{})", u.op.as_str(), shape(&u.operand)),
        Expr::Binary(b) => format!("({} {} {})", shape(&b.left), b.op.as_str(), shape(&b.right)),
        Expr::Null(_) => "null".to_string(),
        Expr::Call(c) => {
            let args: Vec<_> = c
                .args
                .iter()
                .map(|a| match &a.name {
                    Some(name) => format!("{}: {}", name.name, shape(&a.value)),
                    None => shape(&a.value),
                })
                .collect();
            format!("{}({})", shape(&c.callee), args.join(", "))
        }
        Expr::Range(r) => format!(
            "({}{}{})",
            shape(&r.start),
            if r.inclusive { "..=" } else { ".." },
            shape(&r.end)
        ),
        Expr::If(i) => format!("if({})", shape(&i.condition)),
        Expr::Ternary(t) => format!(
            "({} ? {} : {})",
            shape(&t.condition),
            shape(&t.when_true),
            shape(&t.when_false)
        ),
        Expr::Increment(i) => match i.fix {
            IncrementFix::Prefix => format!("({}{})", i.op.as_str(), i.target.name()),
            IncrementFix::Postfix => format!("({}{})", i.target.name(), i.op.as_str()),
        },
        Expr::This(_) => "this".to_string(),
        Expr::Super(_) => "super".to_string(),
        Expr::Field(f) => format!(
            "{}{}{}",
            shape(&f.object),
            if f.safe { "?." } else { "." },
            f.name.name
        ),
        Expr::Match(m) => format!("match({}, {} arms)", shape(&m.scrutinee), m.arms.len()),
        Expr::Lambda(l) => format!("lambda/{}", l.params.len()),
        Expr::Variant(v) => format!("{}.{}", v.enum_name.name, v.variant.name),
        Expr::Println(p) => format!("println({})", shape(&p.arg)),
        Expr::Cast(c) => format!("({} as {})", shape(&c.expr), c.target.name),
        Expr::Interpolated(s) => {
            let parts: Vec<_> = s
                .parts
                .iter()
                .map(|p| match p {
                    InterpolatedPart::Literal(text) => format!("{text:?}"),
                    InterpolatedPart::Expr(e) => shape(e),
                })
                .collect();
            format!("interp({})", parts.join(", "))
        }
    }
}

// --- Function declarations -----------------------------------------------

#[test]
fn valid_function_without_parameters() {
    let p = program("fn main(): Void { }");
    let f = &p.functions[0];

    assert_eq!(f.name.name, "main");
    assert!(f.params.is_empty());
    assert_eq!(f.return_type.name, "Void");
}

#[test]
fn valid_function_with_parameters() {
    let p = program("fn add(a: Int32, b: Int32): Int32 { return a + b; }");
    let f = &p.functions[0];

    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name.name, "a");
    assert_eq!(f.params[0].ty.name, "Int32");
    assert_eq!(f.return_type.name, "Int32");
}

#[test]
fn valid_several_functions_in_one_file() {
    let p = program("fn a(): Void { } fn b(): Void { }");
    assert_eq!(p.functions.len(), 2);
}

#[test]
fn invalid_function_without_return_type() {
    let output = errors("fn main() { }");
    assert!(output.contains(codes::MISSING_RETURN_TYPE.as_str()));
    assert!(output.contains("= help:"));
}

#[test]
fn invalid_function_without_name() {
    let output = errors("fn (): Void { }");
    assert!(output.contains(codes::UNEXPECTED_TOKEN.as_str()));
}

// --- Variable declarations -----------------------------------------------

#[test]
fn valid_variable_with_explicit_type() {
    let Stmt::Let(l) = statements("mut count: Int32 = 0;").remove(0) else {
        panic!("expected a declaration");
    };

    assert_eq!(l.mutability, Mutability::Mutable);
    assert_eq!(l.name.name, "count");
    assert_eq!(l.ty.unwrap().name, "Int32");
    assert!(l.init.is_some());
}

#[test]
fn valid_variable_with_inferred_type() {
    let Stmt::Let(l) = statements("mut count = 0;").remove(0) else {
        panic!("expected a declaration");
    };

    assert!(l.ty.is_none());
    assert!(l.init.is_some());
}

#[test]
fn valid_immutable_variable() {
    let Stmt::Let(l) = statements("inmut NAME: String = \"Zirk\";").remove(0) else {
        panic!("expected a declaration");
    };

    assert_eq!(l.mutability, Mutability::Immutable);
}

#[test]
fn valid_strict_variable() {
    let Stmt::Let(l) = statements("inmut::strict s = 0;").remove(0) else {
        panic!("expected a declaration");
    };

    assert_eq!(l.mutability, Mutability::Strict);
    assert_eq!(l.name.name, "s");
}

#[test]
fn invalid_inmut_double_colon_without_strict() {
    let output = errors("fn main(): Void { inmut::frozen s = 0; }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_declaration_without_type_or_initializer() {
    let output = errors("fn main(): Void { mut count; }");
    assert!(output.contains(codes::UNTYPED_DECLARATION.as_str()));
    assert!(output.contains("= help:"));
}

#[test]
fn valid_inmut_strict_is_implemented() {
    let Stmt::Let(l) = statements("inmut::strict X: Int32 = 1;").remove(0) else {
        panic!("expected a declaration");
    };
    assert_eq!(l.mutability, Mutability::Strict);
}

// --- Precedence and associativity --------------------------------------------

#[test]
fn valid_multiplicative_binds_tighter_than_additive() {
    assert_eq!(shape(&expression("1 + 2 * 3")), "(1 + (2 * 3))");
    assert_eq!(shape(&expression("1 * 2 + 3")), "((1 * 2) + 3)");
}

#[test]
fn valid_operators_are_left_associative() {
    assert_eq!(shape(&expression("10 - 4 - 3")), "((10 - 4) - 3)");
    assert_eq!(shape(&expression("8 / 4 / 2")), "((8 / 4) / 2)");
}

#[test]
fn valid_parentheses_override_precedence() {
    assert_eq!(shape(&expression("(1 + 2) * 3")), "((1 + 2) * 3)");
}

#[test]
fn valid_conjunction_binds_tighter_than_disjunction() {
    assert_eq!(shape(&expression("a || b && c")), "(a || (b && c))");
    assert_eq!(shape(&expression("a && b || c")), "((a && b) || c)");
}

#[test]
fn valid_comparison_binds_tighter_than_equality() {
    assert_eq!(shape(&expression("a < b == c")), "((a < b) == c)");
}

#[test]
fn valid_arithmetic_binds_tighter_than_relational() {
    assert_eq!(shape(&expression("1 + 2 < 4")), "((1 + 2) < 4)");
}

#[test]
fn valid_unary_operators() {
    assert_eq!(shape(&expression("-x")), "(-x)");
    assert_eq!(shape(&expression("!flag")), "(!flag)");
}

#[test]
fn valid_unary_binds_tighter_than_binary() {
    assert_eq!(shape(&expression("-a + b")), "((-a) + b)");
}

#[test]
fn invalid_incomplete_expression() {
    let output = errors("fn main(): Void { mut x = 1 + ; }");
    assert!(output.contains(codes::UNEXPECTED_TOKEN.as_str()));
}

// --- Conditional ------------------------------------------------------------

#[test]
fn valid_simple_conditional() {
    let Stmt::If(i) = statements("if x > 0 { }").remove(0) else {
        panic!("expected a conditional");
    };
    assert!(i.else_branch.is_none());
}

#[test]
fn valid_conditional_with_alternative() {
    let Stmt::If(i) = statements("if x > 0 { } else { }").remove(0) else {
        panic!("expected a conditional");
    };
    assert!(matches!(i.else_branch, Some(ElseBranch::Block(_))));
}

#[test]
fn valid_chained_conditional() {
    let Stmt::If(i) = statements("if a { } else if b { } else { }").remove(0) else {
        panic!("expected a conditional");
    };
    assert!(matches!(i.else_branch, Some(ElseBranch::If(_))));
}

#[test]
fn valid_conditional_body_without_braces() {
    // `LANGUAGE_SPEC` section 5: an effect-only `if` may govern one immediate
    // statement. It is wrapped in a block, so it has the shape of the braced
    // form it stands for.
    let Stmt::If(conditional) = statements("if closed return;").remove(0) else {
        panic!("expected a conditional");
    };

    assert_eq!(conditional.then_branch.statements.len(), 1);
    assert!(matches!(
        conditional.then_branch.statements[0],
        Stmt::Return(_)
    ));
    assert!(conditional.else_branch.is_none());
}

#[test]
fn invalid_else_over_a_conditional_without_braces() {
    // Allowing it would bring back the dangling-else ambiguity.
    let output = errors("fn main(): Void { if x return; else return; }");
    assert!(output.contains("without braces"), "{output}");
    assert!(output.contains("= help:"));
}

// --- Calls and return -----------------------------------------------------

#[test]
fn valid_call_with_arguments() {
    assert_eq!(shape(&expression("add(1, 2)")), "add(1, 2)");
}

#[test]
fn valid_call_without_arguments() {
    assert_eq!(shape(&expression("ahora()")), "ahora()");
}

#[test]
fn valid_return_with_value() {
    let Stmt::Return(r) = statements("return a + b;").remove(0) else {
        panic!("expected a return");
    };
    assert!(r.value.is_some());
}

#[test]
fn valid_return_without_value() {
    let Stmt::Return(r) = statements("return;").remove(0) else {
        panic!("expected a return");
    };
    assert!(r.value.is_none());
}

#[test]
fn invalid_unclosed_call() {
    let output = errors("fn main(): Void { add(1, 2; }");
    assert!(output.contains(codes::UNEXPECTED_TOKEN.as_str()));
}

// --- Assignment -------------------------------------------------------------

#[test]
fn valid_assignment_to_variable() {
    let Stmt::Assign(a) = statements("x = 5;").remove(0) else {
        panic!("expected an assignment");
    };
    assert_eq!(a.target.name(), "x");
}

#[test]
fn invalid_assignment_to_a_non_variable() {
    let output = errors("fn main(): Void { 1 = 5; }");
    assert!(output.contains(codes::UNEXPECTED_TOKEN.as_str()));
}

// --- Optional semicolon --------------------------------------------------

#[test]
fn valid_statements_without_semicolons() {
    let con = statements("mut a = 1; mut b = 2;");
    let sin = statements("mut a = 1\n mut b = 2\n");

    assert_eq!(con.len(), sin.len());
    assert_eq!(con.len(), 2);
}

// --- println ----------------------------------------------------------------

#[test]
fn valid_println_with_string() {
    assert_eq!(
        shape(&expression("stdout.println(\"Hola\")")),
        "println(\"Hola\")"
    );
}

#[test]
fn valid_println_with_expression() {
    assert_eq!(
        shape(&expression("stdout.println(a + b)")),
        "println((a + b))"
    );
}

#[test]
fn invalid_other_stdout_method() {
    let output = errors("fn main(): Void { stdout.write(\"x\"); }");
    assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()));
    assert!(output.contains("Phase 7"));
}

// --- Constructs from later phases ------------------------------------

#[test]
fn invalid_constructs_from_other_phases_say_which() {
    for (source_text, text, phase) in [
        ("fn main(): Void { try { } }", "try", "Phase 4"),
        ("fn main(): Void { task { } }", "task", "Phase 5"),
        ("fn main(): Void { parallel { } }", "parallel", "Phase 5"),
        // `**` still needs `Float` (bitwise/shift no longer belong here:
        // they work over `Int32` now, roadmap task 3b/4.4).
        ("fn main(): Void { mut x = 2 ** 3; }", "**", "Phase 3b"),
        // Generators belong to the functional style, not to the objects of
        // Phase 3 they used to be filed under.
        ("fn gen numbers(): Int32 { }", "gen", "Phase 7b"),
        (
            "fn main(): Void { mut a = 1; mut x = a |> f; }",
            "|>",
            "Phase 7b",
        ),
        // `default` labels the catch-all arm of a `try`.
        ("fn main(): Void { default { } }", "default", "Phase 4"),
    ] {
        let output = errors(source_text);
        assert!(
            output.contains(codes::NOT_IMPLEMENTED.as_str()),
            "for `{text}` the not-implemented code is missing:\n{output}"
        );
        assert!(
            output.contains(text),
            "the diagnostic must name `{text}`:\n{output}"
        );
        assert!(
            output.contains(phase),
            "the diagnostic must say `{phase}` for `{text}`:\n{output}"
        );
    }
}

#[test]
fn invalid_constructs_from_other_phases_are_not_unexpected_tokens() {
    let output = errors("fn main(): Void { try { } }");
    assert!(
        !output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "a known construct must not be reported as an unexpected token:\n{output}"
    );
}

#[test]
fn invalid_declaration_inside_a_function_says_where_it_belongs() {
    for source_text in [
        "fn main(): Void { import { a } from \"./b\"; }",
        "fn main(): Void { use stdout; }",
        "fn main(): Void { enum E { A } }",
    ] {
        let output = errors(source_text);
        assert!(
            output.contains(codes::MODULES_UNAVAILABLE.as_str()),
            "for `{source_text}`:\n{output}"
        );
        assert!(
            output.contains("top level"),
            "for `{source_text}`:\n{output}"
        );
    }
}

#[test]
fn valid_member_access_parses_uniformly() {
    // `Direction.North` and `user.name` have the same shape. Telling them
    // apart means knowing whether the base names a type or a value, which is
    // resolution and not parsing, so both produce a field access.
    assert_eq!(shape(&expression("user.name")), "user.name");
    assert_eq!(shape(&expression("Direction.North")), "Direction.North");
    assert_eq!(shape(&expression("a.b.c")), "a.b.c");
    assert_eq!(shape(&expression("user?.name")), "user?.name");
}

#[test]
fn valid_increment_as_expression_keeps_its_fix() {
    // `LANGUAGE_SPEC` section 4 fixes the conventional postfix/prefix
    // semantics, which is the order that used to be missing.
    assert_eq!(shape(&expression("i++")), "(i++)");
    assert_eq!(shape(&expression("++i")), "(++i)");
    assert_eq!(shape(&expression("i--")), "(i--)");
    assert_eq!(shape(&expression("--i")), "(--i)");
}

#[test]
fn valid_increment_as_statement_is_still_an_assignment() {
    // A statement discards the value, so both forms mean `i = i + 1` and
    // nothing downstream needs to know the distinction exists.
    let Stmt::Assign(assignment) = statements("i++;").remove(0) else {
        panic!("expected an assignment");
    };
    assert_eq!(assignment.target.name(), "i");
    assert_eq!(shape(&assignment.value), "(i + 1)");

    let Stmt::Assign(assignment) = statements("++i;").remove(0) else {
        panic!("expected an assignment");
    };
    assert_eq!(shape(&assignment.value), "(i + 1)");
}

#[test]
fn invalid_increment_of_something_that_is_not_a_name() {
    let output = errors("fn main(): Void { mut x = 1++; }");
    assert!(
        output.contains("only a variable can be incremented"),
        "{output}"
    );
}

#[test]
fn invalid_variadic_must_be_last() {
    let output = errors("fn f(...xs: Int32, y: Int32): Void { }");
    assert!(
        output.contains(codes::VARIADIC_NOT_LAST.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_needs_at_least_one_arm() {
    let output = errors("fn main(): Void { match 1 { } }");
    assert!(output.contains(codes::EMPTY_MATCH.as_str()), "{output}");
}

#[test]
fn valid_enum_variant_with_associated_data() {
    let p = program(
        "enum Shape { Circle(radius: Int32), Rectangle(w: Int32, h: Int32), Point }
         fn main(): Void { }",
    );
    let variants = &p.enums[0].variants;

    assert_eq!(variants[0].name.name, "Circle");
    assert_eq!(variants[0].associated.len(), 1);
    assert_eq!(variants[0].associated[0].name.name, "radius");
    assert_eq!(variants[0].associated[0].ty.name, "Int32");

    assert_eq!(variants[1].name.name, "Rectangle");
    assert_eq!(variants[1].associated.len(), 2);
    assert_eq!(variants[1].associated[0].name.name, "w");
    assert_eq!(variants[1].associated[1].name.name, "h");

    assert_eq!(variants[2].name.name, "Point");
    assert!(variants[2].associated.is_empty());
}

#[test]
fn valid_enum_variant_with_an_explicit_mapping() {
    let p = program("enum Status { Ok -> 200, NotFound -> 404 }\nfn main(): Void { }");
    let variants = &p.enums[0].variants;

    assert_eq!(variants[0].name.name, "Ok");
    assert!(variants[0].associated.is_empty());
    assert!(variants[0].mapping.is_some());
}

#[test]
fn invalid_unclosed_enum_variant_associated_data() {
    let output = errors("enum Shape { Circle(radius: Int32 }\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_import_source_must_be_a_path_or_a_module() {
    let output = errors("import { a } from 42;\nfn main(): Void { }");
    assert!(
        output.contains(codes::INVALID_IMPORT_SOURCE.as_str()),
        "{output}"
    );
}

// --- Loops -----------------------------------------------------------------

#[test]
fn valid_while_loop() {
    let stmts = statements("while x > 0 { }");
    let Stmt::Loop(l) = &stmts[0] else {
        panic!("expected a loop, got {:?}", stmts[0]);
    };
    assert_eq!(l.kind, LoopKind::While);
    assert!(l.condition.is_some());
    assert!(l.init.is_none() && l.step.is_none());
}

#[test]
fn valid_loop_is_unconditional() {
    let stmts = statements("loop { break; }");
    let Stmt::Loop(l) = &stmts[0] else {
        panic!("expected a loop");
    };
    assert_eq!(l.kind, LoopKind::Loop);
    assert!(l.condition.is_none());
    assert!(matches!(l.body.statements[0], Stmt::Break(_)));
}

#[test]
fn valid_for_with_three_clauses() {
    // The canonical form of `LANGUAGE_SPEC` section 5, without parentheses.
    let stmts = statements("for mut i = 0; i < 10; i++ { }");
    let Stmt::Loop(l) = &stmts[0] else {
        panic!("expected a loop");
    };
    assert_eq!(l.kind, LoopKind::For);
    assert!(l.init.is_some());
    assert!(l.condition.is_some());
    assert!(l.step.is_some());
}

#[test]
fn valid_control_headers_accept_optional_parentheses() {
    // The parentheses are optional in every control structure and produce the
    // same tree. The canonical style omits them.
    for (bare, parenthesized) in [
        (
            "for mut i = 0; i < 10; i++ { }",
            "for (mut i = 0; i < 10; i++) { }",
        ),
        ("for x in 0..10 { }", "for (x in 0..10) { }"),
        ("while ready { }", "while (ready) { }"),
        ("if ready { }", "if (ready) { }"),
        ("do { } while ready;", "do { } while (ready);"),
    ] {
        // Spans are compared away: the parenthesized source is two characters
        // longer, so every offset in it differs. What must match is the shape.
        assert_eq!(
            without_spans(&statements(bare)),
            without_spans(&statements(parenthesized)),
            "`{bare}` and `{parenthesized}` must produce the same tree"
        );
    }
}

#[test]
fn valid_do_while_runs_its_body_before_its_condition() {
    let stmts = statements("do { poll(); } while pending;");
    let Stmt::Loop(l) = &stmts[0] else {
        panic!("expected a loop");
    };
    assert_eq!(l.kind, LoopKind::DoWhile);
    assert!(l.kind.body_runs_first());
    assert!(l.condition.is_some());
    assert_eq!(l.body.statements.len(), 1);
}

#[test]
fn invalid_do_without_its_while() {
    let output = errors("fn main(): Void { do { } }");
    assert!(output.contains("expected `while`"), "{output}");
}

#[test]
fn valid_ternary_groups_to_the_right() {
    assert_eq!(shape(&expression("a ? b : c")), "(a ? b : c)");
    // Level 16 of the operator table is right-associative.
    assert_eq!(
        shape(&expression("a ? b : c ? d : e")),
        "(a ? b : (c ? d : e))"
    );
    // It binds looser than every binary operator.
    assert_eq!(
        shape(&expression("x > 0 ? x + 1 : x - 1")),
        "((x > 0) ? (x + 1) : (x - 1))"
    );
}

#[test]
fn invalid_ternary_without_its_alternative() {
    let output = errors("fn main(): Void { mut x = a ? b; }");
    assert!(output.contains("expected `:`"), "{output}");
}

#[test]
fn valid_for_in_over_a_range() {
    let stmts = statements("for i in 0..10 { }");
    let Stmt::ForIn(f) = &stmts[0] else {
        panic!("expected a for-in, got {:?}", stmts[0]);
    };
    assert_eq!(f.binding.name, "i");
    assert_eq!(shape(&f.iterable), "(0..10)");
}

#[test]
fn valid_inclusive_range() {
    assert_eq!(shape(&expression("0..=10")), "(0..=10)");
}

#[test]
fn valid_break_and_continue() {
    let stmts = statements("loop { break; continue; }");
    let Stmt::Loop(l) = &stmts[0] else {
        panic!("expected a loop");
    };
    assert!(matches!(l.body.statements[0], Stmt::Break(_)));
    assert!(matches!(l.body.statements[1], Stmt::Continue(_)));
}

// --- Compound assignment and increment -------------------------------------

#[test]
fn valid_compound_assignment_desugars() {
    // The whole point of the desugaring is that nothing downstream can tell
    // the two forms apart.
    let compound = statements("total += 5;");
    let explicit = statements("total = total + 5;");

    let (Stmt::Assign(a), Stmt::Assign(b)) = (&compound[0], &explicit[0]) else {
        panic!("expected two assignments");
    };
    assert_eq!(shape(&a.value), shape(&b.value));
    assert_eq!(a.target.name(), b.target.name());
}

#[test]
fn valid_increment_desugars_in_both_positions() {
    for source_text in ["i++;", "++i;"] {
        let stmts = statements(source_text);
        let Stmt::Assign(a) = &stmts[0] else {
            panic!("expected an assignment for `{source_text}`");
        };
        assert_eq!(shape(&a.value), "(i + 1)", "for `{source_text}`");
    }
}

// --- Functions, lambdas and calls ------------------------------------------

#[test]
fn valid_optional_default_and_variadic_parameters() {
    let p = program("fn f(a?: Int32, b: Int32 = 2, ...rest: Int32): Void { }");
    let params = &p.functions[0].params;

    assert!(params[0].optional);
    assert!(params[1].default.is_some());
    assert!(params[2].variadic);
}

#[test]
fn valid_named_arguments() {
    assert_eq!(shape(&expression("f(name: 1, 2)")), "f(name: 1, 2)");
}

#[test]
fn valid_lambda_with_expression_body() {
    let e = expression("(a: Int32, b: Int32): Int32 => a + b");
    let Expr::Lambda(l) = &e else {
        panic!("expected a lambda, got {e:?}");
    };
    assert_eq!(l.params.len(), 2);
    assert_eq!(l.return_type.name, "Int32");
    assert!(matches!(*l.body, LambdaBody::Expr(_)));
}

#[test]
fn valid_lambda_with_block_body() {
    let e = expression("(): Void => { stdout.println(\"ok\"); }");
    let Expr::Lambda(l) = &e else {
        panic!("expected a lambda");
    };
    assert!(matches!(*l.body, LambdaBody::Block(_)));
}

#[test]
fn valid_parenthesized_expression_is_not_a_lambda() {
    // Both start with `(`; only the `:` after the closing paren tells them
    // apart.
    assert_eq!(shape(&expression("(1 + 2) * 3")), "((1 + 2) * 3)");
}

// --- Type aliases --------------------------------------------------------------

#[test]
fn valid_type_alias_declaration() {
    let p = program("type UserId = Int32;\nfn main(): Void { }");
    let a = &p.type_aliases[0];

    assert_eq!(a.name.name, "UserId");
    assert_eq!(a.target.name, "Int32");
}

#[test]
fn invalid_type_alias_without_a_target() {
    let output = errors("type UserId = ;\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

// --- Casts -------------------------------------------------------------------

#[test]
fn valid_postfix_cast() {
    assert_eq!(shape(&expression("value as String")), "(value as String)");
}

#[test]
fn valid_prefix_cast() {
    let e = expression("<String>value");
    let Expr::Cast(c) = &e else {
        panic!("expected a cast, got {e:?}");
    };
    assert_eq!(c.target.name, "String");
    assert!(matches!(*c.expr, Expr::Path(_)));
}

#[test]
fn valid_prefix_cast_chains_a_field_access_after_it() {
    // `<T>(x).field` casts `(x)`, then reads `.field` off the cast — not the
    // other way around.
    let e = expression("<CustomObject>(obj).field");
    let Expr::Field(f) = &e else {
        panic!("expected a field access, got {e:?}");
    };
    assert!(matches!(*f.object, Expr::Cast(_)));
}

#[test]
fn valid_postfix_cast_binds_tighter_than_a_following_call() {
    let e = expression("value as String");
    assert_eq!(shape(&e), "(value as String)");
}

#[test]
fn invalid_prefix_cast_missing_its_closing_angle_bracket() {
    let output = errors("fn main(): Void { mut x = <String value; }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_postfix_cast_without_a_target_type() {
    let output = errors("fn main(): Void { mut x = value as ; }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

// --- Function types (rejected on purpose, D9) --------------------------------

#[test]
fn invalid_function_type_annotation_states_its_reason() {
    for source in [
        "fn apply(f: Fn(Int32) => Int32): Void { }\nfn main(): Void { }",
        "fn apply(f: Function(Int32) => Int32): Void { }\nfn main(): Void { }",
    ] {
        let output = errors(source);
        assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()), "{output}");
        assert!(output.contains("not implemented"), "{output}");
    }
}

#[test]
fn invalid_function_type_is_not_an_unexpected_token() {
    let output = errors("fn apply(f: Fn(Int32) => Int32): Void { }\nfn main(): Void { }");
    assert!(
        !output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "a known construct must not be reported as an unexpected token:\n{output}"
    );
}

// --- Union types -----------------------------------------------------------

#[test]
fn valid_union_type_on_a_parameter() {
    let p = program("fn f(x: String | Int32): Void { }\nfn main(): Void { }");
    let ty = &p.functions[0].params[0].ty;

    assert_eq!(ty.name, "String");
    assert_eq!(ty.union_with.len(), 1);
    assert_eq!(ty.union_with[0].name, "Int32");
}

#[test]
fn valid_union_type_with_several_alternatives() {
    let p = program("fn f(x: A | B | C): Void { }\nfn main(): Void { }");
    let ty = &p.functions[0].params[0].ty;
    assert_eq!(ty.union_with.len(), 2);
}

#[test]
fn valid_union_type_on_a_field() {
    let p = program("class Box { value: String | Int32; }\nfn main(): Void { }");
    assert_eq!(p.classes[0].fields[0].ty.union_with.len(), 1);
}

#[test]
fn invalid_union_type_missing_an_alternative() {
    let output = errors("fn f(x: String | ): Void { }\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn valid_generic_argument_does_not_admit_a_union() {
    // Out of scope for now: a union inside `<...>` is not parsed as one.
    let p = program(
        "class Box<T> { value: T; }\nclass Holder { b: Box<String>; }\nfn main(): Void { }",
    );
    assert!(p.classes[1].fields[0].ty.arguments[0].union_with.is_empty());
}

// --- Abstract classes ----------------------------------------------------------

#[test]
fn valid_abstract_class_declaration() {
    let p = program(
        "abstract class Shape { name: String; abstract fn area(): Int32; }
         fn main(): Void { }",
    );
    let a = &p.classes[0];

    assert_eq!(a.kind, ClassKind::Abstract);
    assert_eq!(a.fields.len(), 1);
    assert_eq!(a.methods.len(), 1);
    assert!(a.methods[0].is_abstract);
    assert!(a.methods[0].body.is_none());
    assert!(a.constructors.is_empty());
}

#[test]
fn invalid_abstract_method_outside_an_abstract_class_still_states_its_phase() {
    // An ordinary class does not gain `abstract fn` just because the parser
    // now accepts it inside `abstract class`.
    let output = errors("class Shape { abstract fn area(): Int32; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::ABSTRACT_OUTSIDE_ABSTRACT_CLASS.as_str()),
        "{output}"
    );
    assert!(output.contains("abstract"), "{output}");
}

#[test]
fn invalid_abstract_method_with_a_body() {
    let output = errors(
        "abstract class Shape { abstract fn area(): Int32 { return 1; } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

// --- Records and value classes -----------------------------------------------

#[test]
fn valid_record_declaration() {
    let p = program("record Point { x: Float64; y: Float64; }\nfn main(): Void { }");
    let r = &p.classes[0];

    assert_eq!(r.kind, ClassKind::Record);
    assert_eq!(r.name.name, "Point");
    assert_eq!(r.fields.len(), 2);
    assert!(r.constructors.is_empty());
}

#[test]
fn valid_record_with_a_method() {
    let p = program(
        "record Point { x: Float64; y: Float64; fn length(): Float64 { return this.x; } }
         fn main(): Void { }",
    );
    assert_eq!(p.classes[0].methods.len(), 1);
}

#[test]
fn invalid_record_without_a_name() {
    let output = errors("record { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn valid_value_class_declaration() {
    let p = program("value class UserId(value: Int32);\nfn main(): Void { }");
    let v = &p.classes[0];

    assert_eq!(v.kind, ClassKind::ValueClass);
    assert_eq!(v.name.name, "UserId");
    assert_eq!(v.fields.len(), 1);
    assert_eq!(v.fields[0].name.name, "value");
    assert_eq!(v.fields[0].ty.name, "Int32");
    assert!(v.constructors.is_empty());
}

#[test]
fn valid_value_class_with_several_fields() {
    let p = program("value class Point(x: Float64, y: Float64);\nfn main(): Void { }");
    assert_eq!(p.classes[0].fields.len(), 2);
}

#[test]
fn valid_value_does_not_stop_being_an_ordinary_identifier() {
    // `value` is contextual: only `value class` is special.
    let stmts = statements("mut value = 1;");
    let Stmt::Let(l) = &stmts[0] else {
        panic!("expected a declaration");
    };
    assert_eq!(l.name.name, "value");
}

#[test]
fn invalid_unclosed_value_class() {
    let output = errors("value class UserId(value: Int32\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

// --- Enums and match -------------------------------------------------------

#[test]
fn valid_enum_declaration() {
    let p = program("enum Direction { North, South, East, West }\nfn main(): Void { }");
    let e = &p.enums[0];

    assert_eq!(e.name.name, "Direction");
    assert_eq!(e.variants.len(), 4);
    assert_eq!(e.variants[0].name.name, "North");
    assert!(e.variants[0].associated.is_empty());
    assert!(e.variants[0].mapping.is_none());
    assert!(!e.shared);
}

#[test]
fn valid_enum_variant_as_a_value() {
    assert_eq!(shape(&expression("Direction.North")), "Direction.North");
}

#[test]
fn valid_match_as_an_expression() {
    let e = expression("match d { Direction.North => 1, _ => 0 }");
    let Expr::Match(m) = &e else {
        panic!("expected a match, got {e:?}");
    };
    assert_eq!(m.arms.len(), 2);
    assert!(matches!(m.arms[0].pattern, Pattern::Variant(_)));
    assert!(matches!(m.arms[1].pattern, Pattern::Wildcard(_)));
}

#[test]
fn valid_variant_pattern_destructures_associated_data() {
    let e = expression("match s { Shape.Circle(radius) => radius, Shape.Point => 0, _ => 0 }");
    let Expr::Match(m) = &e else {
        panic!("expected a match, got {e:?}");
    };
    let Pattern::Variant(v) = &m.arms[0].pattern else {
        panic!("expected a variant pattern, got {:?}", m.arms[0].pattern);
    };
    assert_eq!(v.variant.name, "Circle");
    assert_eq!(v.bindings.len(), 1);
    assert!(matches!(v.bindings[0], Pattern::Binding(_)));

    let Pattern::Variant(bare) = &m.arms[1].pattern else {
        panic!("expected a variant pattern, got {:?}", m.arms[1].pattern);
    };
    assert!(bare.bindings.is_empty());
}

#[test]
fn valid_match_as_a_statement() {
    let stmts = statements("match d { _ => { } }");
    let Stmt::Expr(e) = &stmts[0] else {
        panic!("expected an expression statement, got {:?}", stmts[0]);
    };
    assert!(matches!(e.expr, Expr::Match(_)));
}

#[test]
fn valid_patterns_cover_the_forms_of_this_phase() {
    let e = expression("match x { 1 => a, \"s\" => b, true => c, null => d, other => e, _ => f }");
    let Expr::Match(m) = &e else {
        panic!("expected a match");
    };
    assert!(matches!(m.arms[0].pattern, Pattern::Int(_)));
    assert!(matches!(m.arms[1].pattern, Pattern::Str(_)));
    assert!(matches!(m.arms[2].pattern, Pattern::Bool(_)));
    assert!(matches!(m.arms[3].pattern, Pattern::Null(_)));
    assert!(matches!(m.arms[4].pattern, Pattern::Binding(_)));
    assert!(matches!(m.arms[5].pattern, Pattern::Wildcard(_)));
}

#[test]
fn valid_match_with_states_its_phase() {
    let output = errors("fn main(): Void { match with r { } }");
    assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()), "{output}");
    assert!(output.contains("Phase 4"), "{output}");
}

// --- Nullability -----------------------------------------------------------

#[test]
fn valid_nullable_type_annotation() {
    let stmts = statements("mut name: String? = null;");
    let Stmt::Let(l) = &stmts[0] else {
        panic!("expected a declaration");
    };
    let ty = l.ty.as_ref().expect("annotated");
    assert_eq!(ty.name, "String");
    assert!(ty.nullable);
    assert!(matches!(l.init, Some(Expr::Null(_))));
}

#[test]
fn valid_coalescing_binds_tighter_than_logical_operators() {
    assert_eq!(shape(&expression("a ?? b || c")), "((a ?? b) || c)");
}

#[test]
fn valid_coalescing_binds_tighter_than_comparison() {
    // Otherwise `name ?? "x" == "x"` would coalesce against a boolean, which
    // is the C# gotcha this precedence avoids.
    assert_eq!(shape(&expression("a ?? b == c")), "((a ?? b) == c)");
}

// --- Modules ---------------------------------------------------------------

#[test]
fn valid_import_from_a_local_path() {
    let p = program("import { User, Role } from \"./domain/user\";\nfn main(): Void { }");
    let i = &p.imports[0];

    assert_eq!(i.names.len(), 2);
    assert_eq!(i.names[0].name.name, "User");
    assert!(matches!(&i.source, ImportSource::Local { path, .. } if path == "./domain/user"));
}

#[test]
fn valid_import_with_alias() {
    let p = program("import { Role -> DomainRole } from \"./user\";\nfn main(): Void { }");
    let name = &p.imports[0].names[0];

    assert_eq!(name.name.name, "Role");
    assert_eq!(name.bound_name().name, "DomainRole");
}

#[test]
fn valid_import_from_a_standard_module() {
    let p = program("import { stdout } from std.io;\nfn main(): Void { }");
    assert!(matches!(&p.imports[0].source, ImportSource::Std { path, .. } if path == "std.io"));
}

#[test]
fn valid_use_declaration() {
    let p = program("use stdout;\nfn main(): Void { }");
    assert_eq!(p.uses[0].name.name, "stdout");
}

#[test]
fn valid_share_marks_the_declaration() {
    let p = program("share fn helper(): Void { }\nshare enum E { A }\nfn main(): Void { }");
    assert!(p.functions[0].shared);
    assert!(p.enums[0].shared);
    assert!(!p.functions[1].shared);
}

// --- Location --------------------------------------------------------------

#[test]
fn valid_every_node_exposes_its_span() {
    let source = SourceFile::new("test.zrk", "fn main(): Void { mut x = 42; }");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let p = parse(&source, &tokens, &mut sink);

    let f = &p.functions[0];
    assert_eq!(source.slice(f.name.span), "main");
    assert_eq!(source.slice(f.return_type.span), "Void");

    let Stmt::Let(l) = &f.body.statements[0] else {
        panic!("expected a declaration");
    };
    assert_eq!(source.slice(l.name.span), "x");
    assert_eq!(source.slice(l.init.as_ref().unwrap().span()), "42");
}

// --- Recovery -----------------------------------------------------------

#[test]
fn invalid_one_error_does_not_prevent_parsing_the_rest() {
    let source = SourceFile::new("test.zrk", "fn a() { }\nfn b(): Void { }");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let p = parse(&source, &tokens, &mut sink);

    assert!(sink.has_errors());
    assert!(
        p.functions.iter().any(|f| f.name.name == "b"),
        "the valid function after the error must still parse"
    );
}

#[test]
fn invalid_the_parser_always_terminates() {
    // Pathological inputs: what matters is that they do not hang.
    for source_text in ["fn", "fn main(", "fn main(): { {{{", "}}}", "((((", "= = ="] {
        let source = SourceFile::new("test.zrk", source_text);
        let mut sink = DiagnosticSink::new();
        let tokens = tokenize(&source, &mut sink);
        parse(&source, &tokens, &mut sink);
    }
}

// --- No duplicate errors -------------------------------------------------

#[test]
fn invalid_a_token_that_cannot_start_an_expression_is_reported_once() {
    let source = SourceFile::new("test.zrk", "fn main(): Void { mut x = ; }");
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    parse(&source, &tokens, &mut sink);

    let expected_count = sink
        .diagnostics()
        .iter()
        .filter(|d| d.message.contains("expected an expression"))
        .count();

    assert_eq!(
        expected_count,
        1,
        "the same token must not be reported twice:\n{}",
        sink.render(RenderStyle::Human)
    );
}

// --- Diagnostic ordering ----------------------------------------------

#[test]
fn invalid_diagnostics_come_out_in_source_order() {
    // The lexical error is on line 3 and the syntactic one on line 2. Stages
    // emit all lexing first, so without sorting they would come out reversed.
    let source_text = "fn main(): Void {\n    class User { }\n    mut x = 123abc;\n}\n";
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    parse(&source, &tokens, &mut sink);

    let output = sink.render(RenderStyle::Human);
    let line_2 = output
        .find(":2:")
        .expect("there must be an error on line 2");
    let line_3 = output
        .find(":3:")
        .expect("there must be an error on line 3");

    assert!(
        line_2 < line_3,
        "diagnostics must come out top to bottom:\n{output}"
    );
}

#[test]
fn valid_a_negative_literal_is_one_literal() {
    // Otherwise `-2147483648` would be rejected: its magnitude does not fit in
    // `Int32` even though the value does.
    let e = expression("-2147483648");
    let Expr::Int(lit) = &e else {
        panic!("expected an integer literal, got {e:?}");
    };
    assert_eq!(lit.value, -2_147_483_648);
}

#[test]
fn valid_negation_of_a_name_is_still_unary() {
    assert_eq!(shape(&expression("-x")), "(-x)");
}

#[test]
fn invalid_nesting_beyond_the_limit_is_reported_not_crashed() {
    // Recursive descent costs stack: without a limit this aborts the process
    // with no diagnostic at all.
    let source = format!(
        "fn main(): Void {{ mut x = {}1{}; }}",
        "(".repeat(500),
        ")".repeat(500)
    );
    let output = errors(&source);

    assert!(
        output.contains(codes::NESTING_TOO_DEEP.as_str()),
        "{}",
        &output[..200.min(output.len())]
    );
}

// --- Literals from later phases ---------------------------------------------

#[test]
fn invalid_literals_from_other_phases_name_themselves_and_their_phase() {
    for (source_text, what, phase) in [
        (
            "fn main(): Void { mut x = 'a'; }",
            "character literal",
            "Phase 3b",
        ),
        (
            "fn main(): Void { mut x = 250ms; }",
            "duration literal",
            "Phase 7",
        ),
        (
            "fn main(): Void { mut x = re'^a$'; }",
            "regex literal",
            "Phase 7",
        ),
    ] {
        let output = errors(source_text);
        assert!(
            output.contains(codes::NOT_IMPLEMENTED.as_str()),
            "for {what}:\n{output}"
        );
        assert!(
            output.contains(what),
            "the diagnostic must name {what}:\n{output}"
        );
        assert!(
            output.contains(phase),
            "the diagnostic must say {phase} for {what}:\n{output}"
        );
    }
}

#[test]
fn invalid_literal_from_another_phase_reports_once() {
    // Abandoning the statement whole is what keeps a second "expected an
    // expression" — matching no mistake the user made — from following it.
    let output = errors("fn main(): Void { mut x = 'a'; }");
    assert!(
        !output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "a recognized literal must not also be an unexpected token:\n{output}"
    );
}

#[test]
fn valid_float_literal_shapes() {
    assert_eq!(shape(&expression("1.5")), "1.5");
    assert_eq!(shape(&expression("6.02e23")), "6.02e23");
    assert_eq!(shape(&expression("1.5f32")), "1.5f32");
}

#[test]
fn valid_ordinary_string_is_unaffected_by_interpolation() {
    assert_eq!(shape(&expression("\"hola\"")), "\"hola\"");
}

#[test]
fn valid_string_interpolation_parses_its_parts_in_order() {
    assert_eq!(
        shape(&expression(r#""a {1} b {2} c""#)),
        r#"interp("a ", 1, " b ", 2, " c")"#
    );
}

#[test]
fn valid_string_interpolation_escaped_brace_is_literal_text() {
    assert_eq!(
        shape(&expression(r#""\{not interpolated\}""#)),
        r#""{not interpolated}""#
    );
}

#[test]
fn valid_string_interpolation_embeds_a_full_expression() {
    // The braces count depth, so a nested `{...}` inside the expression —
    // here, a lambda's own block — does not close the interpolation early.
    assert_eq!(
        shape(&expression(r#""{1 + 2 * 3}""#)),
        "interp((1 + (2 * 3)))"
    );
}

// --- Clases ------------------------------------------------------------------

#[test]
fn valid_class_with_fields_constructor_and_method() {
    let p = program(
        "class User {
             inmut id: Int32;
             name: String;

             construct(id: Int32, name: String) {
                 this.id = id;
                 this.name = name;
             }

             fn greeting(): String { return this.name; }
         }
         fn main(): Void { }",
    );

    let class = &p.classes[0];
    assert_eq!(class.name.name, "User");
    assert_eq!(class.fields.len(), 2);
    assert_eq!(class.constructors.len(), 1);
    assert_eq!(class.constructors[0].params.len(), 2);
    assert_eq!(class.methods.len(), 1);
    assert_eq!(class.methods[0].name.name, "greeting");
}

#[test]
fn valid_unmodified_field_is_public_mut() {
    // `LANGUAGE_SPEC` section 7: a field with no modifiers is `public mut`.
    let p = program("class User { name: String; }\nfn main(): Void { }");
    let field = &p.classes[0].fields[0];

    assert_eq!(field.visibility, Visibility::Public);
    assert_eq!(field.mutability, Mutability::Mutable);
    assert!(!field.explicit_modifiers);
}

#[test]
fn valid_explicit_modifiers_mean_the_same_and_say_so() {
    let p = program("class User { public mut name: String; }\nfn main(): Void { }");
    let field = &p.classes[0].fields[0];

    assert_eq!(field.visibility, Visibility::Public);
    assert_eq!(field.mutability, Mutability::Mutable);
    assert!(field.explicit_modifiers, "the source did write them");
}

#[test]
fn valid_field_visibility_and_mutability() {
    let p = program(
        "class User { private inmut id: Int32; protected mut role: String; }
         fn main(): Void { }",
    );
    let fields = &p.classes[0].fields;

    assert_eq!(fields[0].visibility, Visibility::Private);
    assert_eq!(fields[0].mutability, Mutability::Immutable);
    assert_eq!(fields[1].visibility, Visibility::Protected);
    assert_eq!(fields[1].mutability, Mutability::Mutable);
}

#[test]
fn valid_several_constructors() {
    // `LANGUAGE_SPEC` section 7 admits more than one when their effective
    // signatures differ. Which ones are valid is a semantic question.
    let p = program(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
             construct() { this.x = 0; }
         }
         fn main(): Void { }",
    );

    assert_eq!(p.classes[0].constructors.len(), 2);
}

#[test]
fn valid_shared_class() {
    let p = program("share class User { name: String; }\nfn main(): Void { }");
    assert!(p.classes[0].shared);
}

#[test]
fn invalid_class_without_a_name() {
    let output = errors("class { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_field_without_a_type() {
    let output = errors("class User { name; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

// --- Genéricos -----------------------------------------------------------

#[test]
fn valid_class_type_parameter() {
    let p = program("class Box<T> { value: T; }\nfn main(): Void { }");
    let class = &p.classes[0];

    assert_eq!(class.type_params.len(), 1);
    assert_eq!(class.type_params[0].name.name, "T");
    assert!(class.type_params[0].constraints.is_empty());
}

#[test]
fn valid_contract_type_parameter() {
    let p = program("interface Box<out T> { fn value(): T; }\nfn main(): Void { }");
    let contract = &p.contracts[0];

    assert_eq!(contract.type_params.len(), 1);
    assert_eq!(contract.type_params[0].name.name, "T");
    assert_eq!(contract.type_params[0].variance, Variance::Out);
}

#[test]
fn valid_enum_type_parameter() {
    let p = program("enum Box<T> { Full(value: T), Empty }\nfn main(): Void { }");
    let e = &p.enums[0];

    assert_eq!(e.type_params.len(), 1);
    assert_eq!(e.type_params[0].name.name, "T");
}

#[test]
fn valid_implements_with_type_arguments() {
    let p = program(
        "class Counter implements Iterable<Int32> { construct() { } }\nfn main(): Void { }",
    );
    let implements = &p.classes[0].implements;

    assert_eq!(implements.len(), 1);
    assert_eq!(implements[0].name, "Iterable");
    assert_eq!(implements[0].arguments.len(), 1);
    assert_eq!(implements[0].arguments[0].name, "Int32");
}

#[test]
fn valid_type_parameter_with_combined_constraints() {
    let p = program("class Box<T from Clone & Serializable> { value: T; }\nfn main(): Void { }");
    let constraints = &p.classes[0].type_params[0].constraints;

    assert_eq!(constraints.len(), 2);
    assert_eq!(constraints[0].name, "Clone");
    assert_eq!(constraints[1].name, "Serializable");
}

#[test]
fn valid_generic_function_and_method() {
    let p = program(
        "fn identity<T>(value: T): T { return value; }
         class Box<T> { value: T; fn get<U>(other: U): U { return other; } }
         fn main(): Void { }",
    );

    assert_eq!(p.functions[0].type_params[0].name.name, "T");
    assert_eq!(p.classes[0].methods[0].type_params[0].name.name, "U");
}

#[test]
fn valid_type_arguments_on_a_type_reference() {
    let p = program("class Box<T> { value: Array<T>; }\nfn main(): Void { }");
    let ty = &p.classes[0].fields[0].ty;

    assert_eq!(ty.name, "Array");
    assert_eq!(ty.arguments.len(), 1);
    assert_eq!(ty.arguments[0].name, "T");
}

#[test]
fn valid_nested_type_arguments_split_the_shift_right_token() {
    // The lexer emits `>>` as one token (`token.rs` explains why), so this
    // exercises the parser splitting it back into two `>` to close both
    // levels of nesting.
    let p = program("class Nested { value: Box<Box<Int32>>; }\nfn main(): Void { }");
    let ty = &p.classes[0].fields[0].ty;

    assert_eq!(ty.name, "Box");
    assert_eq!(ty.arguments[0].name, "Box");
    assert_eq!(ty.arguments[0].arguments[0].name, "Int32");
}

#[test]
fn valid_nullable_generic_type() {
    let p = program("class Holder { value: Box<Int32>?; }\nfn main(): Void { }");
    let ty = &p.classes[0].fields[0].ty;

    assert!(ty.nullable);
    assert_eq!(ty.arguments[0].name, "Int32");
}

#[test]
fn invalid_unclosed_type_parameter_list() {
    let output = errors("class Box<T { value: T; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_unclosed_type_argument_list() {
    let output = errors("class Box { value: Array<Int32; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::UNEXPECTED_TOKEN.as_str()),
        "{output}"
    );
}

#[test]
fn valid_declared_variance_on_a_type_parameter() {
    let p = program("class Box<out T> { value: T; }\nfn main(): Void { }");
    assert_eq!(p.classes[0].type_params[0].variance, Variance::Out);

    let p = program("class Sink<in T> { value: T; }\nfn main(): Void { }");
    assert_eq!(p.classes[0].type_params[0].variance, Variance::In);
}

#[test]
fn valid_type_parameter_without_variance_is_invariant() {
    let p = program("class Box<T> { value: T; }\nfn main(): Void { }");
    assert_eq!(p.classes[0].type_params[0].variance, Variance::Invariant);
}
