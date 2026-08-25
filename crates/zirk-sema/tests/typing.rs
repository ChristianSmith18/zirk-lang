//! Type-system tests.
//!
//! `ZIRK_SPEC_FINAL.md` section 8 requires one valid and one invalid case per
//! rule. Tests are grouped by rule so that correspondence stays verifiable.

use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile, SourceMap};
use zirk_lexer::tokenize;
use zirk_parser::parse;
use zirk_sema::{check, codes};

/// Checks a full program expecting no errors.
fn accepted(source_text: &str) {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("test.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    check(&sources, &program, &mut sink);

    assert!(
        !sink.has_errors(),
        "no errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

/// Checks a full program expecting errors, returning them rendered.
fn rejected(source_text: &str) -> String {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("test.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    check(&sources, &program, &mut sink);

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
    // `Int64`/`Float64`/`Char` and the rest of the scalars in this phase's
    // scope resolve now (roadmap Phase 3b, tasks 4.1/5.1/6.1) — `UInt` is
    // still genuinely pending: the spec never names it as an alias the way
    // `Int`/`Integer` name `Int32`.
    let output = rejected_body("mut x: UInt = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Phase 3b"), "{output}");
}

#[test]
fn valid_short_aliases_of_the_default_integer() {
    // `Int` and `Integer` name `Int32`, which has existed since Phase 1.
    accepted_body("mut a: Int = 1;\nmut b: Integer = 2;\nmut c: Int32 = a + b;");
}

// --- Integer widths (roadmap Phase 3b) ---------------------------------------

#[test]
fn valid_safe_widening_between_integer_widths() {
    // Same signedness, no narrower destination — implicit (`Type::accepts`).
    accepted_body("mut a: Int32 = 1;\nmut b: Int64 = a;\nmut c: Int128 = b;");
}

#[test]
fn invalid_narrowing_is_not_implicit() {
    let output = rejected_body("mut a: Int32 = 1;\nmut b: Int8 = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("no implicit conversion"));
}

#[test]
fn invalid_widening_across_signedness_is_not_implicit() {
    let output = rejected_body("mut a: Int32 = 1;\nmut b: UInt64 = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_explicit_narrowing_with_as() {
    accepted_body("mut a: Int32 = 300;\nmut b = a as Int8;");
}

#[test]
fn valid_explicit_sign_crossing_with_as() {
    accepted_body("mut a: Int32 = -1;\nmut b = a as UInt32;");
}

#[test]
fn invalid_arithmetic_between_different_integer_widths() {
    let output = rejected_body("mut a: Int64 = 1;\nmut b: Int32 = 2;\nmut c = a + b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_negation_of_an_unsigned_width() {
    let output = rejected_body("mut a: UInt32 = 1 as UInt32;\nmut b = -a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("signed"));
}

#[test]
fn valid_bitwise_not_on_an_unsigned_width() {
    accepted_body("mut a: UInt32 = 1 as UInt32;\nmut b = ~a;");
}

#[test]
fn valid_shift_amount_may_be_a_different_width() {
    accepted_body("mut a: UInt32 = 1 as UInt32;\nmut b = a << 2;");
}

#[test]
fn valid_float_family_resolves() {
    accepted_body(
        "mut a: Float64 = 1.5;\nmut b: Float32 = 1.5f32;\nmut c: Float16 = 1.5f16;\nmut d: Float128 = 1.5f128;\nreturn;",
    );
}

#[test]
fn valid_safe_widening_between_float_widths() {
    accepted_body("mut a: Float16 = 1.5f16;\nmut b: Float32 = a;\nmut c: Float64 = b;");
}

#[test]
fn invalid_float_narrowing_is_not_implicit() {
    let output = rejected_body("mut a: Float64 = 1.5;\nmut b: Float16 = a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_explicit_float_narrowing_with_as() {
    accepted_body("mut a: Float64 = 1.5;\nmut b: Float16 = a as Float16;");
}

#[test]
fn valid_explicit_conversion_between_int_and_float() {
    accepted_body("mut a: Int32 = 5;\nmut b: Float64 = a as Float64;\nmut c: Int32 = b as Int32;");
}

#[test]
fn valid_mixed_integer_and_float_arithmetic_produces_float() {
    accepted_body("mut a: Int32 = 2;\nmut b: Float64 = 1.5;\nmut c: Float64 = a + b;");
}

#[test]
fn invalid_arithmetic_between_different_float_widths() {
    let output = rejected_body("mut a: Float32 = 1.5f32;\nmut b: Float64 = 1.5;\nmut c = a + b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_comparison_between_integer_and_float() {
    let output = rejected_body("mut a: Int32 = 1;\nmut b: Float64 = 1.0;\nmut c = a < b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_unary_negation_on_float() {
    accepted_body("mut a: Float64 = 1.5;\nmut b = -a;");
}

#[test]
fn invalid_bitwise_not_on_float() {
    let output = rejected_body("mut a: Float64 = 1.5;\nmut b = ~a;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_char_literal_of_a_single_code_point() {
    accepted_body("mut a: Char = 'a';");
}

#[test]
fn valid_char_literal_of_an_extended_grapheme_cluster() {
    // A family emoji joined by ZWJ: several code points, one grapheme.
    accepted_body("mut a: Char = '👨‍👩‍👧‍👦';");
}

#[test]
fn invalid_char_literal_of_more_than_one_grapheme() {
    let output = rejected_body("mut a: Char = 'ab';");
    assert!(output.contains(codes::INVALID_CHAR_LITERAL.as_str()));
}

#[test]
fn invalid_char_literal_that_is_empty() {
    let output = rejected_body("mut a: Char = '';");
    assert!(output.contains(codes::INVALID_CHAR_LITERAL.as_str()));
}

#[test]
fn valid_char_equality() {
    accepted_body("mut a: Char = 'a';\nmut b: Char = 'a';\nmut c = a == b;");
}

#[test]
fn invalid_char_identity_comparison() {
    let output = rejected_body("mut a: Char = 'a';\nmut b: Char = 'a';\nmut c = a is b;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_temporal_type_states_its_phase() {
    let output = rejected_body("mut x: Instant = 1;");
    assert!(output.contains("Phase 7"), "{output}");
}

#[test]
fn invalid_decimal_is_not_a_type_of_the_language() {
    // The family was removed: announcing a phase would teach a language that
    // does not exist.
    let output = rejected_body("mut x: Decimal64 = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(!output.contains("Phase"), "{output}");
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

#[test]
fn valid_bitwise_and_shift_on_int32() {
    // Roadmap Phase 3b, task 4.4 — over `Int32` for now.
    accepted_body(
        "mut a = 6; mut b = 3;
         mut x = a & b;
         mut y = a | b;
         mut z = a ^ b;
         mut w = ~a;
         mut s = a << 2;
         mut t = a >> 1;",
    );
}

#[test]
fn invalid_bitwise_on_boolean() {
    let output = rejected_body("mut a = true; mut x = a & true;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("requires numbers"));
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

// --- `mut`/`inmut`/`inmut::strict` matrix on object references (D11) --------

#[test]
fn valid_strict_reference_to_a_fresh_object() {
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { inmut::strict s = User(\"ana\"); }",
    );
}

#[test]
fn invalid_reassignment_of_a_strict_variable() {
    let output = rejected_body(
        "class User { name: String; construct(name: String) { this.name = name; } }
         inmut::strict s = User(\"ana\");
         s = User(\"beto\");",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_mutable_alias_of_a_strict_object_reference() {
    // A strict reference cannot produce a mutable alias of its object (D11).
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             inmut::strict s = User(\"ana\");
             mut alias = s;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_strict_reference_acquired_from_an_accessible_mutable_alias() {
    // A strict reference cannot be acquired while a `mut` alias of the same
    // object is still reachable (D11).
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             mut m = User(\"ana\");
             inmut::strict s = m;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

// --- `inmut::strict` projection writes (fase-4e-inmut-strict-proyeccion) ----

#[test]
fn invalid_write_through_strict_projection() {
    let output = rejected(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             inmut::strict p = Point(1);
             p.x = 5;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_write_through_multi_level_strict_projection() {
    // Confirms the root-binding walk is not limited to one field access.
    let output = rejected(
        "class Inner { z: Int32; construct(z: Int32) { this.z = z; } }
         class Middle { inner: Inner; construct(inner: Inner) { this.inner = inner; } }
         class Outer { middle: Middle; construct(middle: Middle) { this.middle = middle; } }
         fn main(): Void {
             inmut::strict o = Outer(Middle(Inner(1)));
             o.middle.inner.z = 5;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_write_through_mut_projection() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             mut p = Point(1);
             p.x = 5;
         }",
    );
}

#[test]
fn valid_write_through_non_strict_inmut_projection() {
    // `inmut` (non-strict) only prohibits rebinding `p` itself; mutating
    // what it reaches is a matter of the field's own `inmut`, not the
    // reference's (`ZIRK_SPEC_FINAL.md`/spec: only `inmut::strict`
    // prohibits referent mutation).
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             inmut p = Point(1);
             p.x = 5;
         }",
    );
}

#[test]
fn invalid_multi_assign_write_through_strict_projection() {
    let output = rejected(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             inmut::strict p = Point(1);
             mut left: Int32 = 0;
             left, p.x = 1, 5;
         }",
    );
    assert!(
        output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_constructor_field_write_unaffected_by_strict_projection_check() {
    // `Expr::This` returns `None` from `root_binding_mutability` (design D1
    // of `fase-4e-inmut-strict-proyeccion`), so the in-constructor exemption
    // keeps working regardless of the new projection check — a regression
    // guard, not new behavior.
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void { Point(1); }",
    );
}

// --- Comma-grouped declarations and simultaneous assignment (roadmap Phase 4d) --

#[test]
fn valid_multi_let_type_and_permission_fan_out() {
    accepted_body(
        "mut first, second: String = \"a\", \"b\";
         inmut third, fourth: Int32 = 1, 2;
         first = \"c\";
         second = \"d\";",
    );
}

#[test]
fn invalid_multi_let_initializer_arity_mismatch() {
    let output = rejected_body("mut a, b: Int32 = 1, 2, 3;");
    assert!(
        output.contains(codes::MULTI_LET_ARITY_MISMATCH.as_str()),
        "{output}"
    );
}

#[test]
fn valid_multi_let_missing_initializer_uses_type_default() {
    // `zirk-type-system`'s "Default initialization": no initializer list
    // still leaves every binding usable, defaulted independently.
    accepted_body(
        "mut a, b: Int32;
         a = a + 1;
         b = b + 1;",
    );
}

#[test]
fn invalid_multi_assign_arity_mismatch() {
    let output = rejected_body(
        "mut left: Int32 = 1;
         mut right: Int32 = 2;
         left, right = right, left, 9;",
    );
    assert!(
        output.contains(codes::MULTI_ASSIGN_ARITY_MISMATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_multi_assign_duplicate_destination() {
    let output = rejected_body(
        "mut a: Int32 = 1;
         mut b: Int32 = 2;
         a, a = b, 3;",
    );
    assert!(
        output.contains(codes::DUPLICATE_ASSIGN_TARGET.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_multi_assign_rebinding_immutable() {
    let output = rejected_body(
        "inmut a: Int32 = 1;
         mut b: Int32 = 2;
         a, b = 5, 6;",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_multi_assign_rebinding_strict() {
    let output = rejected_body(
        "inmut::strict a: Int32 = 1;
         mut b: Int32 = 2;
         a, b = 5, 6;",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_multi_assign_swap_type_checks() {
    accepted_body(
        "mut left: Int32 = 3;
         mut right: Int32 = 4;
         left, right = right, left;",
    );
}

#[test]
fn valid_inmut_alias_of_a_mutable_object_reference() {
    // Only `mut` targets and `inmut::strict` sources trigger the matrix;
    // ordinary `inmut` sharing a mutable object's reference is unrestricted.
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             mut m = User(\"ana\");
             inmut alias = m;
         }",
    );
}

#[test]
fn valid_strict_alias_of_a_strict_object_reference() {
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void {
             inmut::strict s = User(\"ana\");
             inmut::strict alias = s;
         }",
    );
}

#[test]
fn valid_strict_matrix_does_not_apply_to_value_classes() {
    // Value classes are inline, not reference-backed (task 11.5), so the
    // aliasing half of D11 has nothing to police for them. `value class` is
    // itself still gated by `NOT_LOWERED` (task 11.5 lowers it), so this
    // checks that gate is the only error, not the strict-alias one.
    let output = rejected(
        "value class Point(x: Int32, y: Int32);
         fn main(): Void {
             mut m = Point(1, 2);
             inmut::strict s = m;
         }",
    );
    assert!(
        !output.contains(codes::STRICT_ALIAS_VIOLATION.as_str()),
        "{output}"
    );
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
fn invalid_inner_block_shadows_the_outer_name() {
    // There is no ordinary shadowing (D10).
    let output = rejected_body("mut x: Int32 = 1;\n{ mut x: String = \"a\"; mut y: String = x; }");
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

#[test]
fn valid_sibling_blocks_reuse_a_name() {
    // The first `x` goes out of scope before the second one exists, so
    // neither ever hides the other — this is not shadowing.
    accepted_body("{ mut x: Int32 = 1; }\n{ mut x: String = \"a\"; }");
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

// --- String interpolation (roadmap Phase 3b) ---------------------------------

#[test]
fn valid_string_interpolation_of_printable_types() {
    accepted_body(
        "mut name = \"ana\"; mut age = 1;
         mut s = \"Hola, {name}, edad {age}\";",
    );
}

#[test]
fn valid_string_interpolation_yields_string() {
    accepted_body("mut s: String = \"x {1}\";");
}

#[test]
fn invalid_string_interpolation_of_a_type_without_a_text_form() {
    let output = rejected(
        "class User { construct() { } }
         fn main(): Void { mut u = User(); mut s = \"u: {u}\"; }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("cannot be printed"));
}

#[test]
fn valid_printing_of_the_new_scalars() {
    accepted_body(
        "mut a: Int8 = 1 as Int8;\nmut b: UInt64 = 1 as UInt64;\nmut c: Float64 = 1.5;\nmut d: Char = 'z';\nstdout.println(a);\nstdout.println(b);\nstdout.println(c);\nstdout.println(d);",
    );
}

#[test]
fn valid_printing_of_float16() {
    // Prints by widening to Float32 first — always exact, since every f16
    // value is representable in f32 without loss.
    accepted_body("mut a: Float16 = 1.5f16;\nstdout.println(a);");
}

#[test]
fn invalid_printing_of_float128() {
    let output = rejected_body("mut a: Float128 = 1.5f128;\nstdout.println(a);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("cannot be printed"));
}

#[test]
fn valid_printing_of_a_class_with_to_string() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } fn to_string(): String { return \"{this.x}\"; } }
         fn main(): Void { mut p = Point(1); stdout.println(p); }",
    );
}

#[test]
fn valid_interpolation_of_a_class_with_to_string() {
    accepted(
        "class Point { x: Int32; construct(x: Int32) { this.x = x; } fn to_string(): String { return \"{this.x}\"; } }
         fn main(): Void { mut p = Point(1); mut s = \"p: {p}\"; }",
    );
}

#[test]
fn valid_explicit_to_string_on_a_native_scalar() {
    accepted_body("mut a: Int32 = 1;\nmut s: String = a.to_string();");
}

#[test]
fn valid_explicit_to_string_on_a_string_is_identity() {
    accepted_body("mut s: String = \"already\".to_string();");
}

#[test]
fn invalid_explicit_to_string_with_arguments() {
    let output = rejected_body("mut a: Int32 = 1;\nmut s: String = a.to_string(2);");
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

#[test]
fn valid_to_string_through_a_contract_reference() {
    accepted(
        "interface Printable { fn to_string(): String; }
         class Widget implements Printable { construct() { } fn to_string(): String { return \"w\"; } }
         fn describe(p: Printable): Void { stdout.println(p); }
         fn main(): Void { mut w = Widget(); describe(w); }",
    );
}

// --- Deep contextual conversion (roadmap Phase 3b, task 7) ------------------

#[test]
fn valid_context_conversion_over_arithmetic() {
    accepted_body("mut a: Float64 = Float64(3 / 4);");
}

#[test]
fn valid_context_conversion_over_string_concatenation() {
    accepted_body("mut a: String = String(\"x=\" + 42);");
}

#[test]
fn valid_context_conversion_reaches_any_numeric_target() {
    accepted_body("mut a: Int64 = Int64(3 + 4);");
}

#[test]
fn valid_context_conversion_over_unary_negation() {
    accepted_body("mut a: Float64 = Float64(-3 / 4);");
}

#[test]
fn valid_context_conversion_stops_at_a_call() {
    accepted(
        "fn half(n: Int32): Int32 { return n / 2; }
         fn main(): Void { mut a: Float64 = Float64(half(3) + 0.5); }",
    );
}

#[test]
fn invalid_context_conversion_of_an_incompatible_leaf() {
    let output = rejected_body("mut a: Float64 = Float64(true + 4);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_context_conversion_wrong_argument_count() {
    let output = rejected_body("mut a: Float64 = Float64(1, 2);");
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

// --- `Never` and `fatalError` (roadmap Phase 4a) ----------------------------

#[test]
fn valid_never_is_assignable_anywhere() {
    accepted_body("mut a: Int32 = fatalError(\"x\");");
    accepted_body("mut a: String = fatalError(\"x\");");
    accepted_body("mut a: Boolean = fatalError(\"x\");");
}

#[test]
fn valid_never_unifies_at_a_ternary_join() {
    accepted_body("mut c: Boolean = true;\nmut a: Int32 = c ? 5 : fatalError(\"x\");");
    accepted_body("mut c: Boolean = true;\nmut a: Int32 = c ? fatalError(\"x\") : 5;");
}

#[test]
fn valid_never_unifies_at_an_if_expression_join() {
    accepted_body("mut c: Boolean = true;\nmut a: Int32 = if c { 5 } else { fatalError(\"x\") };");
}

#[test]
fn valid_fatal_error_as_a_bare_statement() {
    accepted_body("fatalError(\"x\");");
}

#[test]
fn invalid_fatal_error_wrong_argument_type() {
    let output = rejected_body("fatalError(5);");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_empty_enum_states_to_use_never() {
    let output = rejected("enum Impossible { }\nfn main(): Void { }");
    assert!(output.contains(codes::DUPLICATE_DECLARATION.as_str()));
    assert!(output.contains("Never"));
}

// --- `Result<T,E>` (roadmap Phase 4a) ---------------------------------------

#[test]
fn valid_result_construction_and_match() {
    accepted(
        "fn load(fail: Boolean): Result<Int32, String> {\n\
             if fail { return Result.Error(\"boom\"); }\n\
             return Result.Ok(5);\n\
         }\n\
         fn main(): Void {\n\
             mut r: Result<Int32, String> = load(false);\n\
             match r {\n\
                 Result.Ok(value) => stdout.println(value);\n\
                 Result.Error(error) => stdout.println(error);\n\
             }\n\
         }",
    );
}

#[test]
fn valid_result_seven_in_scope_methods() {
    accepted_body(
        "mut r: Result<Int32, String> = Result.Ok(5);\n\
         mut a: Boolean = r.is_ok();\n\
         mut b: Boolean = r.is_error();\n\
         mut c: Int32? = r.ok_or_null();\n\
         mut d: String? = r.error_or_null();\n\
         mut e: Int32 = r.get_or(0);\n\
         mut f: Int32 = r.unwrap();\n\
         mut g: String = r.unwrap_error();",
    );
}

#[test]
fn invalid_result_unknown_method() {
    let output = rejected_body("mut r: Result<Int32, String> = Result.Ok(5);\nr.map_to_string();");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()));
}

#[test]
fn invalid_result_get_or_wrong_argument_type() {
    let output = rejected_body(
        "mut r: Result<Int32, String> = Result.Ok(5);\nmut a: Int32 = r.get_or(\"nope\");",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn invalid_result_get_or_wrong_argument_count() {
    let output =
        rejected_body("mut r: Result<Int32, String> = Result.Ok(5);\nmut a: Int32 = r.get_or();");
    assert!(output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()));
}

#[test]
fn invalid_result_cannot_be_reimplemented() {
    let output = rejected("enum Result { Ok; }\nfn main(): Void { }");
    assert!(output.contains(codes::DUPLICATE_DECLARATION.as_str()));
}

#[test]
fn invalid_result_discarded_as_a_bare_statement() {
    let output = rejected(
        "fn load(): Result<Int32, String> { return Result.Ok(5); }\n\
         fn main(): Void { load(); }",
    );
    assert!(output.contains(codes::DISCARDED_RESULT.as_str()));
}

#[test]
fn valid_result_discarded_explicitly_with_underscore() {
    accepted(
        "fn load(): Result<Int32, String> { return Result.Ok(5); }\n\
         fn main(): Void { _ = load(); }",
    );
}

// --- Error recovery ---------------------------------------------------------

#[test]
fn invalid_one_error_does_not_cascade() {
    // `x` is undeclared: that is one error. Everything downstream involves an
    // unknown type and must stay silent instead of piling on derived errors.
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        in_main("mut y: Int32 = x + 1 + 2 + 3;"),
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    check(&sources, &program, &mut sink);

    assert_eq!(
        sink.len(),
        1,
        "only the original error was expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

#[test]
fn invalid_several_independent_errors_are_all_reported() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        in_main("mut a: Int32 = \"x\";\nmut b: Boolean = 1;"),
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    check(&sources, &program, &mut sink);

    assert_eq!(
        sink.len(),
        2,
        "both errors were expected:\n{}",
        sink.render(RenderStyle::Human)
    );
}

// --- Ternary, increment and `do ... while` ----------------------------------

#[test]
fn valid_ternary_with_branches_that_agree() {
    accepted_body("mut x = 1;\nmut label: String = x > 0 ? \"yes\" : \"no\";");
}

#[test]
fn invalid_ternary_with_branches_that_disagree() {
    let output = rejected_body("mut x = 1;\nmut label = x > 0 ? \"yes\" : 0;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
    assert!(output.contains("ternary"), "{output}");
}

#[test]
fn invalid_ternary_condition_is_not_boolean() {
    // No truthiness, here as everywhere else.
    let output = rejected_body("mut x = 1;\nmut label = x ? \"yes\" : \"no\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()));
}

#[test]
fn valid_increment_as_expression() {
    accepted_body("mut i = 0;\nmut previous = i++;\nmut next = ++i;");
}

#[test]
fn invalid_increment_of_an_immutable_binding() {
    let output = rejected_body("inmut I = 0;\nmut x = I++;");
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_increment_of_a_non_numeric_value() {
    let output = rejected_body("mut s = \"a\";\nmut x = s++;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_do_while_checks_a_boolean_condition() {
    accepted_body("mut i = 0;\ndo { i += 1; } while i < 10;");
}

#[test]
fn invalid_do_while_condition_is_not_boolean() {
    let output = rejected_body("mut i = 0;\ndo { i += 1; } while i;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_break_inside_a_do_while() {
    accepted_body("do { break; } while true;");
}

#[test]
fn valid_conditional_without_braces() {
    accepted_body("mut closed = true;\nif closed return;");
}

#[test]
fn valid_string_iteration_binds_a_char() {
    // `for ... in` over a `String` produces `Char` (roadmap Phase 3b, task
    // 6.3, retiring the debt tracked since Phase 2).
    accepted_body("mut text = \"hola\";\nfor c in text { mut x: Char = c; }");
}

// --- `Iterable<T>` / `Iterator<T>` (D8, task 6.9/6.10) -----------------------

#[test]
fn valid_range_iteration_is_unaffected_by_iterable() {
    // A range keeps resolving natively — it does not write
    // `implements Iterable<Int32>`, and none of this is expected to gate it.
    accepted_body("mut sum = 0;\nfor i in 0..5 { sum = sum + i; }");
}

#[test]
fn invalid_iterating_a_type_that_does_not_implement_iterable() {
    let output = rejected(
        "class Foo { construct() { } }
         fn main(): Void {
             mut f = Foo();
             for x in f { }
         }",
    );
    assert!(output.contains(codes::NOT_ITERABLE.as_str()), "{output}");
    assert!(output.contains("Iterable"), "{output}");
}

#[test]
fn invalid_reopening_the_native_iterable_contract() {
    let output = rejected("interface Iterable { fn foo(): Int32; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_reopening_the_native_iterator_contract() {
    let output = rejected("trait Iterator { fn foo(): Int32; }\nfn main(): Void { }");
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_reopening_the_native_iteration_enum() {
    let output = rejected("enum Iteration { A, B }\nfn main(): Void { }");
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn valid_a_class_implementing_iterable_lowers() {
    // `implements Iterable<Int32>`/`Iterator<Int32>` are checked for real
    // conformance (`iterator()`/`next()` required with the right signature)
    // and now lower too (roadmap task 13.5): dispatch through a contract's
    // table (10.7) plus a concrete `Iteration<Int32>` (11.3, specialized the
    // way a generic class's own fields are, 11.1).
    accepted(
        "class Counter implements Iterable<Int32> {
             construct() { }
             fn iterator(): Iterator<Int32> { return CounterIterator(); }
         }
         class CounterIterator implements Iterator<Int32> {
             construct() { }
             fn next(): Iteration<Int32> { return Iteration.Done; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_a_class_missing_iterator_still_reports_the_missing_method() {
    // Conformance is real: leaving out `iterator()` is caught the same way
    // missing any other contract method is.
    let output = rejected(
        "class Counter implements Iterable<Int32> {
             construct() { }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_a_class_implementing_iterable_with_the_wrong_element_type() {
    let output = rejected(
        "class WrongIterator implements Iterator<String> {
             construct() { }
             fn next(): Iteration<String> { return Iteration.Done; }
         }
         class Counter implements Iterable<Int32> {
             construct() { }
             fn iterator(): Iterator<Int32> { return WrongIterator(); }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_for_in_over_a_users_iterable_lowers() {
    accepted(
        "class Counter implements Iterable<Int32> {
             construct() { }
             fn iterator(): Iterator<Int32> { return CounterIterator(); }
         }
         class CounterIterator implements Iterator<Int32> {
             construct() { }
             fn next(): Iteration<Int32> { return Iteration.Done; }
         }
         fn main(): Void {
             mut c = Counter();
             for x in c { }
         }",
    );
}

// --- Clases ------------------------------------------------------------------

/// A class plus a `main`, which every program needs.
fn with_class(class: &str, body: &str) -> String {
    format!("{class}\nfn main(): Void {{\n{body}\n}}")
}

const USER: &str = "class User {
    inmut id: Int32;
    name: String;

    construct(id: Int32, name: String) {
        this.id = id;
        this.name = name;
    }

    fn greeting(): String { return this.name; }
}";

#[test]
fn valid_class_with_fields_constructor_and_method() {
    accepted(&with_class(USER, ""));
}

#[test]
fn valid_this_reads_and_writes_its_own_fields() {
    accepted(&with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
             fn bump(): Int32 { this.count = this.count + 1; return this.count; }
         }",
        "",
    ));
}

#[test]
fn invalid_this_outside_a_class() {
    let output = rejected_body("stdout.println(this.name);");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()), "{output}");
    assert!(output.contains("inside a class"), "{output}");
}

#[test]
fn invalid_member_that_does_not_exist() {
    let output = rejected(&with_class(
        "class User {
             name: String;
             construct(name: String) { this.name = name; }
             fn broken(): String { return this.nombre; }
         }",
        "",
    ));
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    // The diagnostic lists what does exist, which is what turns a typo into a
    // fix rather than a search.
    assert!(output.contains("its fields are: name"), "{output}");
}

#[test]
fn invalid_private_member_from_outside() {
    let output = rejected(&with_class(
        "class User {
             private name: String;
             construct(name: String) { this.name = name; }
         }",
        "mut u = User(\"x\");\nstdout.println(u.name);",
    ));
    // A hidden member is a different mistake from one that does not exist.
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
    assert!(output.contains("private"), "{output}");
}

#[test]
fn valid_private_member_from_inside_its_class() {
    accepted(&with_class(
        "class User {
             private name: String;
             construct(name: String) { this.name = name; }
             fn greeting(): String { return this.name; }
         }",
        "",
    ));
}

#[test]
fn valid_omitted_attributes_take_their_type_default() {
    // `LANGUAGE_SPEC` section 7: an omitted attribute receives its type
    // default before any initializer or the constructor runs, so a constructor
    // only has to write what has none.
    accepted(&with_class(
        "class Config {
             name: String;
             count: Int32;
             on: Boolean;
             construct() { }
         }",
        "",
    ));
}

#[test]
fn invalid_constructor_leaves_a_field_that_has_no_default() {
    // A class is a reference with identity: there is no instance to default to.
    let output = rejected(
        "class Inner { construct() { } }
         class Outer { inner: Inner; construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNINITIALIZED_FIELD.as_str()),
        "{output}"
    );
    assert!(output.contains("inner"), "{output}");
}

#[test]
fn valid_field_assigned_in_both_branches_counts_as_initialized() {
    // A syntactic walk, not flow analysis: setting a field in both arms of an
    // `if` is ordinary code, not a mistake.
    accepted(&with_class(
        "class User {
             name: String;
             construct(anonymous: Boolean) {
                 if anonymous { this.name = \"?\"; } else { this.name = \"x\"; }
             }
         }",
        "",
    ));
}

#[test]
fn valid_inmut_field_is_set_by_the_constructor() {
    accepted(&with_class(
        "class User {
             inmut id: Int32;
             construct(id: Int32) { this.id = id; }
         }",
        "",
    ));
}

#[test]
fn invalid_writing_an_inmut_field_from_outside_the_constructor() {
    let output = rejected(&with_class(
        "class User {
             inmut id: Int32;
             construct(id: Int32) { this.id = id; }
             fn reset(): Void { this.id = 0; }
         }",
        "",
    ));
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_construction_with_the_wrong_number_of_arguments() {
    let output = rejected(&with_class(USER, "mut u = User(1);"));
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_construction_with_the_wrong_argument_type() {
    let output = rejected(&with_class(USER, "mut u = User(\"uno\", \"x\");"));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_several_constructors_resolve_by_arity() {
    accepted(&with_class(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
             construct() { this.x = 0; }
         }",
        "mut a = Point(1);\nmut b = Point();",
    ));
}

#[test]
fn invalid_duplicate_class() {
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_duplicate_field() {
    let output = rejected(&with_class(
        "class User { name: String; name: Int32; construct() { this.name = \"x\"; } }",
        "",
    ));
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_member_of_a_type_without_members() {
    let output = rejected_body("mut n = 1;\nstdout.println(n.field);");
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn invalid_class_type_is_not_interchangeable_with_another() {
    // Nominal: two classes with the same members are still different types.
    let output = rejected(
        "class A { x: Int32; construct() { this.x = 0; } }
         class B { x: Int32; construct() { this.x = 0; } }
         fn main(): Void { mut a: A = B(); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_a_class_names_its_own_type_in_a_field() {
    // The class is registered before its fields resolve, so it can name itself.
    accepted(&with_class(
        "class Node { next: Node?; construct() { this.next = null; } }",
        "",
    ));
}

// --- Métodos -----------------------------------------------------------------

const COUNTER: &str = "class Counter {
    count: Int32;
    private step: Int32;

    construct(step: Int32) { this.count = 0; this.step = step; }

    fn bump(): Int32 { this.count = this.count + this.step; return this.count; }
    private fn secret(): Int32 { return this.step; }
}";

#[test]
fn valid_method_call() {
    accepted(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.bump();",
    ));
}

#[test]
fn valid_method_calls_another_through_this() {
    accepted(&with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
             fn bump(): Int32 { this.count = this.count + 1; return this.count; }
             fn twice(): Int32 { this.bump(); return this.bump(); }
         }",
        "",
    ));
}

#[test]
fn invalid_method_that_does_not_exist() {
    let output = rejected(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.jump();",
    ));
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("its methods are"), "{output}");
}

#[test]
fn invalid_calling_a_field() {
    // Naming a field where a method is called is its own mistake: saying "not
    // callable" would send the reader looking for a typo.
    let output = rejected(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.count();",
    ));
    assert!(output.contains(codes::NOT_CALLABLE.as_str()), "{output}");
    assert!(output.contains("is a field, not a method"), "{output}");
}

#[test]
fn invalid_private_method_from_outside() {
    let output = rejected(&with_class(
        COUNTER,
        "mut c = Counter(1);\nmut n = c.secret();",
    ));
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn valid_private_method_from_inside_its_class() {
    accepted(&with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
             private fn secret(): Int32 { return this.count; }
             fn public_view(): Int32 { return this.secret(); }
         }",
        "",
    ));
}

#[test]
fn invalid_method_call_with_the_wrong_argument_type() {
    let output = rejected(&with_class(
        "class Greeter {
             prefix: String;
             construct() { this.prefix = \">\"; }
             fn greet(name: String): String { return name; }
         }",
        "mut g = Greeter();\nmut s = g.greet(1);",
    ));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_method_and_field_sharing_a_name() {
    // `u.name` would have to mean both.
    let output = rejected(&with_class(
        "class User {
             name: String;
             construct() { this.name = \"x\"; }
             fn name(): String { return this.name; }
         }",
        "",
    ));
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_method_without_a_return_on_every_path() {
    let output = rejected(&with_class(
        "class Broken {
             flag: Boolean;
             construct() { this.flag = true; }
             fn value(): Int32 { if this.flag { return 1; } }
         }",
        "",
    ));
    assert!(output.contains(codes::MISSING_RETURN.as_str()), "{output}");
}

// --- Herencia ----------------------------------------------------------------

const HIERARCHY: &str = "class User {
    name: String;
    protected role: String;

    construct(name: String) { this.name = name; this.role = \"user\"; }

    fn describe(): String { return this.name; }
}

class Manager extends User {
    team: Int32;

    construct(name: String, team: Int32) {
        super(name);
        this.role = \"manager\";
        this.team = team;
    }

    override fn describe(): String { return this.role; }
}";

#[test]
fn valid_subclass_inherits_fields_and_methods() {
    accepted(&with_class(
        HIERARCHY,
        "mut m = Manager(\"x\", 1);\nstdout.println(m.name);\nstdout.println(m.describe());",
    ));
}

#[test]
fn valid_subclass_stands_where_its_base_is_expected() {
    accepted(&format!(
        "{HIERARCHY}\nfn greet(u: User): String {{ return u.describe(); }}\n\
         fn main(): Void {{ mut m = Manager(\"x\", 1); stdout.println(greet(m)); }}"
    ));
}

#[test]
fn invalid_base_where_a_subclass_is_expected() {
    // One direction only: accepting the reverse would promise members the
    // value may not have.
    let output = rejected(&format!(
        "{HIERARCHY}\nfn lead(m: Manager): String {{ return m.describe(); }}\n\
         fn main(): Void {{ mut u = User(\"x\"); stdout.println(lead(u)); }}"
    ));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_protected_reaches_the_subclass() {
    accepted(&with_class(HIERARCHY, ""));
}

#[test]
fn invalid_private_does_not_reach_the_subclass() {
    let output = rejected(
        "class Base {
             private hidden: Int32;
             construct() { this.hidden = 0; }
         }
         class Derived extends Base {
             construct() { this.hidden = 1; }
             fn peek(): Int32 { return this.hidden; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
    assert!(output.contains("only inside its own class"), "{output}");
}

#[test]
fn invalid_protected_from_outside_the_hierarchy() {
    let output = rejected(&with_class(
        HIERARCHY,
        "mut u = User(\"x\");\nstdout.println(u.role);",
    ));
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_override_with_a_different_signature() {
    // A call through the base would reach a body expecting something else,
    // which is the one thing the base's type promises it will not.
    let output = rejected(
        "class Base {
             x: Int32;
             construct() { this.x = 0; }
             fn value(): Int32 { return this.x; }
         }
         class Derived extends Base {
             construct() { super(); }
             override fn value(): String { return \"x\"; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("overrides"), "{output}");
}

#[test]
fn invalid_inheritance_cycle() {
    let output = rejected(
        "class A extends B { construct() { } }
         class B extends A { construct() { } }
         fn main(): Void { }",
    );
    assert!(output.contains("inherits from itself"), "{output}");
}

#[test]
fn invalid_extending_something_that_is_not_a_class() {
    let output = rejected("class A extends Int32 { construct() { } }\nfn main(): Void { }");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn invalid_redeclaring_an_inherited_field() {
    let output = rejected(
        "class Base { x: Int32; construct() { this.x = 0; } }
         class Derived extends Base { x: Int32; construct() { this.x = 0; } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(output.contains("inherited field"), "{output}");
}

#[test]
fn invalid_subclass_constructor_leaving_an_inherited_field_unset() {
    // Without `super(...)` the base's constructor never runs, so a field with
    // no default would stay unset.
    let output = rejected(
        "class Inner { construct() { } }
         class Base { inner: Inner; construct() { this.inner = Inner(); } }
         class Derived extends Base { construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::UNINITIALIZED_FIELD.as_str()),
        "{output}"
    );
}

#[test]
fn valid_super_covers_what_the_base_declares() {
    accepted(
        "class Inner { construct() { } }
         class Base { inner: Inner; construct() { this.inner = Inner(); } }
         class Derived extends Base { construct() { super(); } }
         fn main(): Void { }",
    );
}

#[test]
fn valid_super_initializes_a_private_field_of_the_base() {
    // The one thing a subclass could not do before: reach what only the base's
    // own constructor may write.
    accepted(
        "class Base {
             private secret: Inner;
             construct() { this.secret = Inner(); }
         }
         class Inner { construct() { } }
         class Derived extends Base { construct() { super(); } }
         fn main(): Void { }",
    );
}

#[test]
fn valid_super_method_reaches_the_inherited_body() {
    accepted(
        "class Base { fn describe(): String { return \"base\"; } construct() { } }
         class Derived extends Base {
             construct() { }
             override fn describe(): String { return super.describe(); }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_super_outside_a_class() {
    let output = rejected_body("super();");
    assert!(output.contains(codes::UNDECLARED_NAME.as_str()), "{output}");
}

#[test]
fn invalid_super_without_a_base() {
    let output = rejected("class Alone { construct() { super(); } }\nfn main(): Void { }");
    assert!(output.contains("no base class"), "{output}");
}

#[test]
fn invalid_override_without_the_keyword() {
    // Otherwise adding a method to a base silently changes what a subclass
    // means.
    let output = rejected(
        "class Base { fn value(): Int32 { return 1; } construct() { } }
         class Derived extends Base { fn value(): Int32 { return 2; } construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
    assert!(output.contains("override fn value"), "{output}");
}

#[test]
fn invalid_override_that_overrides_nothing() {
    // The mirror mistake, and usually a typo in the name.
    let output = rejected(
        "class Base { fn value(): Int32 { return 1; } construct() { } }
         class Derived extends Base { override fn valeu(): Int32 { return 2; } construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
    assert!(output.contains("overrides nothing"), "{output}");
}

#[test]
fn valid_a_class_may_extend_one_declared_later() {
    accepted(
        "class Derived extends Base { construct() { this.x = 1; } }
         class Base { x: Int32; construct() { this.x = 0; } }
         fn main(): Void { }",
    );
}

// --- Contratos ---------------------------------------------------------------

const CONTRACTS: &str = "interface Describable {
    fn describe(): String;
}

trait Labelled {
    fn label(): String;
    fn shout(): String { return this.label(); }
}

class User implements Describable, Labelled {
    name: String;
    construct(name: String) { this.name = name; }
    fn describe(): String { return this.name; }
    fn label(): String { return \"user\"; }
}";

#[test]
fn valid_class_implements_an_interface_and_a_trait() {
    accepted(&with_class(CONTRACTS, ""));
}

#[test]
fn valid_a_class_stands_where_its_contract_is_expected() {
    accepted(&format!(
        "{CONTRACTS}\nfn announce(d: Describable): String {{ return d.describe(); }}\n\
         fn main(): Void {{ mut u = User(\"x\"); stdout.println(announce(u)); }}"
    ));
}

#[test]
fn valid_a_trait_default_body_is_adopted() {
    // The class never writes `shout`, and still has it.
    accepted(&format!(
        "{CONTRACTS}\nfn main(): Void {{ mut u = User(\"x\"); stdout.println(u.shout()); }}"
    ));
}

#[test]
fn invalid_two_traits_offer_conflicting_defaults() {
    // Neither trait's default should silently win by declaration order (D4).
    let output = rejected(
        "trait Loud { fn greet(): String { return \"HELLO\"; } }
         trait Quiet { fn greet(): String { return \"hello\"; } }
         class Both implements Loud, Quiet { construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(output.contains("greet"), "{output}");
}

#[test]
fn valid_class_own_method_resolves_two_traits_offering_the_same_name() {
    // Writing the method itself is how the class picks, so it is not a
    // conflict: both traits require the same signature and get it.
    accepted(
        "trait Loud { fn greet(): String { return \"HELLO\"; } }
         trait Quiet { fn greet(): String { return \"hello\"; } }
         class Both implements Loud, Quiet {
             construct() { }
             fn greet(): String { return \"hi\"; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_class_missing_what_a_contract_requires() {
    let output = rejected(
        "interface Describable { fn describe(): String; }
         class Robot implements Describable { construct() { } }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
    assert!(output.contains("describe"), "{output}");
}

#[test]
fn invalid_implementation_with_a_different_signature() {
    let output = rejected(
        "interface Describable { fn describe(): String; }
         class Robot implements Describable {
             construct() { }
             fn describe(): Int32 { return 1; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_implementation_that_is_not_public() {
    // A contract declares behaviour anyone may reach.
    let output = rejected(
        "interface Describable { fn describe(): String; }
         class Robot implements Describable {
             construct() { }
             private fn describe(): String { return \"x\"; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::INACCESSIBLE_MEMBER.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_reaching_a_member_the_contract_does_not_declare() {
    // Which class is behind it is what a contract exists not to say.
    let output = rejected(&format!(
        "{CONTRACTS}\nfn announce(d: Describable): String {{ return d.label(); }}\n\
         fn main(): Void {{ }}"
    ));
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

#[test]
fn invalid_constructing_a_contract() {
    let output = rejected(
        "interface Describable { fn describe(): String; }
         fn main(): Void { mut d = Describable(); }",
    );
    assert!(output.contains(codes::NOT_CALLABLE.as_str()), "{output}");
    assert!(output.contains("describes behaviour"), "{output}");
}

#[test]
fn invalid_implementing_something_that_is_not_a_contract() {
    let output = rejected(
        "class Base { construct() { } }
         class Other implements Base { construct() { } }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn valid_a_subclass_satisfies_what_its_base_satisfies() {
    accepted(&format!(
        "{CONTRACTS}
         class Admin extends User {{ construct(name: String) {{ super(name); }} }}
         fn announce(d: Describable): String {{ return d.describe(); }}
         fn main(): Void {{ mut a = Admin(\"x\"); stdout.println(announce(a)); }}"
    ));
}

#[test]
fn invalid_interface_method_with_a_body() {
    // A trait is the one that may carry implementation.
    let output = rejected(
        "interface Describable { fn describe(): String { return \"x\"; } }\nfn main(): Void { }",
    );
    assert!(output.contains("has no body"), "{output}");
}

// --- Contratos de operador ---------------------------------------------------

#[test]
fn valid_string_concatenation() {
    // La deuda de la Fase 2, cerrada.
    accepted_body("mut greeting: String = \"hello\" + \" \" + \"world\";");
}

#[test]
fn valid_string_repetition_in_either_order() {
    accepted_body("mut a: String = \"ja\" * 3;\nmut b: String = 3 * \"ja\";");
}

#[test]
fn invalid_string_minus_string() {
    // Native types expose no undocumented operator.
    let output = rejected_body("mut x = \"a\" - \"b\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_string_plus_a_number_without_a_conversion() {
    // There is no implicit conversion: `String("count=" + 4)` is the form that
    // works, and it arrives with contextual conversion.
    let output = rejected_body("mut x = \"count=\" + 4;");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_user_type_supplies_an_operator() {
    accepted(
        "class Money {
             amount: Int32;
             construct(amount: Int32) { this.amount = amount; }
             fn _add(other: Money): Money { return Money(this.amount + other.amount); }
         }
         fn main(): Void { mut total = Money(30) + Money(12); }",
    );
}

#[test]
fn invalid_operator_a_user_type_does_not_supply() {
    // The diagnostic names the method that would make it work.
    let output = rejected(
        "class Money {
             amount: Int32;
             construct(amount: Int32) { this.amount = amount; }
         }
         fn main(): Void { mut total = Money(1) + Money(2); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("_add"), "{output}");
}

#[test]
fn invalid_operator_method_with_the_wrong_operand() {
    let output = rejected(
        "class Money {
             amount: Int32;
             construct(amount: Int32) { this.amount = amount; }
             fn _add(other: Int32): Money { return this; }
         }
         fn main(): Void { mut total = Money(1) + Money(2); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_reopening_a_native_type() {
    // Application code cannot replace what `String` means.
    for name in ["String", "Int32", "Boolean", "Float"] {
        let output = rejected(&format!(
            "class {name} {{ construct() {{ }} }}\nfn main(): Void {{ }}"
        ));
        assert!(
            output.contains("type of the language"),
            "for `{name}`:\n{output}"
        );
    }
}

// --- Identidad e igualdad ----------------------------------------------------

#[test]
fn invalid_equality_on_a_type_that_has_not_defined_it() {
    // Answering by address would be `is` wearing the wrong operator, and
    // answering `true` because the types match would be worse still.
    let output = rejected(
        "class U { name: String; construct() { this.name = \"a\"; } }
         fn main(): Void { mut a = U(); mut b = U(); stdout.println(a == b); }",
    );
    assert!(output.contains("does not define equality"), "{output}");
    assert!(output.contains("_equals"), "{output}");
}

#[test]
fn valid_equality_through_the_reserved_method() {
    accepted(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
             fn _equals(other: Point): Boolean { return this.x == other.x; }
         }
         fn main(): Void { mut a = Point(1); stdout.println(a == Point(1)); }",
    );
}

#[test]
fn valid_identity_needs_no_contract() {
    // Identity *is* the address, so there is nothing to define.
    accepted(
        "class U { construct() { } }
         fn main(): Void { mut a = U(); mut b = a; stdout.println(a is b); }",
    );
}

#[test]
fn invalid_identity_on_a_value() {
    let output = rejected_body("mut x = 1 is 2;");
    assert!(output.contains("no identity to compare"), "{output}");
}

#[test]
fn valid_identity_between_two_nullable_receivers() {
    // Unlike `==` (`reject_nullable_comparison`), `Is`'s own arm of
    // `check_binary` has no such rejection: identity is the one place
    // comparing against a possibly-absent reference is allowed
    // (`ZIRK_LANGUAGE_SPEC.md` section 4).
    accepted(
        "class U { construct() { } }
         fn main(): Void { mut a: U? = U(); mut b: U? = a; stdout.println(a is b); }",
    );
}

#[test]
fn valid_identity_between_a_bare_and_a_nullable_receiver() {
    // `expect_same`'s own `unify` widens `U` to `U?` the same way an
    // ordinary assignment would — found via a lowering bug where the
    // checker accepted this and the IR did not agree with itself about
    // which of the two types the comparison ran at.
    accepted(
        "class U { construct() { } }
         fn main(): Void { mut a: U = U(); mut b: U? = a; stdout.println(a is b); }",
    );
}

#[test]
fn invalid_reaching_a_member_through_a_nullable_receiver() {
    // Calling through a value that may be absent is the same mistake as
    // reading through one.
    for body in [
        "mut u: U? = null;\nstdout.println(u.name);",
        "mut u: U? = null;\nstdout.println(u.describe());",
    ] {
        let output = rejected(&format!(
            "class U {{ name: String; construct() {{ this.name = \"a\"; }} \
             fn describe(): String {{ return this.name; }} }}\n\
             fn main(): Void {{ {body} }}"
        ));
        assert!(output.contains("may be absent"), "for `{body}`:\n{output}");
    }
}

#[test]
fn invalid_safe_navigation_on_a_receiver_that_is_never_null() {
    // Found in manual verification after the fase-4 nullability fixes:
    // `?.`'s whole purpose is handling absence, so — the same judgment
    // `check_coalesce` already makes for `??` on a non-nullable left side
    // — applying it to a receiver that is never absent is flagged instead
    // of silently accepted and left to reach the lowering, which assumes
    // every `?.` receiver is nullable.
    for body in [
        "mut u: U = U();\nstdout.println(u?.name);",
        "mut u: U = U();\nstdout.println(u?.describe());",
    ] {
        let output = rejected(&format!(
            "class U {{ name: String; construct() {{ this.name = \"a\"; }} \
             fn describe(): String {{ return this.name; }} }}\n\
             fn main(): Void {{ {body} }}"
        ));
        assert!(
            output.contains(codes::REDUNDANT_OPERATOR.as_str()),
            "for `{body}`:\n{output}"
        );
    }
}

#[test]
fn valid_safe_call_on_a_method_that_returns_void() {
    // `Void?` is rejected as a type (see the test below), so the checker
    // types `objeto?.algo()` as plain `Void` when `algo` returns `Void` —
    // there is nothing to widen to a nullable form of "no value" — rather
    // than panicking the lowering trying to build one
    // (`docs/decisions/ADR-003-investigacion-fase-4.md`, section "Extensión:
    // criterio 3...").
    accepted(
        "class C { construct() { } fn bump(): Void { } }
         fn main(): Void { mut c: C? = C(); c?.bump(); }",
    );
}

#[test]
fn valid_a_nullable_contract_is_a_type() {
    // It reaches lowering as an ordinary nullable, which is what the uniform
    // representation buys.
    accepted(
        "interface D { fn describe(): String; }
         class R implements D { construct() { } fn describe(): String { return \"r\"; } }
         fn main(): Void { mut d: D? = R(); mut e: D? = null; }",
    );
}

// --- Genéricos ---------------------------------------------------------------

#[test]
fn valid_generic_class_body_type_checks_against_its_own_type_parameter() {
    // `T` resolves the same way everywhere the class names it, so assigning a
    // `T` to a `T` field and returning a `T` from a method that declares `T`
    // both type-check. A class this simple — every use of `T` direct, no
    // `extends`/`implements` — also fully lowers now (roadmap task 11.1), so
    // this is accepted outright rather than merely free of these two codes.
    accepted(
        "class Box<T> {
             value: T;
             construct(value: T) { this.value = value; }
             fn get(): T { return this.value; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_generic_function_is_not_lowered_yet() {
    // Task 11.1 specializes a generic *class*; a generic function's own type
    // parameter is a separate mechanism (`enter_type_params`, shared with
    // methods) that this pass does not build a specialization for.
    let output = rejected("fn identity<T>(value: T): T { return value; }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_generic_class_with_extends_is_not_lowered_yet() {
    // Task 11.1's specialization pass does not build a specialized copy's
    // own dispatch table, so a generic class that `extends` stays gated.
    let output = rejected(
        "class Base { }
         class Box<T> extends Base { value: T; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_generic_class_with_nested_type_argument_is_not_lowered_yet() {
    // `T` nested inside another generic type (here, itself) rather than
    // named directly is the one shape task 11.1's substitution does not
    // recurse into (`Self::type_references_any_param`).
    let output = rejected(
        "class Box<T> {
             value: T;
             next: Box<T>?;
             construct(value: T) { this.value = value; this.next = null; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_returning_a_concrete_type_where_the_type_parameter_is_declared() {
    // `T` is opaque: nothing proves it is `Int32`, so a literal does not fit.
    let output = rejected(
        "class Box<T> {
             fn wrong(): T { return 5; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_type_parameter_is_out_of_scope_outside_its_declaration() {
    let output = rejected(
        "class Box<T> { value: T; }
         class Other { value: T; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()), "{output}");
}

#[test]
fn invalid_declared_variance_is_not_verified_yet() {
    // The grammar accepts `in`/`out` (`ZIRK_LANGUAGE_SPEC.md` section 7), but
    // task 7.5 of the generics slice asks for a diagnostic of its own rather
    // than silently treating it as invariant.
    for source in [
        "class Box<out T> { value: T; }\nfn main(): Void { }",
        "class Sink<in T> { fn take(value: T): Void { } }\nfn main(): Void { }",
    ] {
        let output = rejected(source);
        assert!(output.contains(codes::PENDING_FEATURE.as_str()), "{output}");
        assert!(output.contains("variance"), "{output}");
    }
}

#[test]
fn valid_method_call_through_a_type_parameters_contract_constraint() {
    // `value.greet()` resolves through `Greeter`, the one constraint `T`
    // declares — task 7.4 of the generics slice.
    let output = rejected(
        "interface Greeter { fn greet(): String; }
         fn show<T from Greeter>(value: T): String { return value.greet(); }
         fn main(): Void { }",
    );
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_field_access_through_a_type_parameters_class_constraint() {
    let output = rejected(
        "class Named { name: String; construct(name: String) { this.name = name; } }
         fn show<T from Named>(value: T): String { return value.name; }
         fn main(): Void { }",
    );
    assert!(!output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_method_the_constraint_does_not_promise() {
    let output = rejected(
        "interface Greeter { fn greet(): String; }
         fn show<T from Greeter>(value: T): Void { mut r = value.somethingElse(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("Greeter"), "{output}");
}

#[test]
fn invalid_member_access_on_an_unconstrained_type_parameter() {
    let output = rejected(
        "fn show<T>(value: T): Void { mut r = value.anything(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("no `from` constraint"), "{output}");
}

#[test]
fn valid_call_infers_the_type_parameter_from_its_argument() {
    // `identity(5)` infers `T = Int32` (roadmap task 7.6), so it type-checks
    // as `Int32` with no argument-type or inference error — the class-level
    // `NOT_LOWERED` from 7.1 is the only thing keeping this from compiling.
    let output = rejected(
        "fn identity<T>(value: T): T { return value; }
         fn main(): Void { mut r: Int32 = identity(5); }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_call_cannot_infer_a_type_parameter_used_only_in_the_return_type() {
    // Inference only looks at arguments today: nothing here determines `T`.
    let output = rejected(
        "fn make<T>(): T { return 5; }
         fn main(): Void { mut r = make(); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("cannot infer"), "{output}");
}

#[test]
fn invalid_call_infers_conflicting_types_for_the_same_parameter() {
    let output = rejected(
        "fn pair<T>(a: T, b: T): Void { }
         fn main(): Void { pair(5, \"x\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("cannot infer"), "{output}");
}

#[test]
fn invalid_call_infers_a_type_that_does_not_satisfy_the_constraint() {
    let output = rejected(
        "interface Serializable { fn serialize(): String; }
         fn store<T from Serializable>(value: T): Void { }
         fn main(): Void { store(5); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("Serializable"), "{output}");
}

#[test]
fn valid_call_infers_a_type_that_satisfies_the_constraint() {
    let output = rejected(
        "interface Serializable { fn serialize(): String; }
         class Doc implements Serializable { construct() { } fn serialize(): String { return \"\"; } }
         fn store<T from Serializable>(value: T): Void { }
         fn main(): Void { store(Doc()); }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_generic_type_arguments_resolve_and_check_arity() {
    // `Box<Int32>` itself resolves (roadmap task 7.3), and `Box<T>` is
    // simple enough to fully specialize (11.1), so this compiles clean.
    accepted(
        "class Box<T> { value: T; }
         class Holder { value: Box<Int32>; }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_generic_type_argument_count() {
    let output = rejected(
        "class Pair<A, B> { a: A; b: B; }
         class Holder { value: Pair<Int32>; }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_generic_type_argument_missing_its_constraint() {
    let output = rejected(
        "interface Serializable { fn serialize(): String; }
         class Box<T from Serializable> { value: T; }
         class Holder { value: Box<Int32>; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("Serializable"), "{output}");
}

#[test]
fn valid_generic_type_argument_satisfying_its_class_constraint() {
    accepted(
        "class Animal { }
         class Dog extends Animal { }
         class Cage<T from Animal> { value: T; }
         class Holder { value: Cage<Dog>; }
         fn main(): Void { }",
    );
}

#[test]
fn valid_two_instantiations_of_the_same_class_are_the_same_type() {
    // Interned like a function type: `Box<Int32>` written twice is one id.
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Box<T> { value: T; }
         class Holder { a: Box<Int32>; b: Box<Int32>; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(
        checked.generic_instances.len(),
        1,
        "`Box<Int32>` written twice should intern to one instantiation"
    );
}

// --- Enums algebraicos y mapping ----------------------------------------------

#[test]
fn valid_traditional_enum_is_unaffected() {
    accepted("enum Direction { North, South, East, West }\nfn main(): Void { }");
}

#[test]
fn valid_enum_variant_with_a_string_or_numeric_mapping() {
    accepted("enum Status { Ok -> 200, NotFound -> 404 }\nfn main(): Void { }");
    accepted("enum Role { Admin -> \"admin\", Guest -> \"guest\" }\nfn main(): Void { }");
}

#[test]
fn invalid_enum_variant_mapping_to_an_incompatible_type() {
    let output = rejected("enum Status { Ok -> true }\nfn main(): Void { }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_algebraic_variant_constructs_and_destructures() {
    // Associated data fully lowers now — discriminant plus a flattened
    // payload (roadmap task 11.3/11.4) — so this compiles clean.
    accepted(
        "enum Shape { Circle(radius: Int32), Point }
         fn area(s: Shape): Int32 {
             return match s {
                 Shape.Circle(radius) => radius,
                 Shape.Point => 0,
             };
         }
         fn main(): Void { mut s = Shape.Circle(radius: 3); stdout.println(area(s)); }",
    );
}

#[test]
fn valid_algebraic_variant_matches_named_arguments() {
    accepted(
        "enum Shape { Rectangle(w: Int32, h: Int32) }
         fn main(): Void { mut s = Shape.Rectangle(h: 2, w: 4); }",
    );
}

#[test]
fn invalid_generic_enum_with_data_is_not_lowered_yet() {
    // A non-generic algebraic enum fully lowers (task 11.3); combining that
    // with per-instantiation specialization (11.1) is out of scope.
    let output = rejected(
        "enum Box<T> { Full(value: T), Empty }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_literal_pattern_destructuring_a_variant_is_not_lowered_yet() {
    // A binding or a wildcard destructures a variant's field for real
    // (task 11.4); a literal sub-pattern would need combined-condition
    // compilation this pass does not build.
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void {
             mut s = Shape.Circle(radius: 3);
             match s { Shape.Circle(3) => { } _ => { } }
         }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

// --- Records y value classes --------------------------------------------------

#[test]
fn valid_record_construction_and_field_access() {
    // A non-generic record fully lowers now (roadmap task 11.5).
    accepted(
        "record Point { x: Int32; y: Int32; }
         fn main(): Void { mut p = Point(x: 1, y: 2); stdout.println(p.x); }",
    );
}

#[test]
fn valid_record_omitted_field_takes_its_type_default() {
    accepted(
        "record Settings { count: Int32; flag: Boolean; }
         fn main(): Void { mut s = Settings(); }",
    );
}

#[test]
fn invalid_record_field_without_a_default_is_required() {
    let output = rejected(
        "class Engine { }
         record Car { engine: Engine; }
         fn main(): Void { mut c = Car(); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_record_construction_with_positional_arguments() {
    let output = rejected(
        "record Point { x: Int32; y: Int32; }
         fn main(): Void { mut p = Point(1, 2); }",
    );
    assert!(
        output.contains(codes::UNKNOWN_ARGUMENT_NAME.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_record_extends_a_class() {
    let output = rejected(
        "class Base { }
         record Point extends Base { x: Int32; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_record_declares_a_custom_construct() {
    let output = rejected(
        "record Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_record_field_explicitly_marked_mut() {
    let output = rejected(
        "record Point { mut x: Int32; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_mutating_a_record_field_from_a_method() {
    let output = rejected(
        "record Point {
             x: Int32;
             fn reset(): Void { this.x = 0; }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::ASSIGN_TO_IMMUTABLE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_record_equality_is_derived_without_a_reserved_method() {
    let output = rejected(
        "record Point { x: Int32; y: Int32; }
         fn main(): Void { mut a = Point(x: 1, y: 2); mut b = Point(x: 1, y: 2); stdout.println(a == b); }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_identity_comparison_on_a_record() {
    let output = rejected(
        "record Point { x: Int32; }
         fn main(): Void { mut a = Point(x: 1); mut b = Point(x: 1); stdout.println(a is b); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("no identity"), "{output}");
}

#[test]
fn invalid_generic_record_is_not_lowered_yet() {
    // A non-generic record/value class fully lowers (task 11.5); combining
    // that with per-instantiation specialization (11.1) is out of scope.
    // `value class` sugar has no syntax for `<T>` at all, so only a
    // `record` can even be written generic.
    let output = rejected("record Box<T> { value: T; }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn valid_record_method_called_directly() {
    // A record's own method dispatches statically (design.md's open
    // question on virtual methods) — nothing gates a plain call the way
    // implementing a contract does below.
    accepted(
        "record Point {
             x: Int32;
             fn double(): Int32 { return this.x * 2; }
         }
         fn main(): Void { mut p = Point(x: 3); stdout.println(p.double()); }",
    );
}

#[test]
fn invalid_record_implements_a_contract_is_not_lowered_yet() {
    // Conformance is checked (a missing or mismatched `describe` would
    // still be caught), but a record has no descriptor to carry the
    // contract's own table, so reaching one through the contract type is
    // not compilable yet — only the type declaration itself is gated.
    let output = rejected(
        "interface Describable { fn describe(): String; }
         record Point implements Describable {
             x: Int32;
             fn describe(): String { return \"point\"; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn valid_value_class_construction() {
    accepted("value class UserId(value: Int32);\nfn main(): Void { mut u = UserId(value: 5); }");
}

#[test]
fn invalid_constructing_a_variant_with_the_wrong_argument_type() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void { mut s = Shape.Circle(radius: \"x\"); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_constructing_a_variant_with_the_wrong_argument_count() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void { mut s = Shape.Circle(1, 2); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_constructing_a_bare_variant_that_carries_data() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn main(): Void { mut s = Shape.Circle; }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_constructing_with_data_a_variant_that_carries_none() {
    let output = rejected(
        "enum Shape { Point }
         fn main(): Void { mut s = Shape.Point(1); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_pattern_omits_the_destructuring_a_variant_needs() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32) }
         fn area(s: Shape): Int32 {
             match s { Shape.Circle => 0 }
         }
         fn main(): Void { mut s = Shape.Circle(radius: 1); stdout.println(area(s)); }",
    );
    assert!(
        output.contains(codes::WRONG_ARGUMENT_COUNT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_over_an_algebraic_enum_is_not_exhaustive() {
    let output = rejected(
        "enum Shape { Circle(radius: Int32), Point }
         fn area(s: Shape): Int32 {
             match s { Shape.Circle(radius) => radius }
         }
         fn main(): Void { mut s = Shape.Point; stdout.println(area(s)); }",
    );
    assert!(
        output.contains(codes::NON_EXHAUSTIVE_MATCH.as_str()),
        "{output}"
    );
}

#[test]
fn valid_match_null_arm_narrows_the_binding_arm() {
    accepted(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node? = Node(1);
             match n {
                 null => { }
                 binding => { stdout.println(binding.value); }
             }
         }",
    );
}

#[test]
fn valid_match_null_arm_narrows_the_variant_pattern_arm() {
    // Exhaustiveness over a nullable enum still needs a catch-all today
    // (a pre-existing rule this change does not touch) — the variant arm's
    // own destructuring is what is under test here, not exhaustiveness.
    accepted(
        "enum Status { Ok(value: Int32), Err(message: String) }
         fn main(): Void {
             mut s: Status? = Status.Ok(1);
             match s {
                 null => { }
                 Status.Ok(value) => { stdout.println(value); }
                 _ => { }
             }
         }",
    );
}

#[test]
fn invalid_match_binding_still_nullable_without_a_null_arm() {
    let output = rejected(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node? = Node(1);
             match n {
                 binding => { stdout.println(binding.value); }
             }
         }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_match_narrowing_does_not_apply_inside_the_null_arm_itself() {
    let output = rejected(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node? = Node(1);
             match n {
                 null => { stdout.println(n.value); }
                 binding => { stdout.println(binding.value); }
             }
         }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_member_access_still_rejected_on_a_non_nullable_match_binding() {
    // Narrowing must not paper over a genuine mismatch: a `Node` scrutinee
    // (never nullable) that is matched against a member the class does not
    // have is still an error, `null`-arm narrowing or not.
    let output = rejected(
        "class Node { mut value: Int32; construct(value: Int32) { this.value = value; } }
         fn main(): Void {
             mut n: Node = Node(1);
             match n {
                 binding => { stdout.println(binding.missing); }
             }
         }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
}

// --- Alias de tipo -------------------------------------------------------------

#[test]
fn valid_type_alias_is_transparent_with_its_target() {
    let output = rejected("type UserId = Int32;\nfn main(): Void { mut id: UserId = 5; }");
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_type_alias_to_a_declared_class() {
    let output = rejected(
        "class User { construct() { } }
         type Account = User;
         fn main(): Void { mut a: Account = User(); }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_type_alias_is_not_lowered_yet() {
    let output = rejected("type UserId = Int32;\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn invalid_type_alias_mismatched_target() {
    let output = rejected("type UserId = Int32;\nfn main(): Void { mut id: UserId = \"x\"; }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_duplicate_type_alias() {
    let output = rejected(
        "type A = Int32;
         type A = String;
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_type_alias_cycle() {
    let output = rejected(
        "type A = B;
         type B = A;
         fn main(): Void { mut x: A = 1; }",
    );
    assert!(
        output.contains(codes::DUPLICATE_DECLARATION.as_str()),
        "{output}"
    );
    assert!(output.contains("itself"), "{output}");
}

#[test]
fn valid_type_alias_chain() {
    let output = rejected(
        "type A = Int32;
         type B = A;
         fn main(): Void { mut x: B = 5; }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- Uniones ---------------------------------------------------------------

#[test]
fn valid_union_type_accepts_either_alternative() {
    let output = rejected(
        "fn f(x: String | Int32): Void { }
         fn main(): Void { f(\"a\"); f(1); }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_union_type_rejects_a_third_type() {
    let output = rejected(
        "fn f(x: String | Int32): Void { }
         fn main(): Void { f(true); }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_union_order_is_irrelevant() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder { a: String | Int32; b: Int32 | String; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(
        checked.unions.len(),
        1,
        "`String | Int32` and `Int32 | String` should intern to one union"
    );
}

#[test]
fn valid_union_duplicates_collapse() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder { a: String | String | Int32; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(
        checked.unions[0].len(),
        2,
        "a repeated alternative is one member"
    );
}

#[test]
fn valid_union_with_null_is_a_plain_nullable_type() {
    // `String | Null` degrades to a plain nullable `String`, not a `Union`.
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Holder { a: String | Null; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert_eq!(checked.unions.len(), 0);
}

#[test]
fn valid_union_subsumes_a_subclass_alternative() {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new(
        "test.zrk",
        "class Animal { }
         class Dog extends Animal { }
         class Holder { a: Animal | Dog; }
         fn main(): Void { }",
    ));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    // `Dog` is subsumed by `Animal`, so nothing needed a `Union` at all.
    assert_eq!(checked.unions.len(), 0);
}

#[test]
fn invalid_member_access_on_an_unnarrowed_union() {
    let output = rejected(
        "class A { fn hello(): Void { } }
         class B { }
         fn f(x: A | B): Void { x.hello(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::UNKNOWN_MEMBER.as_str()), "{output}");
    assert!(output.contains("narrow"), "{output}");
}

#[test]
fn invalid_union_type_is_not_lowered_yet() {
    let output = rejected("fn f(x: String | Int32): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

// --- Casts -----------------------------------------------------------------

#[test]
fn valid_cast_between_a_class_and_its_base() {
    // A class-to-class cast is checked at runtime now (roadmap task 11.6),
    // so this compiles clean rather than merely avoiding `TYPE_MISMATCH`.
    accepted(
        "class Animal { }
         class Dog extends Animal { construct() { } }
         fn main(): Void { mut d = Dog(); mut a = d as Animal; mut b = <Dog>a; }",
    );
}

#[test]
fn valid_cast_between_a_class_and_a_contract() {
    let output = rejected(
        "interface Shape { fn area(): Int32; }
         class Circle implements Shape { construct() { } fn area(): Int32 { return 1; } }
         fn main(): Void { mut c = Circle(); mut s = c as Shape; mut back = <Circle>s; }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_cast_from_a_union_member() {
    let output = rejected(
        "fn f(x: String | Int32): Void { mut s = x as String; }
         fn main(): Void { }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_identity_cast() {
    accepted("fn main(): Void { mut x = 1 as Int32; }");
}

#[test]
fn invalid_cast_between_unrelated_types() {
    let output = rejected("fn main(): Void { mut x = \"a\" as Int32; }");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_cast_between_unrelated_classes() {
    let output = rejected(
        "class A { construct() { } }
         class B { construct() { } }
         fn main(): Void { mut a = A(); mut b = a as B; }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_nullable_cast_is_not_lowered_yet() {
    // A class-to-class cast lowers now (task 11.6), but only once nullability
    // is out of the way: deciding what a null value means for the check is a
    // separate question this pass does not answer yet.
    let output = rejected(
        "class Animal { }
         class Dog extends Animal { construct() { } }
         fn main(): Void { mut a: Animal? = Dog(); mut d = a as Dog?; }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

#[test]
fn valid_cast_from_a_type_parameter_to_its_own_constraint() {
    // `T from Serializable` may always be widened to `Serializable`: that is
    // exactly what the constraint promises, not a leap of faith.
    let output = rejected(
        "interface Serializable { fn serialize(): String; }
         fn show<T from Serializable>(x: T): Void { mut y = x as Serializable; }
         fn main(): Void { }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_cast_from_a_type_parameter_to_an_unrelated_type() {
    let output = rejected(
        "interface Serializable { fn serialize(): String; }
         fn show<T from Serializable>(x: T): Void { mut y = x as String; }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

// --- `?.` --------------------------------------------------------------------

#[test]
fn valid_safe_field_access_types_as_nullable() {
    // The type rules are real (D7): the member's own type is `String`, but
    // reading it through `?.` makes the result `String?`. Field access through
    // `?.` is fully lowered (task 10.8), so this is accepted outright.
    accepted(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { mut u: User? = null; mut n: String? = u?.name; }",
    );
}

#[test]
fn valid_safe_method_call_types_as_nullable() {
    accepted(
        "class User { fn greet(): String { return \"hi\"; } construct() { } }
         fn main(): Void { mut u: User? = null; mut g: String? = u?.greet(); }",
    );
}

#[test]
fn invalid_plain_access_through_a_nullable_receiver_still_rejected() {
    // `?.` is what makes this legal; plain `.` still is not.
    let output = rejected(
        "class User { name: String; construct(name: String) { this.name = name; } }
         fn main(): Void { mut u: User? = null; mut n = u.name; }",
    );
    assert!(output.contains("may be absent"), "{output}");
}

#[test]
fn valid_safe_method_call_lowers() {
    // Both halves of task 10.8 lower now: a field through `?.` and a method
    // call through it, dispatched under the same absent/present split.
    accepted(
        "class User { fn greet(): String { return \"hi\"; } construct() { } }
         fn main(): Void { mut u: User? = null; mut g = u?.greet(); }",
    );
}

#[test]
fn invalid_safe_method_call_through_generic_param_is_not_lowered() {
    // Through a generic parameter's constraint there is no concrete method
    // body to call yet, so that combination stays gated.
    let output = rejected(
        "trait Greeter { fn greet(): String; }
         class Wrap<T from Greeter> { value: T?; construct(value: T?) { this.value = value; } fn hello(): String? { return this.value?.greet(); } }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

// --- Sin shadowing ordinario (D10) --------------------------------------------

#[test]
fn invalid_lambda_parameter_shadows_a_captured_variable() {
    let output = rejected(
        "fn main(): Void {
             inmut outer = 5;
             inmut g = (outer: Int32): Int32 => outer * 2;
             stdout.println(g(3));
         }",
    );
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

#[test]
fn valid_lambda_parameter_with_a_distinct_name_still_captures() {
    accepted(
        "fn main(): Void {
             inmut outer = 5;
             inmut g = (factor: Int32): Int32 => outer * factor;
             stdout.println(g(3));
         }",
    );
}

#[test]
fn valid_lambda_parameter_matches_a_field_name() {
    // A field is never a bare name `Scopes` resolves — `this.name` is how
    // one is always read — so there is nothing for a same-named parameter
    // to shadow.
    accepted(
        "class Multiplier {
             factor: Int32;
             construct(factor: Int32) { this.factor = factor; }
             fn scale(factor: Int32): Int32 { return factor * this.factor; }
         }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_for_in_binding_shadows_an_outer_variable() {
    let output = rejected(
        "fn main(): Void {
             inmut i = 5;
             for i in 0..3 { stdout.println(i); }
         }",
    );
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_binding_shadows_an_outer_variable() {
    let output = rejected(
        "fn main(): Void {
             inmut n = 5;
             mut x: Int32? = 1;
             match x { n => { stdout.println(n); } null => { } }
         }",
    );
    assert!(
        output.contains(codes::ORDINARY_SHADOWING.as_str()),
        "{output}"
    );
}

// --- Closures no anotables ni escapables (D9) ---------------------------------

#[test]
fn valid_closure_stored_in_an_inferred_local_and_called() {
    accepted(
        "fn main(): Void { mut add = (a: Int32, b: Int32): Int32 => a + b; stdout.println(add(1, 2)); }",
    );
}

#[test]
fn valid_closure_returned_from_a_function() {
    // `Fn(...) => R` is a real, writable return type (roadmap Phase 4d): a
    // capture-less closure literal — uniformly represented the same way a
    // named function is (design D12) — satisfies it like any other
    // structurally compatible value. `invalid_closure_returned_from_a_function`
    // used to stand in for this with an unrelated class purely because D9
    // made the annotation itself impossible to write at all.
    accepted(
        "fn make(): Fn() => Void { return (): Void => { }; }
         fn main(): Void { }",
    );
}

#[test]
fn invalid_class_returned_where_a_closure_is_expected() {
    // The class-vs-closure mismatch the old test actually exercised is still
    // rejected — replacing the class with a real class value shows the
    // mismatch is about the *types*, not about `Fn(...) => R` being
    // unwritable.
    let output = rejected(
        "class NotAClosure { }
         fn make(): Fn() => Void { return NotAClosure(); }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn valid_generic_inference_from_a_closure_argument() {
    // A closure argument infers a generic parameter `T` the same way any
    // other value does (roadmap Phase 4d): the checker no longer refuses to
    // name `Base::Function` as `T`'s solution — that refusal
    // (`invalid_generic_inference_from_a_closure_argument`) existed only
    // because D9 made a closure's type unwritable anywhere, generic
    // arguments included. `store<T>` itself is still `NOT_LOWERED` (generic
    // functions are not code-generated yet, roadmap task 7.6) — unrelated
    // to closures, and the one diagnostic this now reports.
    let output = rejected(
        "fn store<T>(value: T): Void { }
         fn main(): Void { store((): Void => { }); }",
    );
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
    assert!(
        !output.contains("closure"),
        "a closure argument should no longer be singled out:\n{output}"
    );
}

#[test]
fn invalid_two_differently_captured_closures_in_one_return_type() {
    // Design D14: a `Fn(...) => R` return type only accepts a *single*
    // capturing closure literal — two different `return`s, each with its
    // own captures, would need the captures boxed behind a uniform
    // representation this pass does not build (design D13). Rejected with a
    // dedicated diagnostic, not a generic `TYPE_MISMATCH`.
    let output = rejected(
        "fn make(pick: Boolean): Fn() => Int32 {
             mut a = 1;
             mut b = 2;
             if pick {
                 return (): Int32 => a;
             }
             return (): Int32 => b;
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::AMBIGUOUS_CAPTURING_CALLABLE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_recursive_lambda_binding_used_as_a_value() {
    // A recursive lambda's own binding (`ZIRK_LANGUAGE_SPEC.md` section 6)
    // refers to itself only through a direct call inside its own body:
    // `zirk-ir` rewrites that one shape into an ordinary recursive call,
    // since no closure value for it exists yet at the point the literal is
    // still being built. Reading it into a variable — or any other use
    // besides a direct call — has no value to give and must be rejected
    // here, not reach a panic during lowering.
    let output = rejected(
        "fn main(): Void {
             mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => {
                 mut self_ref = fact;
                 return n <= 1 ? 1 : n * fact(n - 1);
             };
             stdout.println(fact(5));
         }",
    );
    assert!(
        output.contains(codes::RECURSIVE_BINDING_NOT_A_VALUE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_recursive_lambda_calling_itself_only_directly() {
    // The one shape a recursive lambda's own binding may appear in: the
    // direct callee of a call, anywhere inside its own body (including
    // nested inside a ternary/if), never through a nested lambda.
    accepted(
        "fn main(): Void {
             mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => n <= 1 ? 1 : n * fact(n - 1);
             stdout.println(fact(5));
         }",
    );
}

#[test]
fn invalid_recursive_lambda_binding_used_from_a_nested_lambda() {
    // Even in call position, a reference reached from inside a lambda
    // nested within the recursive one is an ordinary (but unimplemented)
    // capture-of-a-capture, not a self-call — `zirk-ir`'s rewrite only
    // covers the recursive lambda's own immediate body.
    let output = rejected(
        "fn main(): Void {
             mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => {
                 mut helper = (): Int32 => fact(1);
                 return n <= 1 ? 1 : n * fact(n - 1);
             };
             stdout.println(fact(5));
         }",
    );
    assert!(
        output.contains(codes::RECURSIVE_BINDING_NOT_A_VALUE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_generic_class_construction_and_member_access() {
    // A generic class's construction, annotation and member access all type
    // through a real instantiation (`Base::Instance`), not the bare class:
    // `check_construction` used to build the constructor's `Signature` with
    // no type parameters at all, so `Box(5)` never inferred `T` and this
    // failed with `TYPE_MISMATCH`. `Box<T>` is also simple enough to fully
    // specialize (roadmap task 11.1), so the whole program compiles clean.
    accepted(
        "class Box<T> {
             value: T;
             construct(value: T) { this.value = value; }
             fn get(): T { return this.value; }
         }
         fn main(): Void {
             mut b: Box<Int32> = Box(5);
             stdout.println(b.get());
         }",
    );
}

// --- Abstract classes ----------------------------------------------------------

const SHAPE: &str = "abstract class Shape {
    name: String;
    abstract fn area(): Int32;
}";

#[test]
fn valid_class_adopts_an_abstract_class() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             override fn area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        !output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
    assert!(
        !output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_abstract_class_missing_attribute() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             construct() {{ }}
             override fn area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_abstract_class_missing_method() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_abstract_class_method_without_override() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             fn area(): Int32 {{ return 3; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::MISSING_OVERRIDE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_abstract_class_method_with_mismatched_signature() {
    let output = rejected(&format!(
        "{SHAPE}
         class Circle implements Shape {{
             name: String;
             construct(name: String) {{ this.name = name; }}
             override fn area(): String {{ return \"x\"; }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_abstract_class_extends_a_class() {
    let output = rejected(
        "class Base { }
         abstract class Shape extends Base { }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_abstract_class_declares_a_construct() {
    let output = rejected(
        "abstract class Shape { construct() { } }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_abstract_class_is_not_lowered_yet() {
    let output = rejected(&format!("{SHAPE}\nfn main(): Void {{ }}"));
    assert!(output.contains(codes::NOT_LOWERED.as_str()), "{output}");
}

// --- `throw`/`try`/`catch`/`finally` (roadmap Phase 4b) ----------------------

const BOOM_ERROR: &str = "class BoomError implements RuntimeError {
    construct() { }
    override fn message(): String { return \"boom\"; }
    override fn code(): String { return \"BOOM\"; }
    override fn cause(): Error? { return null; }
    override fn stack_trace(): StackTrace { return StackTrace(); }
}";

#[test]
fn valid_throw_and_catch_by_concrete_type() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
}

#[test]
fn valid_catch_by_throwable_supertype() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch Throwable(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
}

#[test]
fn valid_rethrow_inside_catch() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn wrapper(): Void throws BoomError {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
                 throw;
             }}
         }}
         fn main(): Void {{
             try {{
                 wrapper();
             }} catch Throwable(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
}

#[test]
fn valid_throws_declaration_covers_propagation() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn explode(): Void throws BoomError {{
             throw BoomError();
         }}
         fn main(): Void throws BoomError {{
             explode();
         }}"
    ));
}

#[test]
fn valid_finally_always_runs() {
    accepted(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
                 stdout.println(e.message());
             }} finally {{
                 stdout.println(\"cleanup\");
             }}
         }}"
    ));
}

#[test]
fn invalid_throw_of_non_throwable_type() {
    let output = rejected_body("throw \"not throwable\";");
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_uncaught_throw_must_be_declared() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             throw BoomError();
         }}"
    ));
    assert!(output.contains(codes::UNCAUGHT_THROW.as_str()), "{output}");
}

#[test]
fn invalid_uncovered_throw_from_a_call_must_be_declared() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn explode(): Void throws BoomError {{
             throw BoomError();
         }}
         fn main(): Void {{
             explode();
         }}"
    ));
    assert!(output.contains(codes::UNCAUGHT_THROW.as_str()), "{output}");
}

#[test]
fn invalid_unreachable_catch_after_a_broader_one() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn main(): Void {{
             try {{
                 throw BoomError();
             }} catch Throwable(e) {{
                 stdout.println(e.message());
             }} catch BoomError(e) {{
                 stdout.println(e.message());
             }}
         }}"
    ));
    assert!(
        output.contains(codes::UNREACHABLE_CATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_bare_rethrow_outside_catch() {
    let output = rejected_body("throw;");
    assert!(
        output.contains(codes::RETHROW_OUTSIDE_CATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_return_directly_inside_finally() {
    let output = rejected(&format!(
        "{BOOM_ERROR}
         fn f(): Void {{
             try {{
                 throw BoomError();
             }} catch BoomError(e) {{
             }} finally {{
                 return;
             }}
         }}
         fn main(): Void {{ }}"
    ));
    assert!(
        output.contains(codes::FINALLY_REPLACES_OUTCOME.as_str()),
        "{output}"
    );
}

// --- Native `RuntimeError` subclasses (`fase-4d-runtimeerror`) --------------

#[test]
fn valid_catch_native_failure_by_its_own_concrete_type() {
    accepted_body(
        "try {
             mut a: Int32 = 1;
             mut b: Int32 = 0;
             mut c: Int32 = a / b;
         } catch DivisionByZeroError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_native_failure_by_runtime_error_supertype() {
    accepted_body(
        "try {
             mut a: Int32 = 1;
             mut b: Int32 = 0;
             mut c: Int32 = a / b;
         } catch RuntimeError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_native_failure_by_throwable_supertype() {
    accepted_body(
        "try {
             mut a: Int32 = 1;
             mut b: Int32 = 0;
             mut c: Int32 = a / b;
         } catch Throwable(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_catch_each_of_the_four_native_failure_classes() {
    // Every one of the four type-checks against its own catch, exactly like
    // a user's own `implements RuntimeError` class does — `DivisionByZeroError`
    // is covered by the tests above already, so this exercises the other
    // three: `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError`.
    accepted_body(
        "try {
             mut x: Int32 = 1;
             mut s: Int32 = 40;
             mut y: Int32 = x << s;
         } catch InvalidShiftError(e) {
             stdout.println(e.message());
         }
         try {
             mut a: String = \"x\";
             mut n: Int32 = -1;
             mut r: String = a * n;
         } catch InvalidRepeatError(e) {
             stdout.println(e.message());
         }
         try {
             mut zero: Float = 0.0;
             mut nan: Float = zero / zero;
         } catch FloatNanError(e) {
             stdout.println(e.message());
         }",
    );
}

#[test]
fn valid_native_failure_is_never_declared_in_a_throws_clause() {
    // A native failure is implicit by design
    // (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3): a function
    // that divides is not required — and has no syntax — to declare
    // `throws DivisionByZeroError`, unlike a `throw` of its own.
    accepted(
        "fn divide(a: Int32, b: Int32): Int32 {
             return a / b;
         }
         fn main(): Void {
             try {
                 mut r: Int32 = divide(1, 0);
             } catch DivisionByZeroError(e) {
                 stdout.println(e.message());
             }
         }",
    );
}

#[test]
fn valid_native_failure_class_is_constructible_like_any_other_runtime_error() {
    // `fase-4d-runtimeerror`'s design (D9) registers the four native failure
    // classes as concrete, not abstract — a program may name and construct
    // one directly, exactly as it could write its own
    // `class Foo implements RuntimeError { construct(reason: String) { ... } }`
    // and call `Foo("reason")`; nothing about being compiler-known makes
    // construction special or forbidden (see `design.md`'s `## Decisions`
    // for why no explicit rejection was added).
    accepted_body(
        "try {
             throw DivisionByZeroError(\"custom\");
         } catch DivisionByZeroError(e) {
             stdout.println(e.message());
         }",
    );
}

// --- `Resource<E>` / `match ... with` (roadmap Phase 4c) --------------------

const OPEN_ERROR: &str = "class OpenError implements Error {
    inmut reason: String;
    construct(reason: String) { this.reason = reason; }
    override fn message(): String { return this.reason; }
    override fn code(): String { return \"OPEN\"; }
    override fn cause(): Error? { return null; }
}";

const FAKE_FILE: &str = "class FakeFile implements Resource<OpenError> {
    inmut name: String;
    mut closed: Boolean;
    construct(name: String) { this.name = name; this.closed = false; }
    fn nothing(): Void { return; }
    override fn close(): Result<Void, OpenError> {
        this.closed = true;
        return Result.Ok(this.nothing());
    }
    override fn is_closed(): Boolean { return this.closed; }
}

fn open(name: String): Result<FakeFile, OpenError> {
    return Result.Ok(FakeFile(name));
}";

#[test]
fn valid_class_implements_resource() {
    accepted(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn main(): Void {{ }}"
    ));
}

#[test]
fn valid_match_with_closes_the_acquired_resource() {
    accepted(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn main(): Void {{
             match open(\"a.txt\") with file {{
                 Result.Ok(file) => {{
                     stdout.println(file.name);
                 }}
                 Result.Error(error) => {{
                     stdout.println(error.message());
                 }}
             }}
         }}"
    ));
}

#[test]
fn valid_match_with_closes_on_early_return() {
    accepted(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn read(): Int32 {{
             match open(\"a.txt\") with file {{
                 Result.Ok(file) => {{
                     return 1;
                 }}
                 Result.Error(error) => {{
                     return -1;
                 }}
             }}
             return 0;
         }}
         fn main(): Void {{ }}"
    ));
}

#[test]
fn invalid_class_does_not_implement_resource() {
    let output = rejected(
        "class OpenError implements Error {
             inmut reason: String;
             construct(reason: String) { this.reason = reason; }
             override fn message(): String { return this.reason; }
             override fn code(): String { return \"OPEN\"; }
             override fn cause(): Error? { return null; }
         }
         class FakeFile implements Resource<OpenError> {
             construct() { }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::MISSING_IMPLEMENTATION.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_with_binding_unused_by_any_arm() {
    let output = rejected(&format!(
        "{OPEN_ERROR}
         {FAKE_FILE}
         fn main(): Void {{
             match open(\"a.txt\") with file {{
                 Result.Ok(other) => {{ }}
                 Result.Error(error) => {{ }}
             }}
         }}"
    ));
    assert!(
        output.contains(codes::INVALID_RESOURCE_MATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_with_scrutinee_not_a_result() {
    let output = rejected(
        "fn main(): Void {
             match 1 with file {
                 _ => { }
             }
         }",
    );
    assert!(
        output.contains(codes::INVALID_RESOURCE_MATCH.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_match_with_binding_does_not_implement_resource() {
    let output = rejected(
        "fn describe(r: Result<Int32, String>): Void {
             match r with file {
                 Result.Ok(file) => { }
                 Result.Error(error) => { }
             }
         }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::INVALID_RESOURCE_MATCH.as_str()),
        "{output}"
    );
}

// --- Phase 4e: unsafe/Pointer<T>/commit/extern ------------------------------

#[test]
fn valid_pointer_from_read_write_inside_unsafe() {
    accepted_body(
        "mut x: Int32 = 1;
         unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             mut v: Int32 = p.read();
             p.write(v + 1);
             mut q: Pointer<Int32> = p.offset(0);
             mut r: Boolean = q.is_null;
         }",
    );
}

#[test]
fn invalid_pointer_read_outside_unsafe() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         mut p: Pointer<Int32> = Pointer.from(x);
         mut v: Int32 = p.read();",
    );
    assert!(
        output.contains(codes::POINTER_OP_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_pointer_from_outside_unsafe() {
    let output = rejected_body("mut x: Int32 = 1; mut p: Pointer<Int32> = Pointer.from(x);");
    assert!(
        output.contains(codes::POINTER_OP_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_pointer_disallowed_element_type() {
    let output = rejected("fn f(p: Pointer<String>): Void { }\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
}

#[test]
fn valid_byte_resolves_to_uint8() {
    accepted_body(
        "mut b: Byte = 1 as Byte;
         mut u: UInt8 = b;",
    );
}

#[test]
fn invalid_commit_outside_unsafe() {
    let output = rejected_body("commit { mut x: Int32 = 1; }");
    assert!(
        output.contains(codes::COMMIT_OUTSIDE_UNSAFE.as_str()),
        "{output}"
    );
}

#[test]
fn valid_commit_inside_unsafe() {
    accepted_body("unsafe { commit { mut x: Int32 = 1; } }");
}

#[test]
fn valid_extern_call_inside_unsafe_and_commit() {
    accepted(
        "extern \"C\" fn abs(n: Int32): Int32;
         fn main(): Void {
             unsafe {
                 commit {
                     mut r: Int32 = abs(-1);
                 }
             }
         }",
    );
}

#[test]
fn invalid_extern_call_missing_commit() {
    let output = rejected(
        "extern \"C\" fn abs(n: Int32): Int32;
         fn main(): Void {
             unsafe {
                 mut r: Int32 = abs(-1);
             }
         }",
    );
    assert!(
        output.contains(codes::EXTERN_CALL_OUTSIDE_UNSAFE_COMMIT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_extern_disallowed_parameter_type() {
    let output = rejected("extern \"C\" fn f(s: String): Void;\nfn main(): Void { }");
    assert!(output.contains(codes::NOT_FFI_SAFE.as_str()), "{output}");
}

#[test]
fn invalid_pointer_escapes_as_return_value() {
    let output = rejected(
        "fn leak(x: Pointer<Int32>): Pointer<Int32> {
             return x;
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_pointer_escapes_into_a_declared_field() {
    let output = rejected(
        "class Holder {
             value: Pointer<Int32>;
             construct(p: Pointer<Int32>) { this.value = p; }
         }
         fn main(): Void { }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

#[test]
fn invalid_pointer_escapes_via_closure_capture() {
    let output = rejected_body(
        "mut x: Int32 = 1;
         unsafe {
             mut p: Pointer<Int32> = Pointer.from(x);
             mut f: Fn() => Void = (): Void => { mut q: Pointer<Int32> = p; };
         }",
    );
    assert!(output.contains(codes::POINTER_ESCAPES.as_str()), "{output}");
}

// --- Phase 4e: Weak<T> (`fase-4e-weak`) -------------------------------------

#[test]
fn valid_weak_of_a_class_referent() {
    accepted(
        "class Marker { construct() { } }
         fn main(): Void {
             mut m: Marker = Marker();
             mut w: Weak<Marker> = Weak.from(m);
         }",
    );
}

#[test]
fn valid_weak_upgrade_and_is_alive() {
    accepted(
        "class Marker { construct() { } }
         fn main(): Void {
             mut m: Marker = Marker();
             mut w: Weak<Marker> = Weak.from(m);
             mut alive: Boolean = w.is_alive;
             mut upgraded: Marker? = w.upgrade();
         }",
    );
}

#[test]
fn invalid_weak_of_a_scalar_referent() {
    let output = rejected("fn f(w: Weak<Int32>): Void { }\nfn main(): Void { }");
    assert!(
        output.contains(codes::WEAK_DISALLOWED_REFERENT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_weak_of_a_record_referent() {
    let output = rejected(
        "record Point { x: Int32; y: Int32; }
         fn f(w: Weak<Point>): Void { }
         fn main(): Void { }",
    );
    assert!(
        output.contains(codes::WEAK_DISALLOWED_REFERENT.as_str()),
        "{output}"
    );
}

#[test]
fn invalid_weak_from_of_a_scalar_value() {
    let output = rejected_body("mut w: Weak<Int32> = Weak.from(1);");
    assert!(
        output.contains(codes::WEAK_DISALLOWED_REFERENT.as_str()),
        "{output}"
    );
}

// --- Phase 4e: Clone (`fase-4e-clone`) --------------------------------------

#[test]
fn valid_class_with_only_scalar_fields_derives_clone() {
    accepted(
        "class Point { mut x: Int32; mut y: Int32; construct(x: Int32, y: Int32) { this.x = x; this.y = y; } }
         fn main(): Void {
             mut p: Point = Point(1, 2);
             mut q: Point = p.clone();
         }",
    );
}

#[test]
fn valid_class_with_a_clone_class_typed_field_derives_clone() {
    accepted(
        "class Leaf { mut value: Int32; construct(value: Int32) { this.value = value; } }
         class Parent { mut leaf: Leaf; construct(leaf: Leaf) { this.leaf = leaf; } }
         fn main(): Void {
             mut leaf: Leaf = Leaf(1);
             mut parent: Parent = Parent(leaf);
             mut clone: Parent = parent.clone();
         }",
    );
}

#[test]
fn invalid_class_with_a_pointer_field_is_not_clone() {
    let output = rejected(
        "class Holder { mut p: Pointer<Int32>; construct(p: Pointer<Int32>) { this.p = p; } }
         fn main(): Void {
             mut x: Int32 = 1;
             unsafe {
                 mut h: Holder = Holder(Pointer.from(x));
                 mut clone: Holder = h.clone();
             }
         }",
    );
    assert!(output.contains(codes::NOT_CLONE.as_str()), "{output}");
    assert!(output.contains("p"), "{output}");
}

#[test]
fn invalid_class_that_implements_resource_is_not_clone() {
    let output = rejected(
        "class OpenError implements Error {
             inmut reason: String;
             construct(reason: String) { this.reason = reason; }
             override fn message(): String { return this.reason; }
             override fn code(): String { return \"OPEN\"; }
             override fn cause(): Error? { return null; }
         }
         class FakeFile implements Resource<OpenError> {
             inmut name: String;
             mut closed: Boolean;
             construct(name: String) { this.name = name; this.closed = false; }
             override fn close(): Result<Void, OpenError> { this.closed = true; return Result.Ok(_void()); }
             override fn is_closed(): Boolean { return this.closed; }
             fn _void(): Void { return; }
         }
         fn main(): Void {
             mut f: FakeFile = FakeFile(\"a.txt\");
             mut clone: FakeFile = f.clone();
         }",
    );
    assert!(output.contains(codes::NOT_CLONE.as_str()), "{output}");
}

#[test]
fn invalid_class_with_a_non_clone_nested_field_names_the_transitive_path() {
    let output = rejected(
        "class Inner { mut p: Pointer<Int32>; construct(p: Pointer<Int32>) { this.p = p; } }
         class Outer { mut inner: Inner; construct(inner: Inner) { this.inner = inner; } }
         fn main(): Void {
             mut x: Int32 = 1;
             unsafe {
                 mut inner: Inner = Inner(Pointer.from(x));
                 mut outer: Outer = Outer(inner);
                 mut clone: Outer = outer.clone();
             }
         }",
    );
    assert!(output.contains(codes::NOT_CLONE.as_str()), "{output}");
    assert!(output.contains("inner"), "{output}");
    assert!(output.contains("p"), "{output}");
}

// A generic function's own `<T from ...>` unconditionally reports
// `NOT_LOWERED` today (`Checker::enter_type_params`'s own doc comment: "the
// grammar and the type rules for a generic body exist... but lowering does
// not yet"), independent of Clone — the same reason
// `valid_call_infers_a_type_that_satisfies_the_constraint` above already
// uses `rejected()` rather than `accepted()` for an ordinary `from`
// constraint. These two mirror that exact precedent: constraint
// satisfaction is checked by asserting `TYPE_MISMATCH`'s absence/presence,
// not by asserting zero errors overall.

#[test]
fn valid_generic_bound_t_from_clone_is_accepted_for_a_clone_type() {
    let output = rejected(
        "class Marker { mut id: Int32; construct(id: Int32) { this.id = id; } }
         fn dup<T from Clone>(value: T): Void { }
         fn main(): Void {
             mut m: Marker = Marker(1);
             dup(m);
         }",
    );
    assert!(!output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
}

#[test]
fn invalid_generic_bound_t_from_clone_is_rejected_for_a_non_clone_type() {
    let output = rejected(
        "class Holder { mut p: Pointer<Int32>; construct(p: Pointer<Int32>) { this.p = p; } }
         fn dup<T from Clone>(value: T): Void { }
         fn main(): Void {
             mut x: Int32 = 1;
             unsafe {
                 mut h: Holder = Holder(Pointer.from(x));
                 dup(h);
             }
         }",
    );
    assert!(output.contains(codes::TYPE_MISMATCH.as_str()), "{output}");
    assert!(output.contains("Clone"), "{output}");
}

#[test]
fn valid_class_may_write_implements_clone_explicitly() {
    accepted(
        "class Point implements Clone { mut x: Int32; construct(x: Int32) { this.x = x; } }
         fn main(): Void {
             mut p: Point = Point(1);
             mut q: Point = p.clone();
         }",
    );
}

#[test]
fn valid_manual_clone_implementation_is_dispatched_as_an_ordinary_method() {
    // `Greeter`-typed field: not `Clone`-derivable on its own (a
    // contract-typed member's implementors are not all known to be
    // `Clone`), but a manual `clone()` body bypasses derivation entirely —
    // `Self::check_method_call_on`'s own gate never reaches
    // `check_derived_clone_call` once the class declares the method itself.
    accepted(
        "interface Greeter { fn greet(): String; }
         class Loud implements Greeter { construct() { } fn greet(): String { return \"hi\"; } }
         class Holder { mut g: Greeter; construct(g: Greeter) { this.g = g; } fn clone(): Holder { return this; } }
         fn main(): Void {
             mut h: Holder = Holder(Loud());
             mut clone: Holder = h.clone();
         }",
    );
}
