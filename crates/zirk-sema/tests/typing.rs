//! Type-system tests.
//!
//! `ZIRK_SPEC_FINAL.md` section 8 requires one valid and one invalid case per
//! rule. Tests are grouped by rule so that correspondence stays verifiable.

use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile};
use zirk_lexer::tokenize;
use zirk_parser::parse;
use zirk_sema::{check, codes};

/// Checks a full program expecting no errors.
fn accepted(source_text: &str) {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let program = parse(&source, &tokens, &mut sink);
    check(&source, &program, &mut sink);

    assert!(
        !sink.has_errors(),
        "no errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

/// Checks a full program expecting errors, returning them rendered.
fn rejected(source_text: &str) -> String {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let program = parse(&source, &tokens, &mut sink);
    check(&source, &program, &mut sink);

    assert!(sink.has_errors(), "an error was expected and none occurred");
    sink.render(RenderStyle::Human)
}

/// Wraps a body inside a valid `main`.
fn in_main(body: &str) -> String {
    format!("fn main(): Void {{\n{body}\n}}")
}

fn accepted_body(body: &str) {
    accepted(&in_main(body));
}

fn rejected_body(body: &str) -> String {
    rejected(&in_main(body))
}

// --- Entrypoint -------------------------------------------------------------

#[test]
fn valid_minimal_program() {
    accepted("fn main(): Void { }");
}

#[test]
fn invalid_program_without_main() {
    let output = rejected("fn other(): Void { }");
    assert!(output.contains(codes::MISSING_ENTRYPOINT.as_str()));
    assert!(output.contains("= help:"));
}

#[test]
fn invalid_main_with_the_wrong_signature() {
    for source_text in [
        "fn main(): Int32 { return 1; }",
        "fn main(a: Int32): Void { }",
    ] {
        let output = rejected(source_text);
        assert!(
            output.contains(codes::INVALID_ENTRYPOINT.as_str()),
            "for `{source_text}`:\n{output}"
        );
    }
}

// --- Types of the subset ----------------------------------------------------

#[test]
fn valid_the_four_types_of_the_subset() {
    accepted_body("mut a: Int32 = 1;\nmut b: Boolean = true;\nmut c: String = \"x\";\nreturn;");
}

#[test]
fn invalid_type_from_a_later_phase_states_its_phase() {
    let output = rejected_body("mut x: Int64 = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Phase 3"));
}

#[test]
fn invalid_type_that_does_not_exist() {
    let output = rejected_body("mut x: Whatever = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Whatever"));
}

#[test]
fn invalid_variable_of_type_void() {
    let output = rejected_body("mut x: Void = 1;");
    assert!(output.contains(codes::VOID_VARIABLE.as_str()));
}

// --- No implicit conversions ------------------------------------------------

#[test]
fn valid_assignment_of_a_matching_type() {
    accepted_body("mut total: Int32 = 40;");
}

