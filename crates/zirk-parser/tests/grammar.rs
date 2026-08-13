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
        Expr::Call(c) => {
            let args: Vec<_> = c.args.iter().map(shape).collect();
            format!("{}({})", c.callee.name, args.join(", "))
        }
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
        ("fn main(): Void { for x in y { } }", "for", "Phase 2"),
        ("fn main(): Void { while a { } }", "while", "Phase 2"),
        ("fn main(): Void { match x { } }", "match", "Phase 2"),
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
fn invalid_import_has_its_own_diagnostic() {
    let output = errors("import { stdout } from std.io;\nfn main(): Void { }");
    assert!(output.contains(codes::MODULES_UNAVAILABLE.as_str()));
    assert!(output.contains("Phase 2"));
}

#[test]
fn invalid_compound_assignment_states_its_phase() {
    let output = errors("fn main(): Void { mut x = 1; x += 1; }");
    assert!(output.contains(codes::NOT_IMPLEMENTED.as_str()));
    assert!(output.contains("+="));
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
