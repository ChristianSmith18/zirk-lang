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
        capability_requesters: std::collections::HashMap::new(),
        project_globals: Vec::new(),
        strings: Vec::new(),
        closures: Vec::new(),
        objects: Vec::new(),
        contracts: Vec::new(),
        values: Vec::new(),
        enums: Vec::new(),
        pointer_types: Vec::new(),
        channel_types: Vec::new(),
        mutex_types: vec![IrType::Int(IntWidth::I32)],
        weak_types: Vec::new(),
        native_slice_types: Vec::new(),
        native_slice_mut_types: Vec::new(),
        dependent_types: Vec::new(),
        pin_types: Vec::new(),
        array_types: Vec::new(),
        list_types: Vec::new(),
        map_types: Vec::new(),
        set_types: Vec::new(),
        externs: Vec::new(),
    }
}

fn module_with_mutex(blocks: Vec<Block>, return_type: IrType, mut slots: Vec<Slot>) -> Module {
    let handle_slot = slots.len() as u32;
    slots.push(Slot {
        name: "mutex".into(),
        ty: IrType::Mutex(0),
        span: S,
    });
    let mut module = module_with(blocks, return_type, slots);
    module.functions[0].gc_roots.push(SlotId(handle_slot));
    module
}

fn value(id: u32, kind: InstKind, ty: IrType) -> Instruction {
    Instruction {
        result: Some(ValueId(id)),
        kind,
        ty,
        span: S,
    }
}

fn borrowed_view_module(mut instructions: Vec<Instruction>, return_type: IrType) -> Module {
    if instructions.first().is_some_and(|instruction| {
        matches!(
            (&instruction.kind, instruction.result, instruction.ty),
            (InstKind::Undefined, Some(ValueId(0)), IrType::Mutex(0))
        )
    }) {
        let temporary = instructions
            .iter()
            .filter_map(|instruction| instruction.result.map(|value| value.0))
            .max()
            .unwrap_or(0)
            + 1;
        instructions[0] = value(0, InstKind::Load(SlotId(0)), IrType::Mutex(0));
        instructions.splice(
            0..0,
            [
                value(temporary, InstKind::Undefined, IrType::Mutex(0)),
                mutex_op(InstKind::Store(SlotId(0), Operand(ValueId(temporary)))),
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(99),
                    mutex: SlotId(0),
                }),
            ],
        );
        instructions.push(mutex_op(InstKind::MutexRelease {
            id: MutexId(99),
            mutex: SlotId(0),
        }));
    }
    module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions,
            terminator: Some(Terminator::Return(None)),
        }],
        return_type,
        Vec::new(),
    )
}

#[test]
fn mutex_with_call_verifies_typed_callback_and_result_slot() {
    let payload = IrType::Int(IntWidth::I32);
    let view_ty = IrType::BorrowedView {
        region: 31,
        mutex_type: 0,
    };
    let module = Module {
        functions: vec![
            Function {
                name: "caller".into(),
                params: vec![],
                return_type: IrType::Void,
                slots: vec![Slot {
                    name: "result".into(),
                    ty: payload,
                    span: S,
                }],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        value(0, InstKind::Undefined, IrType::Mutex(0)),
                        value(
                            1,
                            InstKind::MakeCallable {
                                target: "callback".into(),
                                captures: vec![],
                            },
                            IrType::Callable(0),
                        ),
                        value(
                            2,
                            InstKind::MutexWithCall {
                                mutex: Operand(ValueId(0)),
                                callback: Operand(ValueId(1)),
                                layout: 0,
                                result_slot: Some(SlotId(0)),
                            },
                            IrType::Int(IntWidth::I32),
                        ),
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: S,
                gc_roots: vec![],
            },
            Function {
                name: "callback".into(),
                params: vec![SlotId(0)],
                return_type: payload,
                slots: vec![Slot {
                    name: "view".into(),
                    ty: view_ty,
                    span: S,
                }],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        value(0, InstKind::Load(SlotId(0)), view_ty),
                        value(
                            1,
                            InstKind::BorrowViewLoad {
                                view: Operand(ValueId(0)),
                            },
                            payload,
                        ),
                    ],
                    terminator: Some(Terminator::Return(Some(Operand(ValueId(1))))),
                }],
                entry: BlockId(0),
                span: S,
                gc_roots: vec![],
            },
        ],
        closures: vec![ClosureLayout {
            captures: vec![],
            params: vec![view_ty],
            returns: payload,
        }],
        mutex_types: vec![payload],
        ..Module::default()
    };
    verify(&module).expect("dedicated callback may borrow and return the payload");
}

#[test]
fn mutex_with_call_rejects_mismatched_payload_and_ordinary_callback_call() {
    let payload = IrType::Int(IntWidth::I32);
    let view_ty = IrType::BorrowedView {
        region: 9,
        mutex_type: 0,
    };
    let mut module = Module {
        functions: vec![
            Function {
                name: "caller".into(),
                params: vec![],
                return_type: IrType::Void,
                slots: vec![],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![
                        value(0, InstKind::Undefined, IrType::Mutex(0)),
                        value(
                            1,
                            InstKind::MakeCallable {
                                target: "callback".into(),
                                captures: vec![],
                            },
                            IrType::Callable(0),
                        ),
                        value(
                            2,
                            InstKind::MutexWithCall {
                                mutex: Operand(ValueId(0)),
                                callback: Operand(ValueId(1)),
                                layout: 0,
                                result_slot: None,
                            },
                            IrType::Int(IntWidth::I32),
                        ),
                        value(
                            3,
                            InstKind::CallCallable {
                                callable: Operand(ValueId(1)),
                                args: vec![Operand(ValueId(0))],
                            },
                            IrType::Void,
                        ),
                    ],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: S,
                gc_roots: vec![],
            },
            Function {
                name: "callback".into(),
                params: vec![SlotId(0)],
                return_type: IrType::Void,
                slots: vec![Slot {
                    name: "view".into(),
                    ty: view_ty,
                    span: S,
                }],
                blocks: vec![Block {
                    id: BlockId(0),
                    instructions: vec![],
                    terminator: Some(Terminator::Return(None)),
                }],
                entry: BlockId(0),
                span: S,
                gc_roots: vec![],
            },
        ],
        closures: vec![ClosureLayout {
            captures: vec![],
            params: vec![view_ty],
            returns: IrType::Void,
        }],
        mutex_types: vec![payload],
        ..Module::default()
    };
    assert!(
        verify(&module)
            .unwrap_err()
            .iter()
            .any(|e| e.detail.contains("only be invoked by MutexWithCall"))
    );
    module.functions[0].blocks[0].instructions[2].kind = InstKind::MutexWithCall {
        mutex: Operand(ValueId(0)),
        callback: Operand(ValueId(1)),
        layout: 0,
        result_slot: Some(SlotId(42)),
    };
    assert!(
        verify(&module).is_err(),
        "Void callbacks cannot receive a result slot"
    );
}

