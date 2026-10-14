use inkwell::context::Context;
use zirk_diagnostics::Span;
use zirk_ir::{
    AtomicKind, Block, BlockId, Function, InstKind, Instruction, IntWidth, IrType, Module, Operand,
    Slot, SlotId, Terminator, ValueId,
};

#[test]
fn managed_atomic_constructor_emits_rooted_gc_abi_call() {
    let context = Context::create();
    let span = Span::new(0, 1);
    let mut module = Module::default();
    module.functions.push(Function {
        name: "managed_atomic_emit".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![Slot {
            name: "atomic".into(),
            ty: IrType::Atomic(AtomicKind::I16),
            span,
        }],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![
                Instruction {
                    result: Some(ValueId(0)),
                    kind: InstKind::ConstInt(-23),
                    ty: IrType::Int(IntWidth::I16),
                    span,
                },
                Instruction {
                    result: Some(ValueId(1)),
                    kind: InstKind::AtomicNew {
                        kind: AtomicKind::I16,
                        value: Operand(ValueId(0)),
                        object: SlotId(0),
                    },
                    ty: IrType::Int(IntWidth::I32),
                    span,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span,
        gc_roots: vec![SlotId(0)],
    });
    zirk_ir::verify(&module).expect("managed Atomic IR verifies");
    let llvm = zirk_codegen_llvm::emit(&context, &module, "managed_atomic");
    llvm.verify().expect("managed Atomic LLVM verifies");
    let text = llvm.to_string();
    assert!(text.contains("@zirk_rt_atomic_managed_new"), "{text}");
    assert!(
        text.contains("call i32 @zirk_rt_atomic_managed_new(i32 3, i64 -23, ptr"),
        "{text}"
    );
    assert!(!text.contains("@zirk_rt_atomic_new("), "{text}");
    assert!(!text.contains("@zirk_rt_atomic_free("), "{text}");
    assert!(
        text.contains("gc_roots"),
        "Atomic slot is absent from root map:\n{text}"
    );
}

#[test]
fn managed_atomic_operations_emit_typed_runtime_calls() {
    let context = Context::create();
    let span = Span::new(0, 1);
    let mut module = Module::default();
    let u32_ty = IrType::Int(IntWidth::U32);
    let status_ty = IrType::Int(IntWidth::I32);
    module.functions.push(Function {
        name: "managed_atomic_ops".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![
            Slot {
                name: "atomic".into(),
                ty: IrType::Atomic(AtomicKind::U32),
                span,
            },
            Slot {
                name: "loaded".into(),
                ty: u32_ty,
                span,
            },
            Slot {
                name: "exchanged".into(),
                ty: u32_ty,
                span,
            },
            Slot {
                name: "added".into(),
                ty: u32_ty,
                span,
            },
            Slot {
                name: "subtracted".into(),
                ty: u32_ty,
                span,
            },
            Slot {
                name: "observed".into(),
                ty: u32_ty,
                span,
            },
            Slot {
                name: "did_exchange".into(),
                ty: IrType::Boolean,
                span,
            },
        ],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![
                Instruction {
                    result: Some(ValueId(0)),
                    kind: InstKind::ConstInt(5),
                    ty: u32_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(1)),
                    kind: InstKind::AtomicNew {
                        kind: AtomicKind::U32,
                        value: Operand(ValueId(0)),
                        object: SlotId(0),
                    },
                    ty: status_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(2)),
                    kind: InstKind::Load(SlotId(0)),
                    ty: IrType::Atomic(AtomicKind::U32),
                    span,
                },
                Instruction {
                    result: Some(ValueId(3)),
                    kind: InstKind::ConstInt(2),
                    ty: u32_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(4)),
                    kind: InstKind::ConstInt(4),
                    ty: u32_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(5)),
                    kind: InstKind::AtomicLoad {
                        object: Operand(ValueId(2)),
                        order: Some(Operand(ValueId(4))),
                        out: SlotId(1),
                    },
                    ty: status_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(6)),
                    kind: InstKind::AtomicStore {
                        object: Operand(ValueId(2)),
                        value: Operand(ValueId(3)),
                        order: Some(Operand(ValueId(4))),
                    },
                    ty: status_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(7)),
                    kind: InstKind::AtomicExchange {
                        object: Operand(ValueId(2)),
                        value: Operand(ValueId(3)),
                        order: Some(Operand(ValueId(4))),
                        out: SlotId(2),
                    },
                    ty: status_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(8)),
                    kind: InstKind::AtomicFetchAdd {
                        object: Operand(ValueId(2)),
                        value: Operand(ValueId(3)),
                        order: Some(Operand(ValueId(4))),
                        out: SlotId(3),
                    },
                    ty: status_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(9)),
                    kind: InstKind::AtomicFetchSub {
                        object: Operand(ValueId(2)),
                        value: Operand(ValueId(3)),
                        order: Some(Operand(ValueId(4))),
                        out: SlotId(4),
                    },
                    ty: status_ty,
                    span,
                },
                Instruction {
                    result: Some(ValueId(10)),
                    kind: InstKind::AtomicCompareExchange {
                        object: Operand(ValueId(2)),
                        current: Operand(ValueId(3)),
                        new_value: Operand(ValueId(0)),
                        success_order: Operand(ValueId(4)),
                        failure_order: Operand(ValueId(4)),
                        observed: SlotId(5),
                        exchanged: SlotId(6),
                    },
                    ty: status_ty,
                    span,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span,
        gc_roots: vec![SlotId(0)],
    });
    zirk_ir::verify(&module).expect("all managed Atomic operations verify");
    let llvm = zirk_codegen_llvm::emit(&context, &module, "managed_atomic_ops");
    llvm.verify()
        .expect("all managed Atomic calls produce valid LLVM");
    let text = llvm.to_string();
    for symbol in [
        "zirk_rt_atomic_managed_load_ordered",
        "zirk_rt_atomic_managed_store_ordered",
        "zirk_rt_atomic_managed_exchange_ordered",
        "zirk_rt_atomic_managed_fetch_add_ordered",
        "zirk_rt_atomic_managed_fetch_sub_ordered",
        "zirk_rt_atomic_managed_compare_exchange",
    ] {
        assert!(text.contains(symbol), "missing {symbol}:\n{text}");
    }
}