#[test]
fn invalid_assignment_of_an_incompatible_type() {
    let output = rejected_body("mut total: Int32 = \"cuarenta\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("no implicit conversion"));
}

#[test]
fn invalid_arithmetic_between_different_types() {
    let output = rejected_body("mut x = 1 + \"a\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- No truthiness ----------------------------------------------------------

#[test]
fn valid_boolean_condition() {
    accepted_body("mut x: Int32 = 1;\nif x > 0 { }");
}

#[test]
fn invalid_numeric_condition() {
    let output = rejected_body("if 1 { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(
        output.contains("truthiness"),
        "the help must name the reason:\n{output}"
    );
}

#[test]
fn invalid_string_condition() {
    let output = rejected_body("if \"x\" { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Logical operators ------------------------------------------------------

#[test]
fn valid_logical_operators_over_booleans() {
    accepted_body("mut a = true;\nmut b = false;\nmut c = a && b || !a;");
}

#[test]
fn invalid_conjunction_over_integers() {
    let output = rejected_body("mut x = 1 && 2;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_negation_of_a_number() {
    let output = rejected_body("mut x = !5;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Mutability -------------------------------------------------------------

#[test]
fn valid_reassignment_of_a_mutable_variable() {
    accepted_body("mut count: Int32 = 0;\ncount = 1;");
}

#[test]
fn invalid_reassignment_of_an_immutable_variable() {
    let output = rejected_body("inmut NAME: String = \"Zirk\";\nNAME = \"other\";");
    assert!(output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()));
    assert!(output.contains("= help:"));
}

// --- Inference --------------------------------------------------------------

#[test]
fn valid_inference_from_the_initializer() {
    // `mut x = 5` infers Int32: with a single integer type in the subset the
    // inference is unambiguous, which is what `LANGUAGE_SPEC` section 2 asks.
    accepted_body("mut a = 5;\nmut b: Int32 = a;");
}

#[test]
fn valid_inference_of_string_and_boolean() {
    accepted_body("mut a = \"x\";\nmut b: String = a;\nmut c = true;\nmut d: Boolean = c;");
}

#[test]
fn invalid_inferred_type_is_still_checked() {
    let output = rejected_body("mut a = 5;\nmut b: String = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Name resolution --------------------------------------------------------

#[test]
fn valid_use_of_a_declared_variable() {
    accepted_body("mut x: Int32 = 1;\nmut y: Int32 = x;");
}

#[test]
fn invalid_undeclared_identifier() {
    let output = rejected_body("mut y: Int32 = x;");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()));
    assert!(output.contains('x'));
}

#[test]
fn invalid_variable_outside_its_scope() {
    let output = rejected_body("{ mut inner: Int32 = 1; }\nmut y: Int32 = inner;");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()));
}

#[test]
fn valid_inner_block_shadows_the_outer_name() {
    accepted_body("mut x: Int32 = 1;\n{ mut x: String = \"a\"; mut y: String = x; }");
}

// --- Use before initialization ----------------------------------------------

#[test]
fn valid_read_after_assigning() {
    accepted_body("mut x: Int32;\nx = 1;\nmut y: Int32 = x;");
}

#[test]
fn invalid_read_before_assigning() {
    let output = rejected_body("mut x: Int32;\nmut y: Int32 = x;");
    assert!(output.contains(codes::USE_BEFORE_INITIALIZATION.as_str()));
}

// --- Calls ------------------------------------------------------------------

#[test]
fn valid_call_matching_the_signature() {
    accepted(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1, 2); }",
    );
}

#[test]
fn valid_a_function_can_call_one_declared_later() {
    accepted("fn main(): Void { mut x: Int32 = later(); }\nfn later(): Int32 { return 1; }");
}

#[test]
fn invalid_wrong_argument_count() {
    let output = rejected(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1); }",
    );
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

#[test]
fn invalid_argument_of_the_wrong_type() {
    let output = rejected(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1, \"two\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_call_to_an_undeclared_function() {
    let output = rejected_body("missing();");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()));
}

#[test]
fn invalid_two_functions_with_the_same_name() {
    let output = rejected("fn f(): Void { }\nfn f(): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::DUPLICATE_FUNCTION.as_str()));
    assert!(
        output.contains("overloading"),
        "the help must explain there is no overloading:\n{output}"
    );
}

// --- Return coherence -------------------------------------------------------

#[test]
fn valid_return_of_the_declared_type() {
    accepted("fn f(): Int32 { return 1; }\nfn main(): Void { }");
}

#[test]
fn valid_every_path_returns() {
    accepted(
        "fn f(a: Boolean): Int32 { if a { return 1; } else { return 2; } }\nfn main(): Void { }",
    );
}

#[test]
fn invalid_return_of_the_wrong_type() {
    let output = rejected("fn f(): Int32 { return \"x\"; }\nfn main(): Void { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_path_without_a_return() {
    // Without the `else` branch, falling through returns nothing.
    let output = rejected("fn f(a: Boolean): Int32 { if a { return 1; } }\nfn main(): Void { }");
    assert!(output.contains(codes::MISSING_RETURN.as_str()));
}

#[test]
fn invalid_value_returned_from_a_void_function() {
    let output = rejected("fn f(): Void { return 1; }\nfn main(): Void { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

// --- Integer overflow -------------------------------------------------------

#[test]
fn valid_literal_at_the_edge_of_the_range() {
    accepted_body("mut x: Int32 = 2147483647;");
}

#[test]
fn invalid_literal_out_of_range() {
    let output = rejected_body("mut x: Int32 = 2147483648;");
    assert!(output.contains(codes::INTEGER_OUT_OF_RANGE.as_str()));
    assert!(output.contains("Int32"));
}

// --- println ----------------------------------------------------------------

#[test]
fn valid_println_accepts_any_type() {
    // The checker admits any type. Turning the value into a `String` is the
    // lowering's job, and the IR verifier enforces that `Println` only ever
    // receives one — this test used to be read as if it covered that too, and
    // it never did: a value reaching the runtime raw crashed the program.
    accepted_body("stdout.println(\"a\");\nstdout.println(1);\nstdout.println(true);");
}

#[test]
fn valid_the_reference_program_of_the_roadmap() {
    accepted("fn main(): Void {\n    stdout.println(\"Hola desde Zirk\");\n}");
}

// --- Error recovery ---------------------------------------------------------

#[test]
fn invalid_one_error_does_not_cascade() {
    // `x` is undeclared: that is one error. Everything downstream involves an
    // unknown type and must stay silent instead of piling on derived errors.
    let source = SourceFile::new("test.zrk", in_main("mut y: Int32 = x + 1 + 2 + 3;"));
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let program = parse(&source, &tokens, &mut sink);
    check(&source, &program, &mut sink);

    assert_eq!(
        sink.len(),
        1,
        "only the original error was expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn invalid_several_independent_errors_are_all_reported() {
    let source = SourceFile::new(
        "test.zrk",
        in_main("mut a: Int32 = \"x\";\nmut b: Boolean = 1;"),
    );
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let program = parse(&source, &tokens, &mut sink);
    check(&source, &program, &mut sink);

    assert_eq!(
        sink.len(),
        2,
        "both errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}