fn borrow_start(result: Option<ValueId>, region: u32, source: u32, ty: IrType) -> Instruction {
    Instruction {
        result,
        kind: InstKind::BorrowViewStart {
            region,
            mutex: Operand(ValueId(source)),
        },
        ty,
        span: S,
    }
}

fn borrow_end(view: u32) -> Instruction {
    Instruction {
        result: None,
        kind: InstKind::BorrowViewEnd {
            view: Operand(ValueId(view)),
        },
        ty: IrType::Void,
        span: S,
    }
}

fn borrow_source() -> Instruction {
    value(0, InstKind::Undefined, IrType::Mutex(0))
}

#[test]
fn borrowed_view_region_must_open_and_close_once_in_one_block() {
    let valid = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    verify(&valid).expect("well-formed block-local borrow region");

    let unclosed = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
        ],
        IrType::Void,
    );
    assert!(
        verify(&unclosed).is_err(),
        "an open region must be rejected"
    );

    let double_close = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            borrow_end(1),
            borrow_end(1),
        ],
        IrType::Void,
    );
    assert!(
        verify(&double_close).is_err(),
        "a region cannot close twice"
    );

    let duplicate_region = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            borrow_start(
                Some(ValueId(2)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            borrow_end(1),
            borrow_end(2),
        ],
        IrType::Void,
    );
    assert!(
        verify(&duplicate_region).is_err(),
        "region ids must be unique"
    );
}

#[test]
fn mutex_new_requires_exact_payload_type_and_rooted_destination() {
    let mut module = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::ConstInt(7), IrType::Int(IntWidth::I32)),
                value(
                    1,
                    InstKind::MutexNew {
                        payload: Operand(ValueId(0)),
                        mutex_type: 0,
                        object_slot: SlotId(0),
                    },
                    IrType::Int(IntWidth::I32),
                ),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    verify(&module).expect("well-formed MutexNew");
    module.mutex_types[0] = IrType::String;
    assert!(
        verify(&module)
            .unwrap_err()
            .iter()
            .any(|e| e.detail.contains("does not exactly match"))
    );
    module.mutex_types[0] = IrType::Int(IntWidth::I32);
    module.functions[0].gc_roots.clear();
    assert!(
        verify(&module)
            .unwrap_err()
            .iter()
            .any(|e| e.detail.contains("destination must be a GC root"))
    );
    module.functions[0].gc_roots.push(SlotId(0));
    module.functions[0].blocks[0].instructions[1].kind = InstKind::MutexNew {
        payload: Operand(ValueId(0)),
        mutex_type: 0,
        object_slot: SlotId(99),
    };
    assert!(
        verify(&module)
            .unwrap_err()
            .iter()
            .any(|e| e.detail.contains("destination slot does not exist"))
    );
    module.functions[0].blocks[0].instructions[1].kind = InstKind::MutexNew {
        payload: Operand(ValueId(0)),
        mutex_type: 99,
        object_slot: SlotId(0),
    };
    assert!(
        verify(&module)
            .unwrap_err()
            .iter()
            .any(|e| e.detail.contains("valid mutex payload type"))
    );
}

#[test]
fn borrowed_value_field_paths_are_typed_and_enum_paths_are_rejected() {
    let payload = IrType::Value(0);
    let view_ty = IrType::BorrowedView {
        region: 8,
        mutex_type: 0,
    };
    let mut module = borrowed_view_module(
        vec![
            value(0, InstKind::Undefined, IrType::Mutex(0)),
            borrow_start(Some(ValueId(1)), 8, 0, view_ty),
            value(
                2,
                InstKind::BorrowViewFieldLoad {
                    view: Operand(ValueId(1)),
                    path: vec![0],
                },
                IrType::Int(IntWidth::I32),
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    module.mutex_types[0] = payload;
    module.values.push(zirk_ir::ValueLayout {
        name: "Payload".into(),
        fields: vec![zirk_ir::ObjectField {
            name: "count".into(),
            ty: IrType::Int(IntWidth::I32),
        }],
    });
    verify(&module).expect("typed Value field path");
    module.mutex_types[0] = IrType::Enum(0);
    module.enums.push(zirk_ir::EnumLayout {
        name: "Choice".into(),
        fields: vec![zirk_ir::ObjectField {
            name: "value".into(),
            ty: IrType::Int(IntWidth::I32),
        }],
        variants: vec![vec![0]],
    });
    assert!(
        verify(&module)
            .unwrap_err()
            .iter()
            .any(|e| e.detail.contains("crosses an Enum"))
    );
}

#[test]
fn borrowed_view_requires_the_same_mutex_lock_to_be_held() {
    let unlocked = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                17,
                0,
                IrType::BorrowedView {
                    region: 17,
                    mutex_type: 0,
                },
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    // This helper normally adds the matching held lock. Remove its acquire
    // and release to model malformed IR which forges a view without locking.
    let mut malformed = unlocked;
    malformed.functions[0].blocks[0]
        .instructions
        .retain(|instruction| {
            !matches!(
                instruction.kind,
                InstKind::MutexAcquire {
                    id: MutexId(99),
                    ..
                } | InstKind::MutexRelease {
                    id: MutexId(99),
                    ..
                }
            )
        });
    let errors = errors_of(&malformed);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("requires its Mutex handle")),
        "an unlocked mutex must not authorize payload access: {errors:?}"
    );
}

#[test]
fn borrowed_view_must_end_before_its_mutex_is_released() {
    let module = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                18,
                0,
                IrType::BorrowedView {
                    region: 18,
                    mutex_type: 0,
                },
            ),
            mutex_op(InstKind::MutexRelease {
                id: MutexId(99),
                mutex: SlotId(0),
            }),
            borrow_end(1),
        ],
        IrType::Void,
    );
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("while a BorrowedView is active")),
        "release cannot invalidate a live borrowed payload view: {errors:?}"
    );
}

