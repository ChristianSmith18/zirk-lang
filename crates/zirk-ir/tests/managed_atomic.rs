use zirk_diagnostics::Span;
use zirk_ir::{
    AtomicKind, Block, BlockId, Function, InstKind, Instruction, IntWidth, IrType, Module, Operand,
    Slot, SlotId, Terminator, ValueId, verify,
};

fn constructor_module(rooted: bool) -> Module {
    let span = Span::new(0, 1);
    let mut module = Module::default();
    module.functions.push(Function {
        name: "managed_atomic_constructor".into(),
        params: vec![],
        return_type: IrType::Void,
        slots: vec![
            Slot {
                name: "atomic".into(),
                ty: IrType::Atomic(AtomicKind::U64),
                span,
            },
            Slot {
                name: "loaded".into(),
                ty: IrType::Int(IntWidth::U64),
                span,
            },
        ],
        blocks: vec![Block {
            id: BlockId(0),
            instructions: vec![
                Instruction {
                    result: Some(ValueId(0)),
                    kind: InstKind::ConstInt(17),
                    ty: IrType::Int(IntWidth::U64),
                    span,
                },
                Instruction {
                    result: Some(ValueId(1)),
                    kind: InstKind::AtomicNew {
                        kind: AtomicKind::U64,
                        value: Operand(ValueId(0)),
                        object: SlotId(0),
                    },
                    ty: IrType::Int(IntWidth::I32),
                    span,
                },
                Instruction {
                    result: Some(ValueId(2)),
                    kind: InstKind::Load(SlotId(0)),
                    ty: IrType::Atomic(AtomicKind::U64),
                    span,
                },
                Instruction {
                    result: Some(ValueId(3)),
                    kind: InstKind::AtomicLoad {
                        object: Operand(ValueId(2)),
                        order: None,
                        out: SlotId(1),
                    },
                    ty: IrType::Int(IntWidth::I32),
                    span,
                },
            ],
            terminator: Some(Terminator::Return(None)),
        }],
        entry: BlockId(0),
        span,
        gc_roots: rooted.then_some(SlotId(0)).into_iter().collect(),
    });
    module
}

#[test]
fn managed_atomic_constructor_and_load_require_a_registered_gc_root() {
    assert!(verify(&constructor_module(true)).is_ok());
    let errors =
        verify(&constructor_module(false)).expect_err("constructor destination must be rooted");
    assert!(
        errors
            .iter()
            .any(|error| error.detail.contains("not a GC root"))
    );
}

#[test]
fn managed_atomic_abi_does_not_reuse_box_owned_symbols() {
    let mut module = Module::default();
    zirk_ir::declare_managed_atomic_abi(&mut module).unwrap();
    let names: Vec<_> = module
        .externs
        .iter()
        .map(|decl| decl.name.as_str())
        .collect();
    assert!(names.contains(&"zirk_rt_atomic_managed_new"));
    assert!(names.contains(&"zirk_rt_atomic_managed_compare_exchange"));
    assert!(!names.contains(&"zirk_rt_atomic_new"));
    assert!(!names.contains(&"zirk_rt_atomic_free"));
}
