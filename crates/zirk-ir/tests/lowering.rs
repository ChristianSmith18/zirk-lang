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
    // Negation goes through a variable: `-1` is one literal, not a negation
    // applied to one, which is what lets `-2147483648` be written at all.
    let f = main_body("mut n: Int32 = 1;\nmut a: Int32 = -n;\nmut b: Boolean = !true;");

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

    assert_eq!(module.strings, vec!["Hola desde Zirk".to_string()]);
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
    let layout = &module.objects[0];

    assert_eq!(layout.name, "User");
    assert_eq!(layout.fields.len(), 2);
    assert_eq!(layout.fields[0].name, "id");
    assert_eq!(layout.fields[0].ty, IrType::Int32);
    assert_eq!(layout.fields[1].ty, IrType::String);
}

#[test]
fn a_constructor_becomes_a_function_over_the_object() {
    let module = with_class(USER, "");
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
        Some(IrType::Object(0))
    );
    assert_eq!(constructor.return_type, IrType::Void);
}

#[test]
fn building_an_object_allocates_and_then_calls_its_constructor() {
    let module = with_class(USER, "mut u = User(1, \"x\");");
    let main = module.function("main").expect("main exists");
    let kinds = instructions(main);

    let alloc = kinds
        .iter()
        .position(|k| matches!(k, InstKind::Alloc(0)))
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
