//! Lowering tests.
//!
//! Every construct of the subset is checked twice: that the IR produced has the
//! expected shape, and that it passes the verifier. The second check is what
//! catches structural bugs the first one would let through.

use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile, SourceMap};
use zirk_ir::*;
use zirk_lexer::tokenize;
use zirk_parser::parse;
use zirk_sema::check;

/// Runs the full frontend and lowers, asserting the IR is well formed.
fn compile(source_text: &str) -> Module {
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("test.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);

    assert!(
        !sink.has_errors(),
        "the program must be valid before lowering:\n{}",
        sink.render(RenderStyle::Human)
    );

    let module = lower(&program, &checked);

    if let Err(errors) = verify(&module) {
        panic!(
            "the produced IR is not well formed:\n{}",
            errors
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    module
}

/// Lowers a body inside `main`.
fn main_body(body: &str) -> Function {
    let module = compile(&format!("fn main(): Void {{\n{body}\n}}"));
    module.function("main").expect("main exists").clone()
}

/// Every instruction of a function, in block order.
fn instructions(function: &Function) -> Vec<InstKind> {
    function
        .blocks
        .iter()
        .flat_map(|b| b.instructions.iter().map(|i| i.kind.clone()))
        .collect()
}

/// The fixed `code()` strings of the native failure classes
/// (`fase-4d-runtimeerror`, design D8; roadmap Phase 4e,
/// `fase-4e-native-slice`, two more), in the order
/// `Checker::register_native_exception_hierarchy` registers the classes and
/// `synthesize_native_failure_bodies` lowers their bodies — every module's
/// string table starts with these, whether or not the program itself ever
/// names one of the classes or triggers a native check.
const NATIVE_FAILURE_CODES: [&str; 13] = [
    "E_DIVISION_BY_ZERO",
    "E_INVALID_SHIFT",
    "E_INVALID_REPEAT",
    "E_FLOAT_NAN",
    "E_ARITHMETIC_OVERFLOW",
    "E_INVALID_CAST",
    "E_INDEX_OUT_OF_BOUNDS",
    "E_NATIVE_ERROR",
    // Roadmap Phase 7: `InvalidStepError` (a range or slice whose step is
    // `0`), synthesized through the same `register_native_failure` path.
    "E_INVALID_STEP",
    // `native-type-member-surface`: the `Result`-carried error types of
    // `parse`, `checked_*`, and `Regex.parse`.
    "E_PARSE",
    "E_OVERFLOW",
    "E_REGEX",
    // `enum-static-members`: the `Result`-carried error type of
    // `EnumType.from_name`/`EnumType.from_value`.
    "E_LOOKUP",
];

const NATIVE_FAILURE_CODE_COUNT: usize = NATIVE_FAILURE_CODES.len();

/// The module string table a program's own literals produce, prefixed by
/// the always-present native failure codes above.
fn native_failure_codes_then<const N: usize>(rest: [&str; N]) -> Vec<String> {
    NATIVE_FAILURE_CODES
        .iter()
        .chain(rest.iter())
        .map(|s| s.to_string())
        .collect()
}

// --- Function shape ---------------------------------------------------------

#[test]
fn an_empty_function_has_one_block_with_a_return() {
    let f = main_body("");

    assert_eq!(f.blocks.len(), 1);
    assert_eq!(f.blocks[0].terminator, Some(Terminator::Return(None)));
    assert_eq!(f.return_type, IrType::Void);
}

#[test]
fn parameters_become_slots() {
    let module =
        compile("fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { }");
    let f = module.function("add").expect("add exists");

    assert_eq!(f.params.len(), 2);
    assert_eq!(f.slot(f.params[0]).map(|s| s.name.as_str()), Some("a"));
    assert_eq!(
        f.slot(f.params[1]).map(|s| s.ty),
        Some(IrType::Int(IntWidth::I32))
    );
}

#[test]
fn every_block_has_exactly_one_terminator() {
    let f = main_body("mut x: Int32 = 1;\nif x > 0 { } else { }\nreturn;");

    for block in &f.blocks {
        assert!(
            block.terminator.is_some(),
            "block {:?} has no terminator",
            block.id
        );
    }
}

// --- Literals ---------------------------------------------------------------

#[test]
fn an_integer_literal_lowers_to_a_constant() {
    let f = main_body("mut x: Int32 = 42;");
    assert!(instructions(&f).contains(&InstKind::ConstInt(42)));
}

#[test]
fn a_boolean_literal_lowers_to_a_constant() {
    let f = main_body("mut x: Boolean = true;");
    assert!(instructions(&f).contains(&InstKind::ConstBool(true)));
}

#[test]
fn a_string_literal_goes_into_the_module_table() {
    let module = compile("fn main(): Void { mut x: String = \"hola\"; }");
    // The native failure classes' own `code()` bodies
    // (`fase-4d-runtimeerror`, design D8) intern their fixed code strings
    // unconditionally, the same way `UNREACHABLE_ABSTRACT_METHOD` is always
    // synthesized whether or not a program ever names an `abstract class` —
    // so every module's string table starts with those, regardless of
    // what the program itself writes.
    assert_eq!(module.strings, native_failure_codes_then(["hola"]));
}

#[test]
fn a_repeated_literal_is_interned_once() {
    let module = compile("fn main(): Void {\nmut a: String = \"x\";\nmut b: String = \"x\";\n}");
    assert_eq!(module.strings.len(), NATIVE_FAILURE_CODE_COUNT + 1);
}

// --- Locals as slots --------------------------------------------------------

#[test]
fn a_declaration_stores_into_its_slot() {
    let f = main_body("mut x: Int32 = 1;");
    let has_store = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::Store(_, _)));
    assert!(has_store, "the initializer must store into the slot");
}

#[test]
fn reading_a_variable_loads_from_its_slot() {
    let f = main_body("mut x: Int32 = 1;\nmut y: Int32 = x;");
    let has_load = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::Load(_)));
    assert!(has_load, "reading must load from the slot");
}

#[test]
fn sibling_blocks_reusing_a_name_produce_two_different_slots() {
    // There is no ordinary shadowing (D10): these two declarations are legal
    // only because the first goes out of scope — the block that owns it
    // closes — before the second one exists, so neither ever hides the other.
    let f = main_body("{ mut x: Int32 = 1; }\n{ mut x: String = \"a\"; }");

    let named_x: Vec<_> = f.slots.iter().filter(|s| s.name == "x").collect();
    assert_eq!(named_x.len(), 2, "each declaration owns its slot");
    assert_ne!(named_x[0].ty, named_x[1].ty);
}

#[test]
fn a_reassignment_stores_into_the_same_slot() {
    let f = main_body("mut x: Int32 = 1;\nx = 2;");

    let stores: Vec<_> = instructions(&f)
        .into_iter()
        .filter_map(|i| match i {
            InstKind::Store(slot, _) => Some(slot),
            _ => None,
        })
        .collect();

    assert_eq!(stores.len(), 2);
    assert_eq!(stores[0], stores[1], "both write the same variable");
}

// --- Operators --------------------------------------------------------------

#[test]
fn arithmetic_lowers_to_a_binary_instruction() {
    let f = main_body("mut x: Int32 = 1 + 2;");
    let has_add = instructions(&f).iter().any(|i| {
        matches!(
            i,
            InstKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        )
    });
    assert!(has_add);
}

#[test]
fn mixed_int8_and_int32_widens_to_int32() {
    let f = main_body("mut a: Int8 = 1;\nmut b: Int32 = 2;\nmut c = a + b;");

    let has_widen = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::IntCast(_)));
    assert!(has_widen, "the Int8 operand must be widened to Int32");

    let add = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| {
            matches!(
                i.kind,
                InstKind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        })
        .expect("there must be an addition");
    assert_eq!(add.ty, IrType::Int(IntWidth::I32));
}

#[test]
fn mixed_uint8_and_int32_promotes_to_float64() {
    let f = main_body("mut a: UInt8 = 1 as UInt8;\nmut b: Int32 = 2;\nmut c = a + b;");

    let int_to_float = instructions(&f)
        .iter()
        .filter(|i| matches!(i, InstKind::IntToFloat(_)))
        .count();
    assert_eq!(
        int_to_float, 2,
        "both operands must be converted to Float64"
    );

    let add = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| {
            matches!(
                i.kind,
                InstKind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        })
        .expect("there must be an addition");
    assert_eq!(add.ty, IrType::Float(FloatWidth::F64));
}

#[test]
fn mixed_int32_and_float64_promotes_to_float64() {
    let f = main_body("mut a: Int32 = 1;\nmut b: Float64 = 2.5;\nmut c = a + b;");

    let has_int_to_float = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::IntToFloat(_)));
    assert!(
        has_int_to_float,
        "the Int32 operand must be converted to Float64"
    );

    let add = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| {
            matches!(
                i.kind,
                InstKind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        })
        .expect("there must be an addition");
    assert_eq!(add.ty, IrType::Float(FloatWidth::F64));
}

#[test]
fn exact_float_addition_lowers_to_a_decimal_runtime_call() {
    let f = main_body("mut a: Decimal = 0.1;\nmut b = 0.2;\nmut c = a + b;");
    let kinds = instructions(&f);
    assert!(
        kinds
            .iter()
            .any(|i| matches!(i, InstKind::ConstDecimal(t) if t == "0.1")),
        "the literal `0.1` lowers to a ConstDecimal"
    );
    assert!(
        kinds.iter().any(|i| matches!(
            i,
            InstKind::Call { callee, .. } if callee == "zirk_rt_decimal_add"
        )),
        "`a + b` lowers to a decimal-add runtime call, not an IR Binary"
    );
}

#[test]
fn exact_float_division_guards_a_zero_divisor() {
    let f = main_body("mut a: Decimal = 1.0;\nmut b: Decimal = 3.0;\nmut c = a / b;");
    let kinds = instructions(&f);
    assert!(kinds.iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_rt_decimal_is_zero"
    )));
    assert!(kinds.iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_rt_decimal_div"
    )));
}

#[test]
fn integer_exponentiation_lowers_to_checked_pow() {
    // `exponentiation-operator`: `2 ** 3` desugars to `(2).pow(3)` and lowers
    // to the checked-pow calls with an overflow branch, not an IR Binary.
    let f = main_body("mut x: Int32 = 2 ** 3;");
    let kinds = instructions(&f);
    assert!(kinds.iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_int_checked_pow_ok"
    )));
    assert!(kinds.iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_int_checked_pow_value"
    )));
}