#[test]
fn borrowed_view_start_and_end_shapes_are_verified() {
    let wrong_region_type = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 8,
                    mutex_type: 0,
                },
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    assert!(verify(&wrong_region_type).is_err());

    let missing_result = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                None,
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
        ],
        IrType::Void,
    );
    assert!(verify(&missing_result).is_err());

    let mut bad_end = borrow_end(1);
    bad_end.result = Some(ValueId(2));
    let resultful_end = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            bad_end,
        ],
        IrType::Void,
    );
    assert!(verify(&resultful_end).is_err());
}

#[test]
fn borrowed_view_rejects_calls_until_transitive_effects_are_available() {
    let module = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            Instruction {
                result: None,
                kind: InstKind::Call {
                    callee: "unknown_effect".into(),
                    args: vec![],
                },
                ty: IrType::Void,
                span: S,
            },
            borrow_end(1),
        ],
        IrType::Void,
    );
    let errors = verify(&module).expect_err("unknown callee effect must fail closed");
    assert!(errors.iter().any(|error| {
        error
            .detail
            .contains("forbidden while a BorrowedView is live")
    }));

    let lock_wait = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            mutex_op(InstKind::MutexAcquire {
                id: MutexId(1),
                mutex: SlotId(0),
            }),
            borrow_end(1),
        ],
        IrType::Void,
    );
    let errors = verify(&lock_wait).expect_err("mutex acquisition may suspend");
    assert!(errors.iter().any(|error| {
        error
            .detail
            .contains("forbidden while a BorrowedView is live")
    }));
}

#[test]
fn borrowed_view_rejects_safepoints_and_host_blocking_io() {
    let safepoint = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            Instruction {
                result: None,
                kind: InstKind::SafepointPoll,
                ty: IrType::Void,
                span: S,
            },
            borrow_end(1),
        ],
        IrType::Void,
    );
    let errors = verify(&safepoint).expect_err("a live view cannot cross a collector safepoint");
    assert!(
        errors
            .iter()
            .any(|error| { error.detail.contains("crossing a GC/runtime boundary") })
    );

    let host_io = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            Instruction {
                result: None,
                kind: InstKind::Println(Operand(ValueId(0))),
                ty: IrType::Void,
                span: S,
            },
            borrow_end(1),
        ],
        IrType::Void,
    );
    let errors = verify(&host_io).expect_err("host output may block while a view is live");
    assert!(
        errors
            .iter()
            .any(|error| error.detail.contains("performing host I/O"))
    );
}

#[test]
fn borrowed_view_allows_transitively_synchronous_direct_calls() {
    let mut module = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            Instruction {
                result: None,
                kind: InstKind::Call {
                    callee: "sync_helper".into(),
                    args: vec![],
                },
                ty: IrType::Void,
                span: S,
            },
            borrow_end(1),
        ],
        IrType::Void,
    );
    module.functions.push(Function {
        name: "sync_helper".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![Instruction {
                result: None,
                kind: InstKind::Call {
                    callee: "sync_leaf".into(),
                    args: vec![],
                },
                ty: IrType::Void,
                span: S,
            }],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span: S,
        gc_roots: vec![],
    });
    module.functions.push(Function {
        name: "sync_leaf".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span: S,
        gc_roots: vec![],
    });

    verify(&module).expect("direct calls with complete non-suspending bodies are safe");
}

#[test]
fn borrowed_view_allows_only_audited_non_safepoint_string_leaf_calls() {
    let mut module = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            value(2, InstKind::Undefined, IrType::String),
            value(
                3,
                InstKind::Call {
                    callee: "zirk_str_length".into(),
                    args: vec![Operand(ValueId(2))],
                },
                IrType::Int(IntWidth::I32),
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    module.externs.push(ExternFn {
        name: "zirk_str_length".into(),
        params: vec![IrType::String],
        return_type: IrType::Int(IntWidth::I32),
    });

    verify(&module).expect("audited pure string metadata reads do not cross a safepoint");
}

#[test]
fn borrowed_view_runtime_leaf_requires_its_exact_declared_abi() {
    let mut module = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            value(2, InstKind::Undefined, IrType::String),
            value(
                3,
                InstKind::Call {
                    callee: "zirk_str_length".into(),
                    args: vec![Operand(ValueId(2))],
                },
                IrType::Int(IntWidth::I32),
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    module.externs.push(ExternFn {
        name: "zirk_str_length".into(),
        params: vec![IrType::Boolean],
        return_type: IrType::Boolean,
    });

    let errors = verify(&module).expect_err("forged leaf signatures must fail closed");
    assert!(
        errors
            .iter()
            .any(|error| error.detail.contains("must have signature"))
    );
}

#[test]
fn borrowed_view_rejects_direct_calls_with_transitive_gc_boundary() {
    let mut module = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            Instruction {
                result: None,
                kind: InstKind::Call {
                    callee: "sync_helper".into(),
                    args: vec![],
                },
                ty: IrType::Void,
                span: S,
            },
            borrow_end(1),
        ],
        IrType::Void,
    );
    module.functions.push(Function {
        name: "sync_helper".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![Instruction {
                result: None,
                kind: InstKind::Call {
                    callee: "cycle_leaf".into(),
                    args: vec![],
                },
                ty: IrType::Void,
                span: S,
            }],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span: S,
        gc_roots: vec![],
    });
    module.functions.push(Function {
        name: "cycle_leaf".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![
                Instruction {
                    result: None,
                    kind: InstKind::SafepointPoll,
                    ty: IrType::Void,
                    span: S,
                },
                Instruction {
                    result: None,
                    kind: InstKind::Call {
                        callee: "sync_helper".into(),
                        args: vec![],
                    },
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span: S,
        gc_roots: vec![],
    });

    let errors = verify(&module)
        .expect_err("recursive transitive safepoint must not occur under a borrowed view");
    assert!(
        errors
            .iter()
            .any(|error| { error.detail.contains("crossing a GC/runtime boundary") })
    );
}

#[test]
fn borrowed_view_cannot_escape_through_slots_returns_or_instructions() {
    let store = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            Instruction {
                result: None,
                kind: InstKind::Store(SlotId(0), Operand(ValueId(1))),
                ty: IrType::Void,
                span: S,
            },
            borrow_end(1),
        ],
        IrType::Void,
    );
    assert!(verify(&store).is_err(), "a view cannot be stored");

    let retype = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            value(
                2,
                InstKind::Retype(Operand(ValueId(1))),
                IrType::Int(IntWidth::I64),
            ),
            borrow_end(1),
        ],
        IrType::Void,
    );
    assert!(verify(&retype).is_err(), "a view cannot be retyped");

    let mut slotted = borrowed_view_module(vec![], IrType::Void);
    slotted.functions[0].slots.push(Slot {
        name: "borrowed".into(),
        ty: IrType::BorrowedView {
            region: 7,
            mutex_type: 0,
        },
        span: S,
    });
    assert!(verify(&slotted).is_err(), "a view cannot occur in a slot");

    let returned = Module {
        functions: vec![Function {
            name: "returns_view".into(),
            params: Vec::new(),
            return_type: IrType::BorrowedView {
                region: 7,
                mutex_type: 0,
            },
            slots: Vec::new(),
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    borrow_source(),
                    borrow_start(
                        Some(ValueId(1)),
                        7,
                        0,
                        IrType::BorrowedView {
                            region: 7,
                            mutex_type: 0,
                        },
                    ),
                    borrow_end(1),
                ],
                terminator: Some(Terminator::Return(Some(Operand(ValueId(1))))),
            }],
            entry: BlockId(0),
            span: S,
            gc_roots: Vec::new(),
        }],
        ..borrowed_view_module(vec![], IrType::Void)
    };
    assert!(
        verify(&returned).is_err(),
        "a function cannot return a view"
    );

    let mut layout = borrowed_view_module(vec![], IrType::Void);
    layout.values.push(ValueLayout {
        name: "Escaping".into(),
        fields: vec![ObjectField {
            name: "view".into(),
            ty: IrType::BorrowedView {
                region: 7,
                mutex_type: 0,
            },
        }],
    });
    assert!(
        verify(&layout).is_err(),
        "a view cannot occur in a value layout"
    );
}

