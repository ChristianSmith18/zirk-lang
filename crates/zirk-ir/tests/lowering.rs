//! Lowering tests.
//!
//! Every construct of the subset is checked twice: that the IR produced has the
//! expected shape, and that it passes the verifier. The second check is what
//! catches structural bugs the first one would let through.

use zirk_diagnostics::{DiagnosticSink, RenderStyle, SourceFile};
use zirk_ir::*;
use zirk_lexer::tokenize;
use zirk_parser::parse;
use zirk_sema::check;

/// Runs the full frontend and lowers, asserting the IR is well formed.
fn compile(source_text: &str) -> Module {
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let program = parse(&source, &tokens, &mut sink);
    let checked = check(&source, &program, &mut sink);

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
    assert_eq!(f.slot(f.params[1]).map(|s| s.ty), Some(IrType::Int32));
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
    assert_eq!(module.strings, vec!["hola".to_string()]);
}

#[test]
fn a_repeated_literal_is_interned_once() {
    let module = compile("fn main(): Void {\nmut a: String = \"x\";\nmut b: String = \"x\";\n}");
    assert_eq!(module.strings.len(), 1);
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
fn shadowing_produces_two_different_slots() {
    let f = main_body("mut x: Int32 = 1;\n{ mut x: String = \"a\"; }");

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
    let f = main_body("mut a: Int32 = -1;\nmut b: Boolean = !true;");

    let unaries: Vec<_> = f
        .blocks
        .iter()
        .flat_map(|b| &b.instructions)
        .filter(|i| matches!(i.kind, InstKind::Unary { .. }))
        .collect();

    assert_eq!(unaries.len(), 2);
    assert!(unaries.iter().any(|i| i.ty == IrType::Int32));
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
    let source = SourceFile::new("test.zrk", source_text);
    let mut sink = DiagnosticSink::new();
    let tokens = tokenize(&source, &mut sink);
    let program = parse(&source, &tokens, &mut sink);
    let checked = check(&source, &program, &mut sink);
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

    assert_eq!(module.strings, vec!["Hola desde Zirk".to_string()]);
    let main = module.function("main").expect("main exists");
    assert_eq!(main.return_type, IrType::Void);
    assert!(
        instructions(main)
            .iter()
            .any(|i| matches!(i, InstKind::Println(_)))
    );
}