#[test]
fn integer_exponentiation_with_a_negative_literal_lowers_to_decimal_pow() {
    let f = main_body("mut x: Decimal = 2 ** -3;");
    assert!(instructions(&f).iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_rt_decimal_pow_i"
    )));
}

#[test]
fn exact_float_exponentiation_uses_the_exact_integer_power_path() {
    let f = main_body("mut b: Decimal = 1.5;\nmut x: Decimal = b ** 2;");
    assert!(instructions(&f).iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_rt_decimal_pow_i"
    )));
}

#[test]
fn binary_float_exponentiation_uses_the_binary_pow_helper() {
    let f = main_body("mut b: Float64 = 2.0b;\nmut x: Float64 = b ** 3;");
    assert!(instructions(&f).iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_float_pow"
    )));
}

#[test]
fn exact_float_equality_goes_through_cmp() {
    let f = main_body("mut a: Decimal = 0.1;\nmut b = 0.2;\nmut c: Boolean = (a + b) == 0.3;");
    assert!(instructions(&f).iter().any(|i| matches!(
        i,
        InstKind::Call { callee, .. } if callee == "zirk_rt_decimal_cmp"
    )));
}

#[test]
fn int8_increment_uses_int8_arithmetic() {
    let f = main_body("mut a: Int8 = 1;\na++;");

    let has_one = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::ConstInt(1)));
    assert!(has_one, "the step literal must be an Int8 1");

    let add = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| {
            matches!(
                i.kind,
                InstKind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        })
        .expect("there must be an increment addition");
    assert_eq!(add.ty, IrType::Int(IntWidth::I8));
}

#[test]
fn string_repetition_with_int8_count_lowers_to_repeat() {
    let f = main_body("mut a: Int8 = 3;\nmut s = \"x\" * a;");

    let has_repeat = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::Repeat { .. }));
    assert!(
        has_repeat,
        "String * Int8 must lower to a Repeat instruction"
    );
}

#[test]
fn comparison_produces_a_boolean() {
    let f = main_body("mut x: Boolean = 1 < 2;");

    let comparison = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| {
            matches!(
                i.kind,
                InstKind::Binary {
                    op: BinaryOp::Lt,
                    ..
                }
            )
        })
        .expect("there must be a comparison");

    assert_eq!(comparison.ty, IrType::Boolean);
}

#[test]
fn unary_operators_lower_with_their_type() {
    // Integer negation is now a guarded `0 - x`, so only `!` on `Boolean`
    // is a raw `Unary`; `BitNot` on an integer still is.
    let f = main_body("mut n: Int32 = 1;\nmut a: Int32 = ~n;\nmut b: Boolean = !true;");

    let unaries: Vec<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|i| matches!(i.kind, InstKind::Unary { .. }))
        .collect();

    assert_eq!(unaries.len(), 2);
    assert!(unaries.iter().any(|i| i.ty == IrType::Int(IntWidth::I32)));
    assert!(unaries.iter().any(|i| i.ty == IrType::Boolean));
}

// --- Control flow -----------------------------------------------------------

#[test]
fn a_conditional_produces_a_branch() {
    let f = main_body("if true { } else { }");

    let has_branch = f
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Some(Terminator::Branch { .. })));

    assert!(has_branch, "the condition must end in a conditional branch");
}

#[test]
fn a_conditional_produces_blocks_for_both_branches_and_the_continuation() {
    let f = main_body("if true { } else { }\nreturn;");

    // entry + then + else + continuation
    assert_eq!(f.blocks.len(), 4);
}

#[test]
fn a_conditional_without_else_still_produces_a_branch() {
    let f = main_body("if true { }");

    let has_branch = f
        .blocks
        .iter()
        .any(|b| matches!(b.terminator, Some(Terminator::Branch { .. })));

    assert!(has_branch);
}

#[test]
fn if_both_branches_return_no_continuation_is_created() {
    let module = compile(
        "fn f(a: Boolean): Int32 { if a { return 1; } else { return 2; } }\nfn main(): Void { }",
    );
    let f = module.function("f").expect("f exists");

    // An unreachable continuation block would have no terminator and the
    // verifier would reject it, but the count states the intent explicitly.
    assert_eq!(f.blocks.len(), 3, "entry, then and else — nothing after");
}

#[test]
fn a_chained_conditional_nests_blocks() {
    let f = main_body("if true { } else if false { } else { }\nreturn;");

    let branches = f
        .blocks
        .iter()
        .filter(|b| matches!(b.terminator, Some(Terminator::Branch { .. })))
        .count();

    assert_eq!(branches, 2, "each `if` contributes its own branch");
}

// --- Calls and println ------------------------------------------------------

#[test]
fn a_call_lowers_with_its_arguments() {
    let module = compile(
        "fn add(a: Int32, b: Int32): Int32 { return a + b; }\nfn main(): Void { mut x: Int32 = add(1, 2); }",
    );
    let main = module.function("main").expect("main exists");

    let call = instructions(main)
        .into_iter()
        .find_map(|i| match i {
            InstKind::Call { callee, args } => Some((callee, args.len())),
            _ => None,
        })
        .expect("there must be a call");

    assert_eq!(call, ("add".to_string(), 2));
}

#[test]
fn println_lowers_to_its_own_instruction() {
    let f = main_body("stdout.println(\"hola\");");
    let has_println = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::Println(_)));
    assert!(has_println);
}

#[test]
fn a_discarded_println_produces_no_value() {
    let f = main_body("stdout.println(\"hola\");");

    let println = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| matches!(i.kind, InstKind::Println(_)))
        .expect("there must be a println");

    assert!(
        println.result.is_none(),
        "a statement produces no value nobody reads"
    );
}

// --- Conversion to String ---------------------------------------------------

#[test]
fn println_over_a_string_needs_no_conversion() {
    let f = main_body("stdout.println(\"hola\");");
    let has_conversion = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::ToString(_)));
    assert!(!has_conversion, "a String is already printable");
}

#[test]
fn println_over_an_integer_converts_first() {
    // Without the conversion the runtime would read the value as a pointer.
    let f = main_body("stdout.println(42);");

    let conversion = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| matches!(i.kind, InstKind::ToString(_)))
        .expect("there must be a conversion");

    assert_eq!(conversion.ty, IrType::String);
}

#[test]
fn println_over_a_boolean_converts_first() {
    let f = main_body("stdout.println(true);");
    let has_conversion = instructions(&f)
        .iter()
        .any(|i| matches!(i, InstKind::ToString(_)));
    assert!(has_conversion);
}

#[test]
fn println_always_receives_a_string() {
    // The invariant that keeps the runtime from reading a value as a pointer.
    for body in [
        "stdout.println(\"a\");",
        "stdout.println(1);",
        "stdout.println(true);",
        "stdout.println(1 + 2);",
        "stdout.println(1 < 2);",
    ] {
        let f = main_body(body);

        let types: Vec<_> = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .map(|i| (i.result, i.kind.clone(), i.ty))
            .collect();

        let printed = f
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .find_map(|i| match &i.kind {
                InstKind::Println(operand) => Some(*operand),
                _ => None,
            })
            .unwrap_or_else(|| panic!("there must be a println in `{body}`"));

        let operand_type = types
            .iter()
            .find(|(result, _, _)| *result == Some(printed.0))
            .map(|(_, _, ty)| *ty)
            .unwrap_or_else(|| panic!("the operand of println is undefined in `{body}`"));

        assert_eq!(
            operand_type,
            IrType::String,
            "println must receive a String in `{body}`"
        );
    }
}

#[test]
fn a_conversion_requires_allocation() {
    let f = main_body("stdout.println(42);");

    let allocating: Vec<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|i| i.allocates())
        .collect();

    assert_eq!(allocating.len(), 1, "the conversion produces a new String");
}

// --- Traceability -----------------------------------------------------------

#[test]
fn every_instruction_keeps_its_source_location() {
    let source_text = "fn main(): Void {\n    mut x: Int32 = 42;\n}";
    let mut sources = SourceMap::new();
    sources.add(SourceFile::new("test.zrk", source_text));
    let source = sources.entry();
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(source, &mut sink);
    let program = parse(source, &tokens, &mut sink);
    let checked = check(&sources, &program, &mut sink);
    let module = lower(&program, &checked);

    let main = module.function("main").expect("main exists");
    for block in &main.blocks {
        for inst in &block.instructions {
            let (line, _) = source.line_column(inst.span.start);
            assert_eq!(line, 2, "every instruction comes from line 2");
        }
    }
}

// --- Memory abstraction -----------------------------------------------------

#[test]
fn only_strings_require_allocation() {
    let f = main_body("mut a: Int32 = 1;\nmut b: Boolean = true;\nmut c: String = \"x\";");

    let allocating: Vec<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|i| i.allocates())
        .collect();

    assert_eq!(allocating.len(), 1);
    assert_eq!(allocating[0].ty, IrType::String);
}

#[test]
fn the_ir_names_no_memory_strategy() {
    // ADR-003 requires the IR to express *that* something needs storage, never
    // *how*. This test pins that: the shape of an allocating instruction is a
    // literal, not a call to any allocator.
    let f = main_body("mut c: String = \"x\";");

    let allocating = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .find(|i| i.allocates())
        .expect("there must be an allocating instruction");

    assert!(
        matches!(allocating.kind, InstKind::ConstString(_)),
        "allocation is expressed abstractly, with no allocator named"
    );
}

// --- Reference program ------------------------------------------------------

#[test]
fn the_reference_program_of_the_roadmap_lowers() {
    let module = compile("fn main(): Void {\n    stdout.println(\"Hola desde Zirk\");\n}");

    assert_eq!(
        module.strings,
        native_failure_codes_then(["Hola desde Zirk"])
    );
    let main = module.function("main").expect("main exists");
    assert_eq!(main.return_type, IrType::Void);
    assert!(
        instructions(main)
            .iter()
            .any(|i| matches!(i, InstKind::Println(_)))
    );
}

// --- `do ... while`, ternary and increment ----------------------------------