#[test]
fn borrowed_view_cannot_be_a_function_parameter() {
    let mut module = borrowed_view_module(vec![], IrType::Void);
    module.functions[0].slots.push(Slot {
        name: "borrowed_parameter".into(),
        ty: IrType::BorrowedView {
            region: 7,
            mutex_type: 0,
        },
        span: S,
    });
    module.functions[0].params.push(SlotId(0));

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|error| error.contains("BorrowedView")),
        "borrowed function parameter should be rejected: {errors:?}"
    );
}

#[test]
fn borrowed_view_cannot_be_a_closure_or_lambda_layout_parameter() {
    let mut module = borrowed_view_module(vec![], IrType::Void);
    module.closures.push(ClosureLayout {
        captures: Vec::new(),
        params: vec![IrType::BorrowedView {
            region: 7,
            mutex_type: 0,
        }],
        returns: IrType::Void,
    });

    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|error| error.contains("BorrowedView")),
        "borrowed closure/lambda parameter should be rejected: {errors:?}"
    );
}

#[test]
fn borrowed_view_cannot_be_used_after_end_or_across_blocks() {
    let after_end = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
            borrow_end(1),
            Instruction {
                result: None,
                kind: InstKind::BorrowViewEnd {
                    view: Operand(ValueId(1)),
                },
                ty: IrType::Void,
                span: S,
            },
        ],
        IrType::Void,
    );
    assert!(
        verify(&after_end).is_err(),
        "a closed view cannot be used again"
    );

    let mut cross_block = borrowed_view_module(
        vec![
            borrow_source(),
            borrow_start(
                Some(ValueId(1)),
                7,
                0,
                IrType::BorrowedView {
                    region: 7,
                    mutex_type: 0,
                },
            ),
        ],
        IrType::Void,
    );
    cross_block.functions[0].blocks[0].terminator = Some(Terminator::Jump(BlockId(1)));
    cross_block.functions[0].blocks.push(Block {
        id: BlockId(1),
        instructions: vec![borrow_end(1)],
        terminator: Some(Terminator::Return(None)),
    });
    assert!(
        verify(&cross_block).is_err(),
        "a view cannot cross a block boundary"
    );
}

fn background_start_module() -> Module {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(
                    0,
                    InstKind::MakeCallable {
                        target: "branch".into(),
                        captures: Vec::new(),
                    },
                    IrType::Callable(0),
                ),
                Instruction {
                    result: None,
                    kind: InstKind::BackgroundStart {
                        target: "branch".into(),
                        body: Operand(ValueId(0)),
                        status_slot: SlotId(0),
                    },
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![Slot {
            name: "background_status".into(),
            ty: IrType::Int(IntWidth::U32),
            span: S,
        }],
    );
    module.closures.push(ClosureLayout {
        captures: Vec::new(),
        params: Vec::new(),
        returns: IrType::Void,
    });
    module.functions.push(Function {
        name: "branch".into(),
        params: Vec::new(),
        return_type: IrType::Void,
        slots: Vec::new(),
        blocks: vec![Block {
            id: BlockId(0),
            instructions: Vec::new(),
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span: S,
        gc_roots: Vec::new(),
    });
    module
}

#[test]
fn background_start_accepts_void_branch_callable_and_u32_status() {
    verify(&background_start_module()).expect("well-formed BackgroundStart IR");
}

#[test]
fn background_start_rejects_missing_target_non_callable_bad_status_and_non_void_types() {
    let mut malformed_target = background_start_module();
    if let InstKind::BackgroundStart { target, .. } =
        &mut malformed_target.functions[0].blocks[0].instructions[1].kind
    {
        *target = "missing_branch".into();
    }
    assert!(
        errors_of(&malformed_target)
            .iter()
            .any(|error| error.contains("not a module function"))
    );

    let mut malformed_callable = background_start_module();
    malformed_callable.functions[0].blocks[0].instructions[0].ty = IrType::Int(IntWidth::U32);
    assert!(
        errors_of(&malformed_callable)
            .iter()
            .any(|error| error.contains("body is not a Callable"))
    );

    let mut malformed_status = background_start_module();
    malformed_status.functions[0].slots[0].ty = IrType::Int(IntWidth::I32);
    assert!(
        errors_of(&malformed_status)
            .iter()
            .any(|error| error.contains("status slot must be U32"))
    );

    let mut malformed_instruction_return = background_start_module();
    malformed_instruction_return.functions[0].blocks[0].instructions[1].ty =
        IrType::Int(IntWidth::U32);
    assert!(
        errors_of(&malformed_instruction_return)
            .iter()
            .any(|error| error.contains("BackgroundStart") && error.contains("Void"))
    );

    let mut malformed_target_return = background_start_module();
    malformed_target_return.functions[1].return_type = IrType::Int(IntWidth::U32);
    assert!(
        errors_of(&malformed_target_return)
            .iter()
            .any(|error| error.contains("target must return Void"))
    );
}

#[test]
fn collect_all_scope_requires_its_i64_handle_and_void_close() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(
                    0,
                    InstKind::ScopeEnterCollectAll,
                    IrType::Int(IntWidth::I64),
                ),
                Instruction {
                    result: None,
                    kind: InstKind::ScopeExitCollectAll {
                        scope: Operand(ValueId(0)),
                    },
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        Vec::new(),
    );
    verify(&module).expect("valid collect-all scope IR");

    let mut malformed = module;
    malformed.functions[0].blocks[0].instructions[0].ty = IrType::Int(IntWidth::I32);
    let errors = errors_of(&malformed);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("ScopeEnterCollectAll"))
    );
}

