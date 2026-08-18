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
    let output = rejected_body("mut x: Int64 = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Phase 3b"), "{output}");
}

#[test]
fn valid_short_aliases_of_the_default_integer() {
    // `Int` and `Integer` name `Int32`, which has existed since Phase 1.
    accepted_body("mut a: Int = 1;\nmut b: Integer = 2;\nmut c: Int32 = a + b;");
}

#[test]
fn invalid_float_type_states_its_phase_instead_of_being_unknown() {
    let output = rejected_body("mut x: Float64 = 1;");
    assert!(output.contains(codes::UNKNOWN_TYPE.as_str()));
    assert!(output.contains("Phase 3b"), "{output}");
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
fn invalid_string_iteration_defers_to_the_phase_of_char() {
    // It binds a `Char` — one grapheme — and binding a one-grapheme `String`
    // instead would invent a rule the norm does not have.
    let output = rejected_body("mut text = \"hola\";\nfor c in text { }");
    assert!(output.contains(codes::PENDING_FEATURE.as_str()), "{output}");
    assert!(output.contains("Char"), "{output}");
    assert!(output.contains("Phase 3b"), "{output}");
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