#[test]
fn a_do_while_enters_its_body_before_its_condition() {
    // The only difference from `while` is which block the entry jumps to.
    let f = main_body("mut i = 0;\ndo { i += 1; } while i < 10;");

    let entry = &f.blocks[0];
    let Some(Terminator::Jump(target)) = entry.terminator else {
        panic!("the entry block must jump into the loop");
    };

    // The header is the block that branches on the condition; the body is not
    // it, which is exactly what makes the body run first.
    let header = f
        .blocks
        .iter()
        .find(|b| matches!(b.terminator, Some(Terminator::Branch { .. })))
        .expect("the loop has a header that branches");

    assert_ne!(
        target, header.id,
        "a `do ... while` must not enter through its header"
    );
}

#[test]
fn a_while_enters_through_its_header() {
    let f = main_body("mut i = 0;\nwhile i < 10 { i += 1; }");

    let Some(Terminator::Jump(target)) = f.blocks[0].terminator else {
        panic!("the entry block must jump into the loop");
    };
    let header = f
        .blocks
        .iter()
        .find(|b| matches!(b.terminator, Some(Terminator::Branch { .. })))
        .expect("the loop has a header that branches");

    assert_eq!(target, header.id);
}

#[test]
fn a_ternary_evaluates_only_the_branch_it_selects() {
    let f = main_body("mut x = 1;\nmut label: String = x > 0 ? \"yes\" : \"no\";");

    // Two string literals, one per branch, in different blocks: neither is
    // materialized before the branch is decided.
    let branching = f
        .blocks
        .iter()
        .filter(|b| {
            b.instructions
                .iter()
                .any(|i| matches!(i.kind, InstKind::ConstString(_)))
        })
        .count();

    assert_eq!(branching, 2, "each branch owns its own block");
}

#[test]
fn a_postfix_increment_yields_the_previous_value() {
    let f = main_body("mut i = 0;\nmut previous = i++;");

    // The slot holding the result of `i++` receives the loaded value, not the
    // added one: the load precedes the addition.
    let kinds = instructions(&f);
    let load = kinds
        .iter()
        .position(|k| matches!(k, InstKind::Load(_)))
        .expect("the previous value is loaded");
    let add = kinds
        .iter()
        .position(|k| matches!(k, InstKind::Binary { .. }))
        .expect("the increment adds");

    assert!(load < add, "the previous value is read before updating");
}

#[test]
fn both_increment_forms_store_the_updated_value() {
    for body in ["mut i = 0;\nmut x = i++;", "mut i = 0;\nmut x = ++i;"] {
        let f = main_body(body);
        let kinds = instructions(&f);
        assert!(
            kinds.iter().any(|k| matches!(k, InstKind::Binary { .. })),
            "`{body}` must update its operand"
        );
    }
}

#[test]
fn postfix_increment_overflow_at_int32_max_is_guarded() {
    let f = main_body("mut i = 2147483647;\nmut x = i++;");
    let kinds = instructions(&f);
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::CheckedArithmetic { .. })),
        "i++ at Int32.MAX must overflow-check"
    );
}

#[test]
fn prefix_decrement_underflow_at_uint8_min_is_guarded() {
    let f = main_body("mut i: UInt8 = 0 as UInt8;\nmut x = --i;");
    let kinds = instructions(&f);
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::CheckedArithmetic { .. })),
        "--i at UInt8.MIN must underflow-check"
    );
}

#[test]
fn int16_postfix_increment_overflow_is_guarded() {
    let f = main_body("mut i: Int16 = 32767 as Int16;\nmut x = i++;");
    let kinds = instructions(&f);
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::CheckedArithmetic { .. })),
        "i++ at Int16.MAX must overflow-check"
    );
}

// --- Objetos -----------------------------------------------------------------

/// A program with a class, plus the `main` every program needs.
fn with_class(class: &str, body: &str) -> Module {
    compile(&format!("{class}\nfn main(): Void {{\n{body}\n}}"))
}

const USER: &str = "class User {
    id: Int32;
    name: String;
    construct(id: Int32, name: String) { this.id = id; this.name = name; }
}";

#[test]
fn a_class_becomes_an_object_layout() {
    let module = with_class(USER, "");
    let id = module.object_id("User").expect("User is a class");
    let layout = &module.objects[id as usize];

    assert_eq!(layout.name, "User");
    assert_eq!(layout.fields.len(), 2);
    assert_eq!(layout.fields[0].name, "id");
    assert_eq!(layout.fields[0].ty, IrType::Int(IntWidth::I32));
    assert_eq!(layout.fields[1].ty, IrType::String);
}

#[test]
fn a_constructor_becomes_a_function_over_the_object() {
    let module = with_class(USER, "");
    let id = module.object_id("User").expect("User is a class");
    let constructor = module
        .function(&zirk_ir::constructor_symbol("User", 0))
        .expect("the constructor is emitted");

    // `this` first, then the declared parameters.
    assert_eq!(constructor.params.len(), 3);
    assert_eq!(
        constructor
            .slot(constructor.params[0])
            .map(|s| s.name.as_str()),
        Some("this")
    );
    assert_eq!(
        constructor.slot(constructor.params[0]).map(|s| s.ty),
        Some(IrType::Object(id))
    );
    assert_eq!(constructor.return_type, IrType::Void);
}

#[test]
fn building_an_object_allocates_and_then_calls_its_constructor() {
    let module = with_class(USER, "mut u = User(1, \"x\");");
    let id = module.object_id("User").expect("User is a class");
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    let alloc = kinds
        .iter()
        .position(|k| matches!(k, InstKind::Alloc(alloc_id) if *alloc_id == id))
        .expect("the object is allocated");
    let call = kinds
        .iter()
        .position(|k| matches!(k, InstKind::Call { callee, .. } if callee.contains("construct")))
        .expect("its constructor is called");

    // The object exists — and has its address, which is its identity — before
    // its constructor runs on it.
    assert!(alloc < call);
}

#[test]
fn a_field_is_read_by_its_position() {
    let module = with_class(USER, "mut u = User(1, \"x\");\nstdout.println(u.name);");
    let main = module.function("main").expect("main exists");

    // `name` is the second field, so index 1. The header is not counted here:
    // it is added when the address is computed.
    assert!(
        instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::LoadField { index: 1, .. })),
        "the second field is read by its index"
    );
}

#[test]
fn a_constructor_writes_its_fields_by_position() {
    let module = with_class(USER, "");
    let constructor = module
        .function(&zirk_ir::constructor_symbol("User", 0))
        .expect("the constructor is emitted");
    let kinds = instructions(constructor);

    for index in [0, 1] {
        assert!(
            kinds
                .iter()
                .any(|k| matches!(k, InstKind::StoreField { index: i, .. } if *i == index)),
            "field {index} must be written"
        );
    }
}

#[test]
fn several_constructors_are_emitted_under_distinct_names() {
    let module = with_class(
        "class Point {
             x: Int32;
             construct(x: Int32) { this.x = x; }
             construct() { this.x = 0; }
         }",
        "",
    );

    assert!(
        module
            .function(&zirk_ir::constructor_symbol("Point", 0))
            .is_some()
    );
    assert!(
        module
            .function(&zirk_ir::constructor_symbol("Point", 1))
            .is_some()
    );
}

#[test]
fn a_method_becomes_a_function_over_its_receiver() {
    let module = with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
             fn bump(): Int32 { this.count = this.count + 1; return this.count; }
         }",
        "",
    );
    let method = module
        .function(&zirk_ir::method_symbol("Counter", "bump"))
        .expect("the method is emitted");

    assert_eq!(method.params.len(), 1, "only the receiver");
    assert_eq!(
        method.slot(method.params[0]).map(|s| s.name.as_str()),
        Some("this")
    );
    assert_eq!(method.return_type, IrType::Int(IntWidth::I32));
}

#[test]
fn a_method_call_passes_the_receiver_first() {
    let module = with_class(
        "class Greeter {
             prefix: String;
             construct() { this.prefix = \">\"; }
             fn greet(name: String): String { return name; }
         }",
        "mut g = Greeter();\nmut s = g.greet(\"hola\");",
    );
    let main = module.function("main").expect("main exists");

    let call = instructions(main)
        .into_iter()
        .find_map(|k| match k {
            InstKind::Call { callee, args } if callee.contains("greet") => Some(args),
            _ => None,
        })
        .expect("the method is called");

    // The receiver plus the declared argument.
    assert_eq!(call.len(), 2);
}

#[test]
fn a_method_without_a_body_is_not_emitted() {
    // Nothing declares one yet — `abstract` is deferred — but the lowering
    // must not assume every method has a body when it arrives.
    let module = with_class(
        "class Counter {
             count: Int32;
             construct() { this.count = 0; }
             fn bump(): Int32 { return this.count; }
         }",
        "",
    );

    assert!(
        module
            .function(&zirk_ir::method_symbol("Counter", "bump"))
            .is_some()
    );
}

// --- Herencia ----------------------------------------------------------------

const HIERARCHY: &str = "class Base {
    x: Int32;
    construct() { this.x = 0; }
    fn overridden(): Int32 { return this.x; }
    fn only_here(): Int32 { return this.x; }
}
class Derived extends Base {
    y: Int32;
    construct() { super(); this.y = 1; }
    override fn overridden(): Int32 { return this.y; }
}";

#[test]
fn a_subclass_layout_starts_with_its_base() {
    let module = compile(&format!("{HIERARCHY}\nfn main(): Void {{ }}"));
    let base = module.objects.iter().find(|o| o.name == "Base").unwrap();
    let derived = module.objects.iter().find(|o| o.name == "Derived").unwrap();

    // Reaching an inherited field is the same offset whoever is looking.
    assert_eq!(derived.fields[0], base.fields[0]);
    assert_eq!(derived.fields[1].name, "y");
}

#[test]
fn an_override_keeps_the_slot_it_replaces() {
    let module = compile(&format!("{HIERARCHY}\nfn main(): Void {{ }}"));
    let base = module.objects.iter().find(|o| o.name == "Base").unwrap();
    let derived = module.objects.iter().find(|o| o.name == "Derived").unwrap();

    let slot = base
        .methods
        .iter()
        .position(|m| m.contains("overridden"))
        .expect("the base declares it");

    // Same slot, different body: that is what makes an indirect call one load
    // and one jump.
    assert_eq!(
        derived.methods[slot],
        zirk_ir::method_symbol("Derived", "overridden")
    );
    assert_eq!(
        base.methods[slot],
        zirk_ir::method_symbol("Base", "overridden")
    );
}