fn errors_of(module: &Module) -> Vec<String> {
    match verify(module) {
        Ok(()) => panic!("the verifier accepted malformed IR"),
        Err(errors) => errors.iter().map(|e| e.detail.clone()).collect(),
    }
}

fn mutex_op(kind: InstKind) -> Instruction {
    Instruction {
        result: None,
        kind,
        ty: IrType::Void,
        span: S,
    }
}

#[test]
fn mutex_cfg_verifier_rejects_unmatched_double_and_leaked_locks() {
    let duplicate = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(1),
                    mutex: SlotId(0),
                }),
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(1),
                    mutex: SlotId(0),
                }),
                mutex_op(InstKind::MutexRelease {
                    id: MutexId(1),
                    mutex: SlotId(0),
                }),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    assert!(
        errors_of(&duplicate)
            .iter()
            .any(|e| e.contains("acquired while already held"))
    );

    let release = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![mutex_op(InstKind::MutexRelease {
                id: MutexId(1),
                mutex: SlotId(0),
            })],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    assert!(
        errors_of(&release)
            .iter()
            .any(|e| e.contains("released without being held"))
    );

    let leaked = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![mutex_op(InstKind::MutexAcquire {
                id: MutexId(1),
                mutex: SlotId(0),
            })],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    assert!(
        errors_of(&leaked)
            .iter()
            .any(|e| e.contains("retains acquired mutexes"))
    );
}

#[test]
fn mutex_cfg_verifier_checks_cancel_and_throw_cleanup_edges() {
    let cancelled = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![mutex_op(InstKind::MutexAcquire {
                id: MutexId(5),
                mutex: SlotId(0),
            })],
            terminator: Some(Terminator::Cancel),
        }],
        IrType::Void,
        vec![],
    );
    assert!(errors_of(&cancelled).iter().any(|error| {
        error.contains("reachable exit") && error.contains("retains acquired mutexes")
    }));

    // Uncaught throws set a pending exception and return from the function.
    // The verifier checks that terminal edge after explicit cleanup runs.
    let mut throwing = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::Undefined, IrType::Object(0)),
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(5),
                    mutex: SlotId(0),
                }),
                Instruction {
                    result: None,
                    kind: InstKind::Throw(Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    throwing.objects.push(ObjectLayout {
        name: "Throwable".into(),
        fields: vec![],
        contracts: vec![],
        methods: vec![],
        ancestors: vec![0],
    });
    let throwing_errors = errors_of(&throwing);
    assert!(
        throwing_errors.iter().any(|error| {
            error.contains("reachable exit") && error.contains("retains acquired mutexes")
        }),
        "{throwing_errors:?}"
    );

    let mut cleaned_throw = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::Undefined, IrType::Object(0)),
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(5),
                    mutex: SlotId(0),
                }),
                mutex_op(InstKind::MutexRelease {
                    id: MutexId(5),
                    mutex: SlotId(0),
                }),
                Instruction {
                    result: None,
                    kind: InstKind::Throw(Operand(ValueId(0))),
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    cleaned_throw.objects.push(ObjectLayout {
        name: "Throwable".into(),
        fields: vec![],
        contracts: vec![],
        methods: vec![],
        ancestors: vec![0],
    });
    assert!(verify(&cleaned_throw).is_ok());
}

