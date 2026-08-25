//! Verifier tests.
//!
//! The lowering tests assert that well-formed IR passes. These assert the other
//! half: that malformed IR is **rejected**. Without them the verifier could be
//! a no-op and every other test would still pass.
//!
//! The IR here is built by hand, on purpose: the point is to produce exactly
//! the defect being tested, which a correct lowering would never emit.

use zirk_diagnostics::Span;
use zirk_ir::*;

const S: Span = Span::new(0, 1);

/// A module with one function, built from the given blocks.
fn module_with(blocks: Vec<Block>, return_type: IrType, slots: Vec<Slot>) -> Module {
    Module {
        functions: vec![Function {
            name: "f".into(),
            params: Vec::new(),
            return_type,
            slots,
            blocks,
            entry: BlockId(0),
            span: S,
            gc_roots: Vec::new(),
        }],
        strings: Vec::new(),
        closures: Vec::new(),
        objects: Vec::new(),
        values: Vec::new(),
        enums: Vec::new(),
        pointer_types: Vec::new(),
        weak_types: Vec::new(),
        externs: Vec::new(),
    }
}

fn value(id: u32, kind: InstKind, ty: IrType) -> Instruction {
    Instruction {
        result: Some(ValueId(id)),
        kind,
        ty,
        span: S,
    }
}

fn errors_of(module: &Module) -> Vec<String> {
    match verify(module) {
        Ok(()) => panic!("the verifier accepted malformed IR"),
        Err(errors) => errors.iter().map(|e| e.detail.clone()).collect(),
    }
}

// --- Baseline ---------------------------------------------------------------

#[test]
fn well_formed_ir_is_accepted() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    assert!(verify(&module).is_ok());
}

// --- Terminators ------------------------------------------------------------

#[test]
fn a_block_without_a_terminator_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: None,
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("no terminator")),
        "{errors:?}"
    );
}

#[test]
fn a_jump_to_a_nonexistent_block_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Jump(BlockId(99))),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("does not exist")),
        "{errors:?}"
    );
}

#[test]
fn a_missing_entry_block_is_rejected() {
    let mut module = module_with(
        vec![Block {
            id: BlockId(7),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    module.functions[0].entry = BlockId(0);

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("entry block")),
        "{errors:?}"
    );
}

// --- Values -----------------------------------------------------------------

#[test]
fn using_an_undefined_value_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![Instruction {
                result: None,
                kind: InstKind::Println(Operand(ValueId(42))),
                ty: IrType::Void,
                span: S,
            }],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("undefined value")),
        "{errors:?}"
    );
}

#[test]
fn using_a_value_before_defining_it_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                Instruction {
                    result: None,
                    kind: InstKind::Println(Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: S,
                },
                value(0, InstKind::ConstInt(1), IrType::Int(IntWidth::I32)),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("before it is defined")),
        "{errors:?}"
    );
}

#[test]
fn a_value_crossing_blocks_is_rejected() {
    // In this design locals travel through slots, which is what allows skipping
    // SSA and phi nodes. A value crossing blocks would break that invariant.
    let module = module_with(
        vec![
            Block {
                id: BlockId(0),
                instructions: vec![value(0, InstKind::ConstInt(1), IrType::Int(IntWidth::I32))],
                terminator: Some(Terminator::Jump(BlockId(1))),
            },
            Block {
                id: BlockId(1),
                instructions: vec![Instruction {
                    result: None,
                    kind: InstKind::Println(Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: S,
                }],
                terminator: Some(Terminator::Return(None)),
            },
        ],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("another block")),
        "{errors:?}"
    );
}

#[test]
fn defining_the_same_value_twice_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::ConstInt(1), IrType::Int(IntWidth::I32)),
                value(0, InstKind::ConstInt(2), IrType::Int(IntWidth::I32)),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("defined twice")),
        "{errors:?}"
    );
}

// --- Types ------------------------------------------------------------------