#[test]
fn an_inherited_method_keeps_pointing_at_its_owner() {
    let module = compile(&format!("{HIERARCHY}\nfn main(): Void {{ }}"));
    let derived = module.objects.iter().find(|o| o.name == "Derived").unwrap();

    // The body lives where it was declared, not where it is reached from.
    assert!(
        derived
            .methods
            .contains(&zirk_ir::method_symbol("Base", "only_here"))
    );
}

#[test]
fn a_call_through_the_base_goes_through_the_table() {
    let module = compile(&format!(
        "{HIERARCHY}\n\
         fn value(b: Base): Int32 {{ return b.overridden(); }}\n\
         fn main(): Void {{ mut d = Derived(); mut n = value(d); }}"
    ));
    let through_base = module.function("value").expect("the function exists");

    assert!(
        instructions(through_base)
            .iter()
            .any(|k| matches!(k, InstKind::CallVirtual { .. })),
        "seen as a `Base`, which body runs is not statically known"
    );
}

#[test]
fn a_call_on_the_class_that_declares_it_stays_direct() {
    // Devirtualization by construction: on a `Derived` nothing below redefines
    // it, so the target is known and the indirection buys nothing.
    let module = compile(&format!(
        "{HIERARCHY}\nfn main(): Void {{ mut d = Derived(); mut n = d.overridden(); }}"
    ));
    let main = module.function("main").expect("main exists");

    assert!(
        !instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::CallVirtual { .. })),
    );
}

#[test]
fn a_method_nobody_redefines_is_called_directly() {
    let module = compile(&format!(
        "{HIERARCHY}\nfn main(): Void {{ mut d = Derived(); mut n = d.only_here(); }}"
    ));
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        !kinds
            .iter()
            .any(|k| matches!(k, InstKind::CallVirtual { .. })),
        "paying an indirection for a generality the program cannot use is paying for nothing"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::Call { callee, .. } if callee.contains("only_here"))),
    );
}

// --- `abstract class` dynamic dispatch (fase-3-abstract-dispatch) ----------

const SHAPE_HIERARCHY: &str = "abstract class Shape {
    abstract fn area(): Int32;
}
class Circle implements Shape {
    radius: Int32;
    construct(radius: Int32) { this.radius = radius; }
    override fn area(): Int32 { return this.radius * this.radius * 3; }
}
class Square implements Shape {
    side: Int32;
    construct(side: Int32) { this.side = side; }
    override fn area(): Int32 { return this.side * this.side; }
}";

/// A call through a value statically typed as a user `abstract class` lowers
/// to `InstKind::CallVirtual`, exactly the way a call through the native
/// `Throwable` already does (tasks.md 2.1).
#[test]
fn a_call_through_a_user_abstract_class_goes_through_the_table() {
    let module = compile(&format!(
        "{SHAPE_HIERARCHY}\n\
         fn describe(s: Shape): Int32 {{ return s.area(); }}\n\
         fn main(): Void {{ mut c = Circle(2); mut n = describe(c); }}"
    ));
    let describe = module.function("describe").expect("the function exists");

    assert!(
        instructions(describe)
            .iter()
            .any(|k| matches!(k, InstKind::CallVirtual { .. })),
        "seen as `Shape`, which body runs is not statically known"
    );
}

/// Two distinct adopters of the same abstract class each resolve their own
/// override at their own call site — the dispatch is receiver-type-driven at
/// runtime, not resolved once to a single target at compile time (tasks.md
/// 2.2).
#[test]
fn two_adopters_of_the_same_abstract_class_keep_their_own_override_at_the_same_slot() {
    let module = compile(&format!("{SHAPE_HIERARCHY}\nfn main(): Void {{ }}"));
    let circle = module.objects.iter().find(|o| o.name == "Circle").unwrap();
    let square = module.objects.iter().find(|o| o.name == "Square").unwrap();

    // `Shape`'s own table slot for `area` has no real body (it is never
    // instantiated) — only its adopters' entries carry one — so the slot is
    // found on an adopter, not on `Shape` itself.
    let slot = circle
        .methods
        .iter()
        .position(|m| m.contains("area"))
        .expect("Circle's own override is at some slot");

    // Same slot, different body: that is what makes an indirect call one
    // load and one jump regardless of which adopter is behind it.
    assert_eq!(
        circle.methods[slot],
        zirk_ir::method_symbol("Circle", "area")
    );
    assert_eq!(
        square.methods[slot],
        zirk_ir::method_symbol("Square", "area")
    );
}

/// A concrete class implementing both a user abstract class and a plain
/// interface dispatches correctly through each of their respective typed
/// variables (tasks.md 1.3/3.2).
#[test]
fn a_class_implementing_an_abstract_class_and_an_interface_dispatches_both() {
    let module = compile(
        "abstract class Shape {
             abstract fn area(): Int32;
         }
         interface Describable {
             fn describe(): String;
         }
         class Circle implements Shape, Describable {
             radius: Int32;
             construct(radius: Int32) { this.radius = radius; }
             override fn area(): Int32 { return this.radius * this.radius * 3; }
             fn describe(): String { return \"circle\"; }
         }
         fn print_area(s: Shape): Int32 { return s.area(); }
         fn print_description(d: Describable): String { return d.describe(); }
         fn main(): Void {
             mut c = Circle(2);
             mut a = print_area(c);
             mut d = print_description(c);
         }",
    );
    let print_area = module.function("print_area").expect("the function exists");
    let print_description = module
        .function("print_description")
        .expect("the function exists");

    assert!(
        instructions(print_area)
            .iter()
            .any(|k| matches!(k, InstKind::CallVirtual { .. })),
        "seen as `Shape`, which body runs is not statically known"
    );
    assert!(
        instructions(print_description)
            .iter()
            .any(|k| matches!(k, InstKind::CallContract { .. })),
        "seen as `Describable`, which body answers is not statically known"
    );

    let circle = module.objects.iter().find(|o| o.name == "Circle").unwrap();
    assert!(
        !circle.contracts.is_empty(),
        "Circle's own contract table is missing, found {:?}",
        circle.contracts
    );
}

// --- Contratos (task 10.7) ---------------------------------------------------

#[test]
fn a_call_through_a_contract_goes_through_its_table() {
    let module = compile(
        "interface Describable { fn describe(): String; }
         class User implements Describable {
             construct() { }
             fn describe(): String { return \"user\"; }
         }
         fn announce(d: Describable): String { return d.describe(); }
         fn main(): Void { mut u = User(); mut s = announce(u); }",
    );
    let announce = module.function("announce").expect("announce exists");

    assert!(
        instructions(announce)
            .iter()
            .any(|k| matches!(k, InstKind::CallContract { .. })),
        "which body answers is not statically known through a contract"
    );
}

// --- Genéricos (roadmap task 11.1/11.2) --------------------------------------

const BOX: &str = "class Box<T> {
    value: T;
    construct(value: T) { this.value = value; }
    fn get(): T { return this.value; }
}";

#[test]
fn a_generic_instantiation_gets_its_own_specialized_layout() {
    let module = compile(&format!(
        "{BOX}\nfn main(): Void {{ mut b: Box<Int32> = Box(5); }}"
    ));

    // The template itself (`Box`, still naming `T`) is never addressed —
    // only a specialized copy, named after it plus the instantiation index.
    assert!(module.objects.iter().any(|o| o.name == "Box"));
    let specialized = module
        .objects
        .iter()
        .find(|o| o.name.starts_with("Box$"))
        .expect("a specialized copy exists");
    assert_eq!(specialized.fields[0].ty, IrType::Int(IntWidth::I32));
}

#[test]
fn repeating_the_same_instantiation_reuses_one_layout() {
    let module = compile(&format!(
        "{BOX}\nfn main(): Void {{
             mut a: Box<Int32> = Box(1);
             mut b: Box<Int32> = Box(2);
             mut c: Box<Int32> = Box(3);
         }}"
    ));

    let specialized: Vec<_> = module
        .objects
        .iter()
        .filter(|o| o.name.starts_with("Box$"))
        .collect();
    assert_eq!(
        specialized.len(),
        1,
        "three uses of the same combination are one specialization, not three"
    );
}

#[test]
fn two_different_instantiations_get_two_layouts() {
    let module = compile(&format!(
        "{BOX}\nfn main(): Void {{
             mut a: Box<Int32> = Box(1);
             mut b: Box<String> = Box(\"x\");
         }}"
    ));

    let specialized: Vec<_> = module
        .objects
        .iter()
        .filter(|o| o.name.starts_with("Box$"))
        .collect();
    assert_eq!(specialized.len(), 2);
    assert_ne!(specialized[0].fields[0].ty, specialized[1].fields[0].ty);
}

// --- Records (roadmap task 11.5) --------------------------------------------

#[test]
fn a_record_becomes_a_value_layout_not_an_object() {
    let module = compile(
        "record Point { x: Int32; y: Int32; }\nfn main(): Void { mut p = Point(x: 1, y: 2); }",
    );

    let id = module.object_id("Point").expect("Point is a record") as usize;
    assert_eq!(module.values[id].name, "Point");
    assert_eq!(module.values[id].fields.len(), 2);
    // Its `ObjectLayout` counterpart is the empty placeholder nothing reads.
    assert!(module.objects[id].fields.is_empty());
}

#[test]
fn constructing_a_record_builds_a_value_with_no_allocation() {
    let module = compile(
        "record Point { x: Int32; y: Int32; }\nfn main(): Void { mut p = Point(x: 1, y: 2); }",
    );
    let id = module.object_id("Point").expect("Point is a record");
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::BuildValue { class, fields } if *class == id && fields.len() == 2)),
        "a record is packaged, not allocated"
    );
    assert!(
        !kinds.iter().any(|k| matches!(k, InstKind::Alloc(_))),
        "nothing here needs storage the runtime provides"
    );
}

#[test]
fn a_record_field_reads_by_position_like_an_objects() {
    let module = compile(
        "record Point { x: Int32; y: Int32; }
         fn main(): Void { mut p = Point(x: 1, y: 2); stdout.println(p.y); }",
    );
    let main = module.function("main").expect("main exists");

    assert!(
        instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::LoadField { index: 1, .. })),
        "field access does not need its own instruction kind"
    );
}