#[test]
fn mutex_cfg_verifier_rejects_inconsistent_branch_join_and_loop_backedge() {
    let branch = module_with_mutex(
        vec![
            Block {
                id: BlockId(0),
                instructions: vec![value(0, InstKind::ConstBool(true), IrType::Boolean)],
                terminator: Some(Terminator::Branch {
                    condition: Operand(ValueId(0)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                }),
            },
            Block {
                id: BlockId(1),
                instructions: vec![mutex_op(InstKind::MutexAcquire {
                    id: MutexId(2),
                    mutex: SlotId(0),
                })],
                terminator: Some(Terminator::Jump(BlockId(3))),
            },
            Block {
                id: BlockId(2),
                instructions: vec![],
                terminator: Some(Terminator::Jump(BlockId(3))),
            },
            Block {
                id: BlockId(3),
                instructions: vec![mutex_op(InstKind::MutexRelease {
                    id: MutexId(2),
                    mutex: SlotId(0),
                })],
                terminator: Some(Terminator::Return(None)),
            },
        ],
        IrType::Void,
        vec![],
    );
    assert!(
        errors_of(&branch)
            .iter()
            .any(|e| e.contains("incompatible mutex states"))
    );

    let looped = module_with_mutex(
        vec![
            Block {
                id: BlockId(0),
                instructions: vec![value(0, InstKind::ConstBool(true), IrType::Boolean)],
                terminator: Some(Terminator::Branch {
                    condition: Operand(ValueId(0)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                }),
            },
            Block {
                id: BlockId(1),
                instructions: vec![mutex_op(InstKind::MutexAcquire {
                    id: MutexId(2),
                    mutex: SlotId(0),
                })],
                terminator: Some(Terminator::Jump(BlockId(0))),
            },
            Block {
                id: BlockId(2),
                instructions: vec![],
                terminator: Some(Terminator::Return(None)),
            },
        ],
        IrType::Void,
        vec![],
    );
    assert!(
        errors_of(&looped)
            .iter()
            .any(|e| e.contains("incompatible mutex states"))
    );
}

#[test]
fn mutex_cfg_verifier_accepts_unlocks_on_each_branch_using_the_same_rooted_slot() {
    let module = module_with_mutex(
        vec![
            Block {
                id: BlockId(0),
                instructions: vec![
                    value(0, InstKind::ConstBool(true), IrType::Boolean),
                    mutex_op(InstKind::MutexAcquire {
                        id: MutexId(4),
                        mutex: SlotId(0),
                    }),
                ],
                terminator: Some(Terminator::Branch {
                    condition: Operand(ValueId(0)),
                    then_block: BlockId(1),
                    else_block: BlockId(2),
                }),
            },
            Block {
                id: BlockId(1),
                instructions: vec![mutex_op(InstKind::MutexRelease {
                    id: MutexId(4),
                    mutex: SlotId(0),
                })],
                terminator: Some(Terminator::Jump(BlockId(3))),
            },
            Block {
                id: BlockId(2),
                instructions: vec![mutex_op(InstKind::MutexRelease {
                    id: MutexId(4),
                    mutex: SlotId(0),
                })],
                terminator: Some(Terminator::Jump(BlockId(3))),
            },
            Block {
                id: BlockId(3),
                instructions: vec![],
                terminator: Some(Terminator::Return(None)),
            },
        ],
        IrType::Void,
        vec![],
    );
    verify(&module).expect("each branch releases the same rooted mutex handle");
}

#[test]
fn mutex_cfg_verifier_requires_release_to_use_the_acquired_handle() {
    let mut module = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(9),
                    mutex: SlotId(0),
                }),
                mutex_op(InstKind::MutexRelease {
                    id: MutexId(9),
                    mutex: SlotId(1),
                }),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    module.functions[0].slots.push(Slot {
        name: "other_mutex".into(),
        ty: IrType::Mutex(0),
        span: S,
    });
    module.functions[0].gc_roots.push(SlotId(1));
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("handle does not match")),
        "{errors:?}"
    );
}

