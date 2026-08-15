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

/// Textual rendering of an expression with explicit parentheses.
///
/// Makes the shape of the tree verifiable without depending on its internal
/// structure: a precedence test reads `(1 + (2 * 3))` at a glance.
fn shape(e: &Expr) -> String {
    match e {
        Expr::Int(i) => i.value.to_string(),
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
        Expr::Match(m) => format!("match({}, {} arms)", shape(&m.scrutinee), m.arms.len()),
        Expr::Lambda(l) => format!("lambda/{}", l.params.len()),
        Expr::Variant(v) => format!("{}.{}", v.enum_name.name, v.variant.name),
        Expr::Println(p) => format!("println({})", shape(&p.arg)),
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
fn invalid_declaration_without_type_or_initializer() {
    let output = errors("fn main(): Void { mut count; }");
    assert!(output.contains(codes::UNTYPED_DECLARATION.as_str()));
    assert!(output.contains("= help:"));
}

#[test]
fn invalid_inmut_strict_is_not_implemented() {
    let output = errors("fn main(): Void { inmut::strict X: Int32 = 1; }");
    assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()));
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
fn invalid_conditional_body_without_braces() {
    let output = errors("fn main(): Void { if x return; }");
    assert!(output.contains(codes::MISSING_BRACES.as_str()));
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
    assert_eq!(a.target.name, "x");
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
        ("class User { }", "class", "Phase 3"),
        ("fn main(): Void { try { } }", "try", "Phase 4"),
        ("fn main(): Void { task { } }", "task", "Phase 5"),
        ("fn main(): Void { parallel { } }", "parallel", "Phase 5"),
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
    let output = errors("class User { }");
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
        assert!(output.contains("top level"), "for `{source_text}`:\n{output}");
    }
}

#[test]
fn invalid_safe_access_states_its_phase() {
    let output = errors("fn main(): Void { mut u = 1; mut n = u?.name; }");
    assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()), "{output}");
    assert!(output.contains("?."), "{output}");
    assert!(output.contains("Phase 3"), "{output}");
}

#[test]
fn invalid_increment_as_expression_is_rejected() {
    let output = errors("fn main(): Void { mut i = 0; mut x = i++; }");
    assert!(
        output.contains(codes::INCREMENT_AS_EXPRESSION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_variadic_must_be_last() {
    let output = errors("fn f(...xs: Int32, y: Int32): Void { }");
    assert!(output.contains(codes::VARIADIC_NOT_LAST.as_str()), "{output}");
}

#[test]
fn invalid_match_needs_at_least_one_arm() {
    let output = errors("fn main(): Void { match 1 { } }");
    assert!(output.contains(codes::EMPTY_MATCH.as_str()), "{output}");
}

#[test]
fn invalid_enum_with_associated_data_states_its_phase() {
    let output = errors("enum Shape { Circle(Int32) }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()), "{output}");
    assert!(output.contains("Phase 3"), "{output}");
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
    let stmts = statements("for (mut i = 0; i < 10; i++) { }");
    let Stmt::Loop(l) = &stmts[0] else {
        panic!("expected a loop");
    };
    assert_eq!(l.kind, LoopKind::For);
    assert!(l.init.is_some());
    assert!(l.condition.is_some());
    assert!(l.step.is_some());
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
    assert_eq!(a.target.name, b.target.name);
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

// --- Enums and match -------------------------------------------------------

#[test]
fn valid_enum_declaration() {
    let p = program("enum Direction { North, South, East, West }\nfn main(): Void { }");
    let e = &p.enums[0];

    assert_eq!(e.name.name, "Direction");
    assert_eq!(e.variants.len(), 4);
    assert_eq!(e.variants[0].name, "North");
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