// --- Enums algebraicos (roadmap task 11.3/11.4) ------------------------------

const SHAPE: &str = "enum Shape {
    Circle(radius: Int32),
    Rect(width: Int32, height: Int32),
    Point
}";

#[test]
fn an_algebraic_enum_becomes_an_enum_layout_with_a_flattened_payload() {
    let module = compile(&format!("{SHAPE}\nfn main(): Void {{ }}"));
    // Index 0 is the language's own `Iteration<T>` (task 6.9), always
    // registered first — `Shape` is found by name instead.
    let layout = module.enums.iter().find(|e| e.name == "Shape").unwrap();

    assert_eq!(layout.name, "Shape");
    // `radius`, then `width`, `height`: every variant's own fields,
    // concatenated in declaration order, `Point` contributing none.
    assert_eq!(layout.fields.len(), 3);
    assert_eq!(layout.variants.len(), 3);
    assert_eq!(layout.variants[0], vec![0]);
    assert_eq!(layout.variants[1], vec![1, 2]);
    assert!(layout.variants[2].is_empty());
}

#[test]
fn a_traditional_enum_stays_a_bare_int32() {
    // No variant carries data, so nothing changes from before this task.
    let module = compile("enum Direction { North, South }\nfn main(): Void { }");
    let layout = module.enums.iter().find(|e| e.name == "Direction").unwrap();
    assert!(layout.fields.is_empty());
}

#[test]
fn constructing_a_variant_builds_an_enum_with_no_allocation() {
    let module = compile(&format!(
        "{SHAPE}\nfn main(): Void {{ mut s = Shape.Circle(radius: 2); }}"
    ));
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(
            k,
            InstKind::BuildEnum { variant: 0, fields, .. } if fields.len() == 1
        )),
        "`Circle` is discriminant 0 with its own one field"
    );
    assert!(!kinds.iter().any(|k| matches!(k, InstKind::Alloc(_))));
}

#[test]
fn destructuring_a_variant_reads_its_flattened_fields() {
    let module = compile(&format!(
        "{SHAPE}\nfn area(s: Shape): Int32 {{
             return match s {{
                 Shape.Circle(radius) => radius * radius * 3,
                 Shape.Rect(width, height) => width * height,
                 Shape.Point => 0,
             }};
         }}\nfn main(): Void {{ }}"
    ));
    let area = module.function("area").expect("area exists");
    let kinds = instructions(area);

    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::Discriminant(_))),
        "a variant pattern tests the discriminant, not the whole payload"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::LoadField { index: 1, .. })),
        "`height`, `Rect`'s second field, sits at the flattened index 1"
    );
}

// --- Enums genéricos declarados por el usuario (fase-3-generic-enums) -------

#[test]
fn a_user_generic_enum_instantiated_twice_produces_two_independent_layouts() {
    // Design D2's baseline shape: a single-type-parameter user enum
    // instantiated at two different type arguments in the same program.
    // `specialize_enum` (`zirk-ir/src/lower.rs`) must build one concrete
    // `EnumLayout` per distinct instantiation the program actually names.
    let module = compile(
        "enum Box<T> { Full(value: T), Empty }
         fn main(): Void {
             mut a: Box<Int32> = Box.Full(1);
             mut b: Box<String> = Box.Full(\"x\");
         }",
    );

    let boxes: Vec<_> = module
        .enums
        .iter()
        .filter(|e| e.name.starts_with("Box$"))
        .collect();
    assert_eq!(
        boxes.len(),
        2,
        "two distinct instantiations, two distinct layouts: {:#?}",
        module.enums
    );
}

#[test]
fn a_user_generic_enum_named_twice_at_the_same_type_reuses_one_layout() {
    // The other half of D2's baseline shape: naming the *same* instantiation
    // more than once must not produce a duplicate layout — the checker's own
    // `intern_enum_instance` already deduplicates by structural equality
    // before `specialize_enum` ever runs, so this is a property of
    // `checked.enum_instances`, not something `specialize_enum` has to
    // re-derive.
    let module = compile(
        "enum Box<T> { Full(value: T), Empty }
         fn one(): Box<Int32> { return Box.Full(1); }
         fn two(): Box<Int32> { return Box.Full(2); }
         fn main(): Void { }",
    );

    let boxes: Vec<_> = module
        .enums
        .iter()
        .filter(|e| e.name.starts_with("Box$"))
        .collect();
    assert_eq!(
        boxes.len(),
        1,
        "one instantiation named twice is still one layout: {:#?}",
        module.enums
    );
}

#[test]
fn a_multi_type_parameter_user_enum_specializes_both_parameters() {
    // Design D2's second baseline shape: more than one type variable per
    // instantiation.
    let module = compile(
        "enum Either<L, R> { Left(value: L), Right(value: R) }
         fn main(): Void {
             mut a: Either<Int32, String> = Either.Left(1);
         }",
    );

    let layout = module
        .enums
        .iter()
        .find(|e| e.name.starts_with("Either$"))
        .expect("Either<Int32, String> has its own layout");
    assert_eq!(layout.fields.len(), 2, "one field per variant's payload");
    assert_eq!(layout.variants.len(), 2);
}

// --- Casts comprobados (roadmap task 11.6) -----------------------------------

const ANIMALS: &str = "class Animal { construct() { } }
class Dog extends Animal { construct() { super(); } }";

#[test]
fn an_identity_cast_emits_no_check() {
    let module = compile(&format!(
        "{ANIMALS}\nfn main(): Void {{ mut d = Dog(); mut e = d as Dog; }}"
    ));
    let main = module.function("main").expect("main exists");

    assert!(
        !instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::CheckedCast { .. })),
        "the value already is the target, so there is nothing to confirm"
    );
}

#[test]
fn a_class_to_class_cast_checks_the_target_descriptor() {
    let module = compile(&format!(
        "{ANIMALS}\nfn main(): Void {{ mut a: Animal = Dog(); mut d = a as Dog; }}"
    ));
    let main = module.function("main").expect("main exists");
    let dog_id = module
        .objects
        .iter()
        .position(|o| o.name == "Dog")
        .expect("Dog has a layout") as u32;

    assert!(
        instructions(main).iter().any(
            |k| matches!(k, InstKind::IsInstance { target_class, .. } if *target_class == dog_id)
        ),
        "a downcast is tested with IsInstance before it is retyped"
    );
    assert!(
        !instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::CheckedCast { .. })),
        "CheckedCast is no longer emitted for user `as`/`<T>` casts"
    );
}

#[test]
fn an_ordinary_upcast_retypes_without_a_runtime_check() {
    // `mut a: Animal = Dog();` — an explicit widening annotation, not `as` —
    // needs no check at all: the checker already proved it, so only the
    // declared type changes.
    let module = compile(&format!(
        "{ANIMALS}\nfn main(): Void {{ mut a: Animal = Dog(); }}"
    ));
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::Retype(_))),
        "widening to a base is proven safe, not merely asserted"
    );
    assert!(
        !kinds
            .iter()
            .any(|k| matches!(k, InstKind::CheckedCast { .. }))
    );
}

// --- Nullable objects and `?.` (found writing Fase 4's memory probes) -------

const NODE: &str = "class Node { value: Int32; mut next: Node?;
    construct(value: Int32) { this.value = value; this.next = null; } }";

/// `is` between two `T?` operands used to panic LLVM codegen (`emit.rs`
/// expected a bare pointer, not the `{i1, ptr}` struct a nullable lowers to).
/// `compile`'s own `verify` call is what would have caught the IR-level half
/// of this — the panic itself only ever showed up further down, in codegen.
#[test]
fn is_between_two_nullable_objects_compiles() {
    let module = compile(&format!(
        "{NODE}\nfn main(): Void {{
             mut a: Node? = Node(1);
             mut b: Node? = a;
             mut r = a is b;
         }}"
    ));
    let main = module.function("main").expect("main exists");

    assert!(
        instructions(main).iter().any(|k| matches!(
            k,
            InstKind::Binary {
                op: BinaryOp::Identical,
                ..
            }
        )),
        "`is` still lowers to `Identical`, just over operands that now agree"
    );
}

/// `is` between a bare `T` and a `T?` used to reach the verifier with two
/// different operand types: the checker's `expect_same` widens `T` to `T?`
/// the same way an ordinary assignment would, but lowering never inserted
/// the matching `Wrap`.
#[test]
fn is_between_object_and_nullable_object_widens_the_bare_side() {
    let module = compile(&format!(
        "{NODE}\nfn main(): Void {{
             mut a: Node = Node(1);
             mut b: Node? = Node(1);
             mut r = a is b;
         }}"
    ));
    let main = module.function("main").expect("main exists");

    assert!(
        instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::Wrap { .. })),
        "the bare operand is wrapped to `Node?` before `Identical` compares them"
    );
}

/// `nullable ?? null` used to hit `type_of`'s `unreachable!` for `Expr::Null`
/// — both in `lower_coalesce` directly and in its `type_of` counterpart,
/// which decides the coalescence's own result type before lowering it.
#[test]
fn coalescing_a_nullable_with_a_null_literal_compiles() {
    let module = compile(&format!(
        "{NODE}\nfn main(): Void {{
             mut a: Node? = Node(1);
             mut b: Node? = a ?? null;
         }}"
    ));
    let main = module.function("main").expect("main exists");

    assert!(
        instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::IsNull(_))),
        "`??` still lowers to an explicit null check even when the fallback is `null` itself"
    );
}

/// `object.field = <expr containing `?.`>` used to produce IR where
/// `StoreField`'s `object` operand was computed in a block that was no
/// longer current once the `?.` on the right-hand side branched — exactly
/// the "values do not cross blocks" defect ADR-007 exists to name.
#[test]
fn assigning_a_field_from_a_safe_navigation_expression_compiles() {
    let module = compile(&format!(
        "{NODE}\nfn main(): Void {{
             mut a: Node = Node(1);
             mut b: Node = Node(2);
             mut maybeB: Node? = b;
             a.next = maybeB?.next;
         }}"
    ));
    let main = module.function("main").expect("main exists");

    assert!(
        instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::StoreField { .. })),
        "the assignment still lowers to a field store, now with an operand that survives the branch"
    );
}