#[test]
fn mutex_cfg_verifier_rejects_replacing_a_rooted_handle_while_held() {
    let module = module_with_mutex(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                mutex_op(InstKind::MutexAcquire {
                    id: MutexId(3),
                    mutex: SlotId(0),
                }),
                value(0, InstKind::Undefined, IrType::Mutex(0)),
                mutex_op(InstKind::Store(SlotId(0), Operand(ValueId(0)))),
                mutex_op(InstKind::MutexRelease {
                    id: MutexId(3),
                    mutex: SlotId(0),
                }),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    assert!(
        errors_of(&module)
            .iter()
            .any(|error| error.contains("cannot replace mutex handle"))
    );
}

fn job_wait_any_module(job_types: Vec<IrType>, result_type: IrType) -> Module {
    let slots = job_types
        .iter()
        .enumerate()
        .map(|(index, ty)| Slot {
            name: format!("arg{index}"),
            ty: *ty,
            span: S,
        })
        .collect::<Vec<_>>();
    let mut instructions = job_types
        .iter()
        .enumerate()
        .map(|(index, _)| {
            value(
                index as u32,
                InstKind::Load(SlotId(index as u32)),
                slots[index].ty,
            )
        })
        .collect::<Vec<_>>();
    let jobs = (0..job_types.len())
        .map(|index| Operand(ValueId(index as u32)))
        .collect();
    instructions.push(value(
        job_types.len() as u32,
        InstKind::JobWaitAny { jobs },
        result_type,
    ));
    module_with(
        vec![Block {
            id: BlockId(0),
            instructions,
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        slots,
    )
}

fn job_wait_any_with_timer_module(job_types: Vec<IrType>, result_type: IrType) -> Module {
    let slots = job_types
        .iter()
        .enumerate()
        .map(|(index, ty)| Slot {
            name: format!("arg{index}"),
            ty: *ty,
            span: S,
        })
        .collect::<Vec<_>>();
    let mut instructions = job_types
        .iter()
        .enumerate()
        .map(|(index, _)| {
            value(
                index as u32,
                InstKind::Load(SlotId(index as u32)),
                slots[index].ty,
            )
        })
        .collect::<Vec<_>>();
    let jobs = (0..job_types.len())
        .map(|index| Operand(ValueId(index as u32)))
        .collect();
    instructions.push(value(
        job_types.len() as u32,
        InstKind::JobWaitAnyWithTimer { jobs },
        result_type,
    ));
    module_with(
        vec![Block {
            id: BlockId(0),
            instructions,
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        slots,
    )
}

fn job_outcome_wait_take_module(
    job_type: IrType,
    value_type: IrType,
    throwable_type: IrType,
    roots: Vec<SlotId>,
) -> Module {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(0, InstKind::Load(SlotId(0)), job_type),
                value(
                    1,
                    InstKind::JobOutcomeWaitTake {
                        job: Operand(ValueId(0)),
                        value_slot: SlotId(1),
                        throwable_slot: SlotId(2),
                    },
                    IrType::Int(IntWidth::U32),
                ),
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![
            Slot {
                name: "job".into(),
                ty: IrType::Job,
                span: S,
            },
            Slot {
                name: "value".into(),
                ty: value_type,
                span: S,
            },
            Slot {
                name: "throwable".into(),
                ty: throwable_type,
                span: S,
            },
        ],
    );
    module.functions[0].gc_roots = roots;
    module
}

fn scope_outcome_wait_take_module(ordinal: IrType, value_ty: IrType, roots: Vec<SlotId>) -> Module {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![
                value(
                    0,
                    InstKind::ScopeEnterCollectAll,
                    IrType::Int(IntWidth::I64),
                ),
                value(
                    1,
                    InstKind::ScopeOutcomeWaitTake {
                        scope: Operand(ValueId(0)),
                        ordinal_slot: SlotId(0),
                        value_slot: SlotId(1),
                        throwable_slot: SlotId(2),
                    },
                    IrType::Int(IntWidth::U32),
                ),
                Instruction {
                    result: None,
                    kind: InstKind::ScopeCancel {
                        scope: Operand(ValueId(0)),
                    },
                    ty: IrType::Void,
                    span: S,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![
            Slot {
                name: "ordinal".into(),
                ty: ordinal,
                span: S,
            },
            Slot {
                name: "value".into(),
                ty: value_ty,
                span: S,
            },
            Slot {
                name: "throwable".into(),
                ty: IrType::String,
                span: S,
            },
        ],
    );
    module.functions[0].gc_roots = roots;
    module
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

#[test]
fn job_wait_any_accepts_two_to_eight_jobs_and_returns_u64() {
    for count in 2..=8 {
        let module = job_wait_any_module(vec![IrType::Job; count], IrType::Int(IntWidth::U64));
        assert!(verify(&module).is_ok(), "expected {count} jobs to be valid");
    }
}

#[test]
fn job_wait_any_rejects_counts_outside_two_to_eight() {
    for count in [0, 1, 9] {
        let module = job_wait_any_module(vec![IrType::Job; count], IrType::Int(IntWidth::U64));
        let errors = errors_of(&module);
        assert!(errors.iter().any(|error| error.contains("JobWaitAny has")));
    }
}

#[test]
fn job_wait_any_rejects_non_job_operands() {
    let module = job_wait_any_module(
        vec![IrType::Job, IrType::Int(IntWidth::I32)],
        IrType::Int(IntWidth::U64),
    );
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("JobWaitAny operand 2 is not a Job"))
    );
}

#[test]
fn job_wait_any_rejects_a_non_u64_result() {
    let module = job_wait_any_module(vec![IrType::Job; 2], IrType::Int(IntWidth::U32));
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("JobWaitAny") && error.contains("UInt32"))
    );
}

#[test]
fn job_wait_any_operands_are_checked_as_uses() {
    let mut module = job_wait_any_module(vec![IrType::Job; 2], IrType::Int(IntWidth::U64));
    let instruction = module.functions[0].blocks[0]
        .instructions
        .last_mut()
        .unwrap();
    if let InstKind::JobWaitAny { jobs } = &mut instruction.kind {
        jobs[1] = Operand(ValueId(99));
    }
    let errors = errors_of(&module);
    assert!(
        errors.iter().any(|error| error.contains("undefined value")),
        "{errors:?}"
    );
}

#[test]
fn timer_wait_any_accepts_two_to_nine_jobs_only() {
    for count in 2..=9 {
        let module =
            job_wait_any_with_timer_module(vec![IrType::Job; count], IrType::Int(IntWidth::U64));
        assert!(verify(&module).is_ok(), "expected {count} jobs to be valid");
    }
    let too_many =
        job_wait_any_with_timer_module(vec![IrType::Job; 10], IrType::Int(IntWidth::U64));
    assert!(
        errors_of(&too_many)
            .iter()
            .any(|error| error.contains("JobWaitAnyWithTimer has"))
    );
    let wrong_type = job_wait_any_with_timer_module(
        vec![IrType::Job, IrType::Int(IntWidth::I32)],
        IrType::Int(IntWidth::U64),
    );
    assert!(
        errors_of(&wrong_type)
            .iter()
            .any(|error| error.contains("JobWaitAnyWithTimer operand 2 is not a Job"))
    );
}

#[test]
fn job_outcome_wait_take_accepts_job_and_distinct_rooted_pointer_slots() {
    let module = job_outcome_wait_take_module(
        IrType::Job,
        IrType::String,
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    assert!(verify(&module).is_ok());
}

#[test]
fn job_outcome_wait_take_rejects_non_job_input() {
    let module = job_outcome_wait_take_module(
        IrType::Int(IntWidth::U64),
        IrType::String,
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    assert!(
        errors_of(&module)
            .iter()
            .any(|e| e.contains("operand is not a Job"))
    );
}

#[test]
fn job_outcome_wait_take_rejects_identical_output_slots() {
    let mut module = job_outcome_wait_take_module(
        IrType::Job,
        IrType::String,
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    if let InstKind::JobOutcomeWaitTake { throwable_slot, .. } =
        &mut module.functions[0].blocks[0].instructions[1].kind
    {
        *throwable_slot = SlotId(1);
    }
    assert!(
        errors_of(&module)
            .iter()
            .any(|e| e.contains("must be distinct"))
    );
}

#[test]
fn scope_outcome_wait_take_accepts_distinct_gc_rooted_managed_slots() {
    let module = scope_outcome_wait_take_module(
        IrType::Int(IntWidth::U64),
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    verify(&module).expect("well-typed scope outcomes write through rooted slots");
}

#[test]
fn scope_outcome_wait_take_rejects_bad_ordinal_and_missing_roots() {
    let bad_ordinal = scope_outcome_wait_take_module(
        IrType::Int(IntWidth::I64),
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    assert!(
        errors_of(&bad_ordinal)
            .iter()
            .any(|e| e.contains("ordinal slot is"))
    );

    let missing_root =
        scope_outcome_wait_take_module(IrType::Int(IntWidth::U64), IrType::String, vec![SlotId(1)]);
    assert!(
        errors_of(&missing_root)
            .iter()
            .any(|e| e.contains("missing from the function's GC roots"))
    );
}

#[test]
fn scope_outcome_wait_take_rejects_aggregate_out_slots() {
    let module = scope_outcome_wait_take_module(
        IrType::Int(IntWidth::U64),
        IrType::Int(IntWidth::U64),
        vec![SlotId(1), SlotId(2)],
    );
    assert!(
        errors_of(&module)
            .iter()
            .any(|e| e.contains("non-pointer managed-reference type"))
    );
}

#[test]
fn job_outcome_wait_take_rejects_unmanaged_and_aggregate_outputs() {
    let module = job_outcome_wait_take_module(
        IrType::Job,
        IrType::Int(IntWidth::U64),
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    assert!(
        errors_of(&module)
            .iter()
            .any(|e| e.contains("non-pointer managed-reference type"))
    );

    let mut module = job_outcome_wait_take_module(
        IrType::Job,
        IrType::Value(0),
        IrType::String,
        vec![SlotId(1), SlotId(2)],
    );
    module.values.push(ValueLayout {
        name: "ContainsString".into(),
        fields: vec![ObjectField {
            name: "text".into(),
            ty: IrType::String,
        }],
    });
    assert!(
        errors_of(&module)
            .iter()
            .any(|e| e.contains("non-pointer managed-reference type"))
    );
}

#[test]
fn job_outcome_wait_take_rejects_slots_missing_from_gc_roots() {
    let module =
        job_outcome_wait_take_module(IrType::Job, IrType::String, IrType::String, vec![SlotId(1)]);
    assert!(errors_of(&module).iter().any(
        |e| e.contains("throwable slot") && e.contains("missing from the function's GC roots")
    ));
}

#[test]
fn safepoint_poll_is_void_and_has_no_result() {
    let mut instruction = value(0, InstKind::SafepointPoll, IrType::Void);
    instruction.result = None;
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![instruction],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    assert!(verify(&module).is_ok());
}

#[test]
fn safepoint_poll_with_a_non_void_type_is_rejected() {
    let mut instruction = value(0, InstKind::SafepointPoll, IrType::Boolean);
    instruction.result = None;
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![instruction],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    let errors = errors_of(&module);
    assert!(errors.iter().any(|error| error.contains("SafepointPoll")));
}

#[test]
fn a_managed_slot_missing_from_the_gc_roots_is_rejected() {
    let module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![Slot {
            name: "reference".into(),
            ty: IrType::String,
            span: S,
        }],
    );
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("missing from the GC roots")),
        "{errors:?}"
    );
}

#[test]
fn an_unmanaged_slot_cannot_be_listed_as_a_gc_root() {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![Slot {
            name: "number".into(),
            ty: IrType::Int(IntWidth::I32),
            span: S,
        }],
    );
    module.functions[0].gc_roots.push(SlotId(0));
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("has unmanaged type Int32")),
        "{errors:?}"
    );
}

#[test]
fn a_raw_pointer_slot_cannot_become_a_gc_root() {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![Slot {
            name: "raw".into(),
            ty: IrType::Pointer(0),
            span: S,
        }],
    );
    module.pointer_types.push(IrType::Int(IntWidth::I32));
    module.functions[0].gc_roots.push(SlotId(0));
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("has unmanaged type Pointer")),
        "raw pointers must never be treated as managed roots: {errors:?}"
    );
}