#[test]
fn an_instruction_declaring_the_wrong_type_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![value(0, InstKind::ConstInt(1), IrType::Boolean)],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(errors.iter().any(|e| e.contains("ConstInt")), "{errors:?}");
}

#[test]
fn a_binary_operation_between_different_types_is_rejected() {
    let module = Module {
        functions: vec![Function {
            name: "f".into(),
            params: Vec::new(),
            return_type: IrType::Void,
            slots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    value(0, InstKind::ConstInt(1), IrType::Int(IntWidth::I32)),
                    value(1, InstKind::ConstString(StringId(0)), IrType::String),
                    value(
                        2,
                        InstKind::Binary {
                            op: BinaryOp::Add,
                            left: Operand(ValueId(0)),
                            right: Operand(ValueId(1)),
                        },
                        IrType::Int(IntWidth::I32),
                    ),
                ],
                terminator: Some(Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span: S,
            gc_roots: Vec::new(),
        }],
        strings: vec!["x".into()],
        closures: Vec::new(),
        objects: Vec::new(),
        values: Vec::new(),
        enums: Vec::new(),
        pointer_types: Vec::new(),
        weak_types: Vec::new(),
        externs: Vec::new(),
    };

    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("Int32") && e.contains("String")),
        "{errors:?}"
    );
}

#[test]
fn branching_on_a_non_boolean_is_rejected() {
    let module = module_with(
        vec![
            Block {
                id: BlockId(0),
                instructions: vec![value(0, InstKind::ConstInt(1), IrType::Int(IntWidth::I32))],
                terminator: Some(Terminator::Branch {
                    condition: Operand(ValueId(0)),
                    then_block: BlockId(1),
                    else_block: BlockId(1),
                }),
            },
            Block {
                id: BlockId(1),
                instructions: vec![],
                terminator: Some(Terminator::Return(None)),
            },
        ],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("expected Boolean")),
        "{errors:?}"
    );
}

#[test]
fn returning_the_wrong_type_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![value(0, InstKind::ConstInt(1), IrType::Int(IntWidth::I32))],
            terminator: Some(Terminator::Return(Some(Operand(ValueId(0))))),
        }],
        IrType::Boolean,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("declared Boolean")),
        "{errors:?}"
    );
}

#[test]
fn println_over_a_non_string_is_rejected() {
    // This is the defect that reached runtime: the value was read as a pointer
    // to an arbitrary address.
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::ConstInt(3), IrType::Int(IntWidth::I32)),
                Instruction {
                    result: None,
                    kind: InstKind::Println(Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("expected String")),
        "{errors:?}"
    );
}

#[test]
fn a_conversion_that_does_not_produce_a_string_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::ConstInt(3), IrType::Int(IntWidth::I32)),
                value(
                    1,
                    InstKind::ToString(Operand(ValueId(0))),
                    IrType::Int(IntWidth::I32),
                ),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(errors.iter().any(|e| e.contains("ToString")), "{errors:?}");
}

// --- Slots ------------------------------------------------------------------

#[test]
fn reading_a_nonexistent_slot_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![value(
                0,
                InstKind::Load(SlotId(9)),
                IrType::Int(IntWidth::I32),
            )],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("does not exist")),
        "{errors:?}"
    );
}

#[test]
fn storing_the_wrong_type_into_a_slot_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::ConstBool(true), IrType::Boolean),
                Instruction {
                    result: None,
                    kind: InstKind::Store(SlotId(0), Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![Slot {
            name: "x".into(),
            ty: IrType::Int(IntWidth::I32),
            span: S,
        }],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("stores Boolean")),
        "{errors:?}"
    );
}

// --- Calls ------------------------------------------------------------------

#[test]
fn calling_a_nonexistent_function_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![value(
                0,
                InstKind::Call {
                    callee: "missing".into(),
                    args: vec![],
                },
                IrType::Void,
            )],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("does not exist")),
        "{errors:?}"
    );
}

#[test]
fn a_string_outside_the_module_table_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![value(0, InstKind::ConstString(StringId(5)), IrType::String)],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|e| e.contains("module table")),
        "{errors:?}"
    );
}