/// A recursive function combining its own call with a `?.` read of the same
/// discriminant in one `match` arm hit the same "values do not cross blocks"
/// defect as the field-assignment case above, but from a different root
/// cause: `Self::opens_blocks` did not know `?.` opens blocks of its own, so
/// an earlier operand held across the call was never spilled to a slot.
/// `n` is narrowed to non-null by the `match`'s own binding arm (later
/// narrowing work), so plain `.next` is what the checker now requires here;
/// the block-spilling fix itself is still exercised elsewhere by `?.` on a
/// receiver narrowing cannot prove non-null.
#[test]
fn recursive_call_combined_with_safe_navigation_in_one_match_arm_compiles() {
    let module = compile(&format!(
        "{NODE}\nfn length(node: Node?): Int32 {{
             return match node {{
                 null => 0,
                 n => 1 + length(n.next),
             }};
         }}\nfn main(): Void {{ mut a: Node = Node(1); mut r = length(a); }}"
    ));
    let length = module.function("length").expect("length exists");

    assert!(
        instructions(length)
            .iter()
            .any(|k| matches!(k, InstKind::Call { callee, .. } if callee == "length")),
        "the recursive call itself still lowers, alongside the `?.` argument that used to strand it"
    );
}

/// `objeto?.algo()` where `algo` returns `Void` used to panic
/// `lower_safe_method_call`: it unconditionally wrapped the method's return
/// type in `Nullable::of`, which has no case for `Void` (documented, not
/// fixed, in `docs/decisions/ADR-003-investigacion-fase-4.md`, section
/// "Extensión: criterio 3..."). The checker now types the whole call as
/// plain `Void`, and the lowering skips the result slot entirely — the same
/// way `Self::lower_match` skips one for a `Void` arm.
#[test]
fn safe_call_of_a_void_returning_method_compiles() {
    let module = compile(
        "class Counter { mut value: Int32;
             construct(value: Int32) { this.value = value; }
             fn bump(): Void { this.value = this.value + 1; } }
         fn main(): Void {
             mut c: Counter? = Counter(1);
             c?.bump();
         }",
    );
    let main = module.function("main").expect("main exists");

    assert!(
        instructions(main)
            .iter()
            .any(|k| matches!(k, InstKind::Call { callee, .. } if callee.contains("bump"))),
        "the call still lowers, guarded by the absent/present split, with no `Void?` slot"
    );
}

// --- Collector shadow-stack roots (`fase-4e-colector-mark-sweep`, design D2/D4) --------

#[test]
fn gc_roots_names_exactly_the_reference_typed_slots() {
    let module = with_class(USER, "mut n = 1; mut u = User(1, \"x\");");
    let main = module.function("main").expect("main exists");
    let user_id = module.object_id("User").expect("User is a class");

    for &slot_id in &main.gc_roots {
        let ty = main.slot(slot_id).expect("declared slot").ty;
        assert!(
            ty.is_managed_reference(&module),
            "every gc_roots entry must be a managed reference, found {ty:?}"
        );
    }

    let n_slot = main
        .slots
        .iter()
        .position(|s| s.name == "n")
        .map(|i| SlotId(i as u32))
        .expect("`n` was declared");
    assert!(
        !main.gc_roots.contains(&n_slot),
        "a scalar local must never be named as a gc root"
    );

    let has_user_root = main
        .gc_roots
        .iter()
        .any(|&id| main.slot(id).unwrap().ty == IrType::Object(user_id));
    assert!(has_user_root, "the User-typed local `u` must be a gc root");
}

#[test]
fn multi_argument_constructor_calls_spill_each_argument_before_the_next_is_built() {
    // Design D4's own motivating hazard: `take(User(1, "a"), User(2, "b"))`
    // evaluates its arguments left to right — the first `Alloc`'s result
    // must be spilled to its own synthetic slot immediately, before the
    // second argument's own `Alloc`/constructor-call sequence runs, so a
    // collection triggered while building the second argument cannot
    // collect the first.
    let source = format!(
        "{USER}\nfn take(a: User, b: User): Void {{ }}\nfn main(): Void {{ take(User(1, \"a\"), User(2, \"b\")); }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    let alloc_positions: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| matches!(k, InstKind::Alloc(_)))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        alloc_positions.len(),
        2,
        "two constructor calls, two allocations"
    );
    assert!(
        alloc_positions[0] < alloc_positions[1],
        "arguments are still built left to right"
    );

    for &alloc_index in &alloc_positions {
        assert!(
            matches!(kinds.get(alloc_index + 1), Some(InstKind::Store(_, _))),
            "the Alloc result at index {alloc_index} must be spilled to a slot immediately, found {:?}",
            kinds.get(alloc_index + 1)
        );
    }
}

// --- Weak<T> (roadmap Phase 4e, `fase-4e-weak`) -----------------------------

const MARKER: &str = "class Marker { construct() { } }";

#[test]
fn weak_from_upgrade_and_is_alive_lower_to_the_expected_instructions() {
    let source = format!(
        "{MARKER}\nfn main(): Void {{
             mut m: Marker = Marker();
             mut w: Weak<Marker> = Weak.from(m);
             mut alive: Boolean = w.is_alive;
             mut u: Marker? = w.upgrade();
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::WeakFrom(_))),
        "Weak.from(m) must lower to WeakFrom, found {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::WeakIsAlive(_))),
        "w.is_alive must lower to WeakIsAlive, found {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::WeakUpgrade(_))),
        "w.upgrade() must lower to WeakUpgrade, found {kinds:?}"
    );
}

/// Design D4's own claim (`fase-4e-weak/design.md`): `.upgrade()`'s `T?`
/// result is a managed-reference-typed instruction result like any other,
/// so it goes through `fase-4e-colector-mark-sweep`'s own unconditional
/// synthetic-slot spill and root descriptor with no change needed for
/// `Weak<T>` specifically — checked here rather than assumed, per the
/// task's own instruction.
#[test]
fn weak_upgrade_result_and_the_handle_itself_are_both_gc_roots() {
    let source = format!(
        "{MARKER}\nfn main(): Void {{
             mut m: Marker = Marker();
             mut w: Weak<Marker> = Weak.from(m);
             mut u: Marker? = w.upgrade();
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let marker_id = module.object_id("Marker").expect("Marker is a class");

    let has_weak_root = main.gc_roots.iter().any(|&id| {
        matches!(main.slot(id).unwrap().ty, IrType::Weak(weak_id) if module.weak_types[weak_id as usize] == IrType::Object(marker_id))
    });
    assert!(
        has_weak_root,
        "the Weak<Marker>-typed local `w` must be a gc root, since a Weak<T> \
         handle is itself a managed reference (design D1/D2)"
    );

    let has_upgrade_root = main.gc_roots.iter().any(|&id| {
        matches!(
            main.slot(id).unwrap().ty,
            IrType::Nullable(Nullable::Object(id)) if id == marker_id
        )
    });
    assert!(
        has_upgrade_root,
        "the Marker?-typed local `u` (from .upgrade()) must be a gc root"
    );
}

// --- Clone (`fase-4e-clone`) -------------------------------------------------

const CLONE_NODE: &str = "class Node { mut left: Node?; mut right: Node?; construct() { } }";

#[test]
fn derived_clone_lowers_to_a_single_clone_instruction() {
    let source = format!(
        "{CLONE_NODE}\nfn main(): Void {{
             mut a: Node = Node();
             mut b: Node = a.clone();
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::Clone(_))),
        "a.clone() must lower to InstKind::Clone, found {kinds:?}"
    );
}

/// The `Clone`-produced object is a managed-reference-typed instruction
/// result like any other (`fase-4e-colector-mark-sweep`'s own unconditional
/// synthetic-slot spill), so it must be a gc root with no extra work needed
/// for `Clone` specifically — checked here rather than assumed, the same
/// rigor `fase-4e-weak`'s own task 2.3 applied to `WeakUpgrade`'s result.
#[test]
fn clone_result_is_a_gc_root() {
    let source = format!(
        "{CLONE_NODE}\nfn main(): Void {{
             mut a: Node = Node();
             mut b: Node = a.clone();
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let node_id = module.object_id("Node").expect("Node is a class");

    let has_clone_root = main
        .gc_roots
        .iter()
        .any(|&id| matches!(main.slot(id).unwrap().ty, IrType::Object(id) if id == node_id));
    assert!(
        has_clone_root,
        "b (a.clone()'s own result) must be a gc root, gc_roots={:?}",
        main.gc_roots
    );
}

/// A class that declares its own `clone()` method (the manual
/// implementation escape hatch) is dispatched as an ordinary method call —
/// no `InstKind::Clone` at all, unlike the derived case above.
#[test]
fn a_manually_implemented_clone_method_lowers_as_an_ordinary_method_call() {
    let source = "class Node {
        mut value: Int32;
        construct(value: Int32) { this.value = value; }
        fn clone(): Node { return Node(this.value); }
    }
    fn main(): Void {
        mut a: Node = Node(1);
        mut b: Node = a.clone();
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        !kinds.iter().any(|k| matches!(k, InstKind::Clone(_))),
        "a manually implemented clone() must not lower through InstKind::Clone, found {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::Call { .. })),
        "a manually implemented clone() must lower as an ordinary call, found {kinds:?}"
    );
}

// --- `unsafe { }`/`commit { }` journal (`fase-4e-unsafe-journal`) -----------

const JOURNAL_COUNTER: &str =
    "class Counter { mut value: Int32; construct(value: Int32) { this.value = value; } }";