#[test]
fn an_enum_with_an_invalid_flattened_reference_path_is_rejected() {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    module.enums.push(EnumLayout {
        name: "Broken".into(),
        fields: vec![ObjectField {
            name: "payload".into(),
            ty: IrType::String,
        }],
        variants: vec![vec![1]],
    });
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("out-of-range field index 1")),
        "{errors:?}"
    );
}

#[test]
fn an_infinitely_recursive_inline_layout_is_rejected_before_codegen() {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions: vec![],
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    module.values.push(ValueLayout {
        name: "Recursive".into(),
        fields: vec![ObjectField {
            name: "self_value".into(),
            ty: IrType::Value(0),
        }],
    });
    let errors = errors_of(&module);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("infinitely recursive inline field")),
        "{errors:?}"
    );
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
        capability_requesters: std::collections::HashMap::new(),
        project_globals: Vec::new(),
        strings: vec!["x".into()],
        closures: Vec::new(),
        objects: Vec::new(),
        contracts: Vec::new(),
        values: Vec::new(),
        enums: Vec::new(),
        pointer_types: Vec::new(),
        channel_types: Vec::new(),
        mutex_types: Vec::new(),
        weak_types: Vec::new(),
        native_slice_types: Vec::new(),
        native_slice_mut_types: Vec::new(),
        dependent_types: Vec::new(),
        pin_types: Vec::new(),
        array_types: Vec::new(),
        list_types: Vec::new(),
        map_types: Vec::new(),
        set_types: Vec::new(),
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

// --- `Map<K, V>` / `Set<T>` (roadmap Phase 7, `map-set-collections`) ---------

/// A module whose `main` body is the given instruction list, with the given
/// map/set type tables.
fn module_with_collections(
    instructions: Vec<Instruction>,
    map_types: Vec<(IrType, IrType)>,
    set_types: Vec<IrType>,
) -> Module {
    let mut module = module_with(
        vec![Block {
            id: BlockId(0),
            instructions,
            terminator: Some(Terminator::Return(None)),
        }],
        IrType::Void,
        vec![],
    );
    module.map_types = map_types;
    module.set_types = set_types;
    module
}

#[test]
fn map_new_declares_its_interned_map_type() {
    let module = module_with_collections(
        vec![value(0, InstKind::MapNew { map_id: 0 }, IrType::Map(0))],
        vec![(IrType::String, IrType::Int(IntWidth::I32))],
        vec![],
    );
    assert!(verify(&module).is_ok());
}

#[test]
fn set_new_declares_its_interned_set_type() {
    let module = module_with_collections(
        vec![value(0, InstKind::SetNew { set_id: 0 }, IrType::Set(0))],
        vec![],
        vec![IrType::String],
    );
    assert!(verify(&module).is_ok());
}

#[test]
fn map_new_with_the_wrong_declared_type_is_rejected() {
    let module = module_with_collections(
        vec![value(0, InstKind::MapNew { map_id: 0 }, IrType::String)],
        vec![(IrType::String, IrType::Int(IntWidth::I32))],
        vec![],
    );
    let errors = errors_of(&module);
    assert!(errors.iter().any(|e| e.contains("MapNew")), "{errors:?}");
}

#[test]
fn set_new_with_the_wrong_declared_type_is_rejected() {
    let module = module_with_collections(
        vec![value(0, InstKind::SetNew { set_id: 0 }, IrType::String)],
        vec![],
        vec![IrType::String],
    );
    let errors = errors_of(&module);
    assert!(errors.iter().any(|e| e.contains("SetNew")), "{errors:?}");
}