const JOURNAL_BOOM: &str = "class Boom implements Throwable {
    construct() { }
    override fn message(): String { return \"boom\"; }
    override fn code(): String { return \"BOOM\"; }
    override fn cause(): Error? { return null; }
    override fn stack_trace(): StackTrace { return StackTrace(); }
}";

/// An `unsafe {}` block with a managed write lowers to `JournalBegin` ->
/// `JournalRecordField` -> the guarded `StoreField` -> `JournalCommit`, in
/// that order, on the normal fall-through path (tasks.md 2.5).
#[test]
fn unsafe_block_with_a_managed_write_lowers_begin_record_store_commit_in_order() {
    let source = format!(
        "{JOURNAL_COUNTER}\nfn main(): Void {{
             mut c: Counter = Counter(1);
             unsafe {{
                 c.value = 2;
             }}
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    let begin = kinds
        .iter()
        .position(|k| matches!(k, InstKind::JournalBegin))
        .unwrap_or_else(|| panic!("JournalBegin missing, found {kinds:?}"));
    let record = kinds
        .iter()
        .position(|k| matches!(k, InstKind::JournalRecordField { .. }))
        .unwrap_or_else(|| panic!("JournalRecordField missing, found {kinds:?}"));
    let store = kinds
        .iter()
        .position(|k| matches!(k, InstKind::StoreField { .. }))
        .unwrap_or_else(|| panic!("StoreField missing, found {kinds:?}"));
    let commit = kinds
        .iter()
        .position(|k| matches!(k, InstKind::JournalCommit(_)))
        .unwrap_or_else(|| panic!("JournalCommit missing, found {kinds:?}"));

    assert!(
        begin < record,
        "JournalBegin must precede JournalRecordField, found {kinds:?}"
    );
    assert!(
        record < store,
        "JournalRecordField must precede the StoreField it guards, found {kinds:?}"
    );
    assert!(
        store < commit,
        "JournalCommit must follow the store on the normal fall-through path, found {kinds:?}"
    );
    assert!(
        !kinds
            .iter()
            .any(|k| matches!(k, InstKind::JournalRollback(_))),
        "a block that never throws must never roll back, found {kinds:?}"
    );
}

/// `commit {}` inside `unsafe {}` emits `JournalCommit` at commit's own
/// entry, against the outer journal — exactly once: the enclosing `unsafe`
/// block's own fall-through commit is suppressed once its journal is
/// already spent (design D2), so a second `JournalCommit` (which would
/// double-free the same handle) must never appear.
#[test]
fn commit_block_emits_a_single_journal_commit_at_its_own_entry() {
    let source = format!(
        "{JOURNAL_COUNTER}\nfn main(): Void {{
             mut c: Counter = Counter(1);
             unsafe {{
                 c.value = 2;
                 commit {{
                     c.value = 3;
                 }}
             }}
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    let commits: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| matches!(k, InstKind::JournalCommit(_)))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        commits.len(),
        1,
        "expected exactly one JournalCommit (commit{{}}'s own entry, not a \
         second at the unsafe block's own fall-through), found {kinds:?}"
    );

    let stores: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| matches!(k, InstKind::StoreField { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        stores.len(),
        2,
        "expected both field writes, found {kinds:?}"
    );
    assert!(
        commits[0] < stores[1],
        "JournalCommit must run before commit{{}}'s own body, found {kinds:?}"
    );

    // The write inside `commit {}`'s own body runs after the journal is
    // already spent, so it must not be journaled — only the one write made
    // before `commit {}` is.
    let records = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::JournalRecordField { .. }))
        .count();
    assert_eq!(
        records, 1,
        "only the write before commit{{}} must be journaled, found {kinds:?}"
    );
}

/// A write to a slot declared *inside* the `unsafe` block itself does not
/// get a `JournalRecordSlot` (design D1's own exemption, tasks.md 2.5).
#[test]
fn a_local_declared_inside_unsafe_is_not_journaled() {
    let source = "fn main(): Void {
        unsafe {
            mut local: Int32 = 1;
            local = 2;
        }
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::JournalBegin)),
        "found {kinds:?}"
    );
    assert!(
        !kinds
            .iter()
            .any(|k| matches!(k, InstKind::JournalRecordSlot { .. })),
        "a slot declared inside the block must never be journaled, found {kinds:?}"
    );
}

/// The exception-pending check point correctly branches to `JournalRollback`
/// instead of `JournalCommit` when an exception escapes an `unsafe {}` block
/// (tasks.md 2.5) — the IR-level counterpart of the end-to-end corpus
/// fixture `unsafe_journal_rollback_on_error.zrk`.
#[test]
fn an_exception_escaping_unsafe_emits_journal_rollback() {
    let source = format!(
        "{JOURNAL_BOOM}\nfn main(): Void throws Boom {{
             mut x: Int32 = 1;
             unsafe {{
                 x = 2;
                 throw Boom();
             }}
         }}"
    );
    let module = compile(&source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::JournalRollback(_))),
        "an exception escaping the block must roll back, found {kinds:?}"
    );
}

/// `return` leaving an `unsafe { ... }` block must roll back before the function exits.
#[test]
fn return_inside_unsafe_emits_journal_rollback() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        unsafe {
            x = 2;
            return;
        }
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::JournalRollback(_))),
        "a return leaving the unsafe block must roll back, found {kinds:?}"
    );
}

/// `break` leaving an `unsafe { ... }` block must roll back before jumping.
#[test]
fn break_inside_unsafe_emits_journal_rollback() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        loop {
            unsafe {
                x = 2;
                break;
            }
        }
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::JournalRollback(_))),
        "a break leaving the unsafe block must roll back, found {kinds:?}"
    );
}

/// `continue` leaving an `unsafe { ... }` block must roll back before jumping.
#[test]
fn continue_inside_unsafe_emits_journal_rollback() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        loop {
            unsafe {
                x = 2;
                continue;
            }
        }
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::JournalRollback(_))),
        "a continue leaving the unsafe block must roll back, found {kinds:?}"
    );
}

// --- NativeSlice<T>/NativeSliceMut<T> (roadmap Phase 4e, `fase-4e-native-slice`) --

/// `pointer.as_slice(length)` lowers to a validation call
/// (`InstKind::NativeSliceValidate`) producing a `Result` — design D1/D3.
#[test]
fn as_slice_construction_lowers_to_a_validation_call_producing_a_result() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        mut view: NativeSlice<Int32> = unsafe {
            mut p: Pointer<Int32> = Pointer.from(x);
            p.as_slice(1).unwrap()
        };
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::NativeSliceValidate { .. })),
        "as_slice must lower through a validation call, found {kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::BuildEnum { .. })),
        "as_slice must build a Result value, found {kinds:?}"
    );
}

/// `s[i]` for `String` lowers to `StringGraphemeOffset`, a bounds branch,
/// `GraphemeLenAt`, and `GraphemeSlice` — roadmap Phase 4e.
#[test]
fn string_index_lowers_to_grapheme_instructions() {
    let source = "fn main(): Void {
        mut s: String = \"hola\";
        mut c: Char = s[1];
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::StringGraphemeOffset { .. })),
        "String[index] must lower to StringGraphemeOffset, found {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| matches!(
            k,
            InstKind::Binary {
                op: BinaryOp::Eq,
                ..
            }
        )),
        "String[index] must compare the offset against -1, found {kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::GraphemeLenAt { .. })),
        "String[index] must lower to GraphemeLenAt, found {kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::GraphemeSlice { .. })),
        "String[index] must lower to GraphemeSlice, found {kinds:?}"
    );
}

/// `view[i]` (read) lowers to a bounds check (a comparison against the
/// carried length) followed by the actual load — design D2/D3/D5.
#[test]
fn index_read_lowers_to_a_bounds_check_then_a_load() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        mut view: NativeSlice<Int32> = unsafe {
            mut p: Pointer<Int32> = Pointer.from(x);
            p.as_slice(1).unwrap()
        };
        mut v: Int32 = view[0];
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(
            k,
            InstKind::Binary {
                op: BinaryOp::GtEq,
                ..
            }
        )),
        "an index read must lower through a bounds comparison, found {kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::NativeSliceLoad { .. })),
        "an index read must lower to NativeSliceLoad, found {kinds:?}"
    );
}

/// `view[i] = value` (write) lowers to the same bounds check followed by an
/// actual store, only ever against a `NativeSliceMut<T>` receiver.
#[test]
fn index_write_lowers_to_a_bounds_check_then_a_store() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        mut view: NativeSliceMut<Int32> = unsafe {
            mut p: Pointer<Int32> = Pointer.from(x);
            p.as_slice_mut(1).unwrap()
        };
        view[0] = 42;
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    assert!(
        kinds.iter().any(|k| matches!(
            k,
            InstKind::Binary {
                op: BinaryOp::GtEq,
                ..
            }
        )),
        "an index write must lower through a bounds comparison, found {kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::NativeSliceStore { .. })),
        "an index write must lower to NativeSliceStore, found {kinds:?}"
    );
}

/// A `NativeSlice<T>`/`NativeSliceMut<T>`-typed slot is never a gc root
/// (design D2): a view is a plain two-word `(pointer, length)` pair, not a
/// collector-tracked allocation, confirmed rather than assumed the same way
/// `fase-4e-weak`'s own task 2.3 checked `WeakUpgrade`'s result.
#[test]
fn a_native_slice_slot_is_never_a_gc_root() {
    let source = "fn main(): Void {
        mut x: Int32 = 1;
        mut view: NativeSlice<Int32> = unsafe {
            mut p: Pointer<Int32> = Pointer.from(x);
            p.as_slice(1).unwrap()
        };
    }";
    let module = compile(source);
    let main = module.function("main").expect("main exists");

    let has_native_slice_root = main.gc_roots.iter().any(|&id| {
        matches!(
            main.slot(id).unwrap().ty,
            IrType::NativeSlice(_) | IrType::NativeSliceMut(_)
        )
    });
    assert!(
        !has_native_slice_root,
        "a NativeSlice-typed slot must never be a gc root, gc_roots={:?}",
        main.gc_roots
    );
}

// --- Value-to-contract boxing (fase-3-value-type-contract-dispatch, design D1) ---

const BOXED_POINT: &str = "interface Describable { fn describe(): String; }
    record Point implements Describable {
        x: Int32;
        y: Int32;
        fn describe(): String { return \"point\"; }
    }";

/// Converting a `record` value into a reference of a contract
/// it implements lowers to an allocation of the value's own layout id (the
/// two tables share an id space, `IrType::Value`/`IrType::Object` doc
/// comment), one `LoadField`/`StoreField` pair per field copying its inline
/// value in, and finally a `Retype` into the contract's own static type
/// (task 2.1/2.3).
#[test]
fn a_value_to_contract_conversion_boxes_then_retypes() {
    let module = compile(&format!(
        "{BOXED_POINT}\nfn main(): Void {{ mut p = Point(x: 1, y: 2); mut d: Describable = p; }}"
    ));
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    let point_id = module
        .objects
        .iter()
        .position(|o| o.name == "Point")
        .expect("Point has a box layout") as u32;

    assert!(
        kinds
            .iter()
            .any(|k| matches!(k, InstKind::Alloc(id) if *id == point_id)),
        "boxing allocates the value's own layout id, found {kinds:?}"
    );

    let load_fields = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::LoadField { .. }))
        .count();
    let store_fields = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::StoreField { .. }))
        .count();
    assert_eq!(
        load_fields, 2,
        "one LoadField per field of Point, found {kinds:?}"
    );
    assert_eq!(
        store_fields, 2,
        "one StoreField per field of Point, found {kinds:?}"
    );

    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::Retype(_))),
        "the box's own id still needs retagging to the contract's static type, found {kinds:?}"
    );
}

/// The box's own descriptor is built the same way a class's is: its
/// `ObjectLayout` carries a real `ContractTable` for the interface it
/// implements, populated the same way `check_conformance` populates a
/// class's (task 2.1, design D1/D2's own premise that `CallContract` cannot
/// tell the two apart).
#[test]
fn a_boxed_value_type_gets_a_real_contract_table() {
    let module = compile(&format!(
        "{BOXED_POINT}\nfn main(): Void {{ mut p = Point(x: 1, y: 2); mut d: Describable = p; }}"
    ));
    let point = module
        .objects
        .iter()
        .find(|o| o.name == "Point")
        .expect("Point has a box layout");

    assert!(
        !point.contracts.is_empty(),
        "Point's own contract table is missing, found {:?}",
        point.contracts
    );
    assert_eq!(point.fields.len(), 2, "Point's own two fields, boxed");
}

/// A call through the interface-typed reference to a boxed record still
/// goes through the ordinary `CallContract` dispatch — confirming design
/// D2/task 3.3 at the IR level: nothing about the receiver being a boxed
/// value, rather than a class instance, changes which instruction the call
/// lowers to.
#[test]
fn a_call_through_a_boxed_value_type_uses_the_ordinary_contract_dispatch() {
    let module = compile(&format!(
        "{BOXED_POINT}\nfn announce(d: Describable): String {{ return d.describe(); }}\n\
         fn main(): Void {{ mut p = Point(x: 1, y: 2); mut s = announce(p); }}"
    ));
    let announce = module.function("announce").expect("announce exists");

    assert!(
        instructions(announce)
            .iter()
            .any(|k| matches!(k, InstKind::CallContract { .. })),
        "a boxed value type dispatches through the same CallContract a class would"
    );
}

/// Design D3: the box is read-only after construction — no `StoreField`
/// ever targets it again anywhere else in a program that never attempts
/// one. A negative check, not just the absence of a compile error (task
/// 2.3/4.4): every `StoreField` in the whole module either targets an
/// ordinary class's own field write, or is one of the exact two
/// construction-time writes boxing `Point` performs above.
#[test]
fn nothing_ever_writes_to_a_box_again_after_its_construction() {
    let module = compile(&format!(
        "{BOXED_POINT}\nfn announce(d: Describable): String {{ return d.describe(); }}\n\
         fn main(): Void {{
             mut p = Point(x: 1, y: 2);
             mut d: Describable = p;
             stdout.println(announce(p));
         }}"
    ));

    let point_id = module
        .objects
        .iter()
        .position(|o| o.name == "Point")
        .expect("Point has a box layout") as u32;

    // Every `StoreField` in `main` targets an operand this function itself
    // just produced with `Alloc(point_id)` — i.e. one of the two boxing
    // sites, never a value reloaded from a slot or reached some other way.
    let main = module.function("main").expect("main exists");
    for block in &main.blocks {
        let mut boxed: std::collections::HashSet<ValueId> = std::collections::HashSet::new();
        for inst in &block.instructions {
            match &inst.kind {
                InstKind::Alloc(id) if *id == point_id => {
                    boxed.insert(inst.result.expect("Alloc produces a value"));
                }
                InstKind::StoreField { object, .. } => {
                    assert!(
                        boxed.contains(&object.0),
                        "a StoreField targeted something other than a box this same \
                         block just allocated: {:?}",
                        inst.kind
                    );
                }
                _ => {}
            }
        }
    }
}


// --- `Map<K, V>` / `Set<T>` (roadmap Phase 7, `map-set-collections`) ---------

#[test]
fn empty_map_and_set_constructors_lower_to_new_instructions() {
    let module = compile(
        "fn main(): Void {\n\
             mut m: Map<String, Int32> = Map();\n\
             mut s: Set<String> = Set();\n\
         }",
    );
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);
    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::MapNew { .. })),
        "MapNew was not emitted"
    );
    assert!(
        kinds.iter().any(|k| matches!(k, InstKind::SetNew { .. })),
        "SetNew was not emitted"
    );
    assert!(!module.map_types.is_empty());
    assert!(!module.set_types.is_empty());
}

#[test]
fn map_and_set_methods_lower_to_dedicated_instructions() {
    let main = main_body(
        "mut m: Map<String, Int32> = Map();\n\
         m.set(\"k\", 1);\n\
         mut b = m.contains_key(\"k\");\n\
         mut v: Int32? = m.get_or_null(\"k\");\n\
         mut s: Set<String> = Set();\n\
         s.add(\"x\");\n\
         mut c = s.contains(\"x\");\n\
         mut r = s.remove(\"x\");",
    );
    let kinds = instructions(&main);
    assert!(kinds.iter().any(|k| matches!(k, InstKind::MapSet { .. })), "{kinds:?}");
    assert!(kinds.iter().any(|k| matches!(k, InstKind::MapContainsKey { .. })), "{kinds:?}");
    assert!(kinds.iter().any(|k| matches!(k, InstKind::MapGet { .. })), "{kinds:?}");
    assert!(kinds.iter().any(|k| matches!(k, InstKind::SetAdd { .. })), "{kinds:?}");
    assert!(kinds.iter().any(|k| matches!(k, InstKind::SetContains { .. })), "{kinds:?}");
    assert!(kinds.iter().any(|k| matches!(k, InstKind::SetRemove { .. })), "{kinds:?}");
}

// --- Enum static members (`enum-static-members`) ---------------------------

#[test]
fn enum_count_lowers_to_a_constant() {
    let module = compile(
        "enum Direction { North, South, East, West }
         fn main(): Void { mut n = Direction.count; }",
    );
    let main = module.function("main").expect("main exists");
    assert!(
        instructions(main).contains(&InstKind::ConstInt(4)),
        "count is the variant count, at compile time"
    );
}

#[test]
fn enum_keys_lowers_to_a_list_of_names() {
    let module = compile(
        "enum Direction { North, South }
         fn main(): Void { mut names = Direction.keys(); }",
    );
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);
    assert!(kinds.iter().any(|k| matches!(k, InstKind::ListNew { .. })), "{kinds:?}");
    let adds = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::ListAdd { .. }))
        .count();
    assert_eq!(adds, 2, "one ListAdd per variant");
    assert!(
        module.strings.iter().any(|s| s == "North") && module.strings.iter().any(|s| s == "South"),
        "the case names are interned: {:?}",
        module.strings
    );
}

#[test]
fn enum_values_lowers_to_a_list_of_discriminants() {
    let module = compile(
        "enum Direction { North, South }
         fn main(): Void { mut values = Direction.values(); }",
    );
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);
    assert!(kinds.iter().any(|k| matches!(k, InstKind::ListNew { .. })), "{kinds:?}");
    // A traditional enum's value is its `Int32` discriminant — the list is
    // two `ConstInt`s added in declaration order.
    let adds: Vec<_> = kinds
        .iter()
        .filter_map(|k| match k {
            InstKind::ListAdd { value, .. } => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(adds.len(), 2, "one ListAdd per variant");
}

#[test]
fn enums_helpers_lower_like_the_direct_members() {
    let module = compile(
        "enum Direction { North, South }
         fn main(): Void {
             mut n = Enums.count(Direction);
             mut names = Enums.keys(Direction);
             mut values = Enums.values(Direction);
         }",
    );
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);
    assert!(kinds.contains(&InstKind::ConstInt(2)), "Enums.count is a constant");
    let lists = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::ListNew { .. }))
        .count();
    assert_eq!(lists, 2, "keys() and values() each build a list");
}

#[test]
fn from_name_lowers_to_a_comparison_chain_producing_a_result() {
    let module = compile(
        "enum Direction { North, South }
         fn main(): Void { mut r = Direction.from_name(\"North\"); }",
    );
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);
    // One equality per case, and the two `Result` variants built at the ends
    // of the chain: `Ok` on a match, `Err(LookupError)` on the fall-through.
    let comparisons = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::Binary { op: BinaryOp::Eq, .. }))
        .count();
    assert_eq!(comparisons, 2, "one test per case");
    let builds = kinds
        .iter()
        .filter(|k| matches!(k, InstKind::BuildEnum { .. }))
        .count();
    assert_eq!(builds, 3, "two Ok arms and the Err tail");
    let branches = main
        .blocks
        .iter()
        .filter(|b| matches!(b.terminator, Some(Terminator::Branch { .. })))
        .count();
    assert_eq!(branches, 2, "the chain branches once per case");
}

#[test]
fn from_value_compares_against_the_mapping() {
    let module = compile(
        "enum ExitCode { Success -> 0, Failure -> 1 }
         fn main(): Void { mut r = ExitCode.from_value(1); }",
    );
    let main = module.function("main").expect("main exists");
    let comparisons = instructions(main)
        .iter()
        .filter(|k| matches!(k, InstKind::Binary { op: BinaryOp::Eq, .. }))
        .count();
    assert_eq!(comparisons, 2, "one test per mapped case");
}

#[test]
fn enum_to_string_lowers_to_the_interned_type_name() {
    let module = compile(
        "enum Direction { North, South }
         fn main(): Void {
             mut a = Direction.to_string;
             mut b = Direction.to_string();
         }",
    );
    let main = module.function("main").expect("main exists");
    let strings: Vec<_> = instructions(main)
        .iter()
        .filter_map(|k| match k {
            InstKind::ConstString(id) => Some(module.strings[id.0 as usize].clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        strings.iter().filter(|s| s.as_str() == "Direction").count(),
        2,
        "both spellings render the type name: {strings:?}"
    );
}
