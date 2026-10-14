//! The bounded scalar `AtomicCell` C ABI shared with `zirk-runtime`.
//!
//! This is an ABI declaration surface, not a source-language feature. Calls
//! still use ordinary verified `ExternFn`/`InstKind::Call` entries. The cell
//! itself is an opaque runtime allocation; callers must obtain it from
//! `zirk_rt_atomic_new` and release it exactly once with
//! `zirk_rt_atomic_free`. No operation here exposes atomic access to arbitrary
//! Zirk memory.
//!
//! Supported values are `Boolean`, signed/unsigned 8/16/32/64-bit integers.
//! `load`, `store`, `exchange`, `fetch_add` and `fetch_sub` are sequentially
//! consistent. Compare-exchange takes explicit order discriminants; the
//! runtime validates the success/failure pair before performing the operation.
//! 128-bit values, floats, pointers, and aggregate values are deliberately not
//! represented by this ABI.

use crate::{ExternFn, IntWidth, IrType, Module};

/// Memory-order discriminants shared with `zirk-runtime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum AtomicOrder {
    Relaxed = 0,
    Acquire = 1,
    Release = 2,
    AcquireRelease = 3,
    SequentiallyConsistent = 4,
}

/// Operation selected by managed Atomic IR instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagedAtomicOperation {
    Load,
    Store,
    Exchange,
    CompareExchange,
    FetchAdd,
    FetchSub,
}

impl ManagedAtomicOperation {
    /// Returns the runtime entry point. Ordered forms have distinct symbols;
    /// `order == None` is represented by the SeqCst symbol.
    pub const fn symbol(self, ordered: bool) -> &'static str {
        match (self, ordered) {
            (Self::Load, false) => "zirk_rt_atomic_managed_load",
            (Self::Load, true) => "zirk_rt_atomic_managed_load_ordered",
            (Self::Store, false) => "zirk_rt_atomic_managed_store",
            (Self::Store, true) => "zirk_rt_atomic_managed_store_ordered",
            (Self::Exchange, false) => "zirk_rt_atomic_managed_exchange",
            (Self::Exchange, true) => "zirk_rt_atomic_managed_exchange_ordered",
            (Self::CompareExchange, _) => "zirk_rt_atomic_managed_compare_exchange",
            (Self::FetchAdd, false) => "zirk_rt_atomic_managed_fetch_add",
            (Self::FetchAdd, true) => "zirk_rt_atomic_managed_fetch_add_ordered",
            (Self::FetchSub, false) => "zirk_rt_atomic_managed_fetch_sub",
            (Self::FetchSub, true) => "zirk_rt_atomic_managed_fetch_sub_ordered",
        }
    }
}

/// Declares the GC-managed Atomic ABI. The legacy Box-owned ABI remains
/// separately available via `declare_abi` and is not used by source Atomic.
pub fn declare_managed_abi(module: &mut Module) -> Result<(), &'static str> {
    const NAMES: [&str; 11] = [
        "zirk_rt_atomic_managed_new",
        "zirk_rt_atomic_managed_load",
        "zirk_rt_atomic_managed_load_ordered",
        "zirk_rt_atomic_managed_store",
        "zirk_rt_atomic_managed_store_ordered",
        "zirk_rt_atomic_managed_exchange",
        "zirk_rt_atomic_managed_exchange_ordered",
        "zirk_rt_atomic_managed_compare_exchange",
        "zirk_rt_atomic_managed_fetch_add",
        "zirk_rt_atomic_managed_fetch_add_ordered",
        "zirk_rt_atomic_managed_fetch_sub",
    ];
    const LAST: &str = "zirk_rt_atomic_managed_fetch_sub_ordered";
    if NAMES
        .iter()
        .any(|name| module.externs.iter().any(|e| e.name == *name))
        || module.externs.iter().any(|e| e.name == LAST)
    {
        return Err("managed Atomic ABI declaration already exists in this module");
    }
    let opaque = module.intern_pointer_type(IrType::Void);
    let handle = IrType::Pointer(opaque);
    let handle_out = IrType::Pointer(module.intern_pointer_type(handle));
    let u64 = IrType::Int(IntWidth::U64);
    let u32 = IrType::Int(IntWidth::U32);
    let status = IrType::Int(IntWidth::I32);
    let ptr = |module: &mut Module, ty| IrType::Pointer(module.intern_pointer_type(ty));
    let u64_out = ptr(module, u64);
    let bool_out = ptr(module, IrType::Boolean);
    let declarations = [
        ExternFn {
            name: NAMES[0].into(),
            params: vec![u32, u64, handle_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[1].into(),
            params: vec![handle, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[2].into(),
            params: vec![handle, u32, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[3].into(),
            params: vec![handle, u64],
            return_type: status,
        },
        ExternFn {
            name: NAMES[4].into(),
            params: vec![handle, u64, u32],
            return_type: status,
        },
        ExternFn {
            name: NAMES[5].into(),
            params: vec![handle, u64, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[6].into(),
            params: vec![handle, u64, u32, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[7].into(),
            params: vec![handle, u64, u64, u32, u32, u64_out, bool_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[8].into(),
            params: vec![handle, u64, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[9].into(),
            params: vec![handle, u64, u32, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[10].into(),
            params: vec![handle, u64, u64_out],
            return_type: status,
        },
        ExternFn {
            name: LAST.into(),
            params: vec![handle, u64, u32, u64_out],
            return_type: status,
        },
    ];
    module.externs.extend(declarations);
    Ok(())
}

/// Stable `AtomicKind` discriminants from `zirk-runtime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum AtomicKind {
    Boolean = 0,
    I8 = 1,
    U8 = 2,
    I16 = 3,
    U16 = 4,
    I32 = 5,
    U32 = 6,
    I64 = 7,
    U64 = 8,
}

impl AtomicKind {
    /// The exact IR value type accepted by this ABI kind.
    pub const fn ir_type(self) -> IrType {
        match self {
            Self::Boolean => IrType::Boolean,
            Self::I8 => IrType::Int(IntWidth::I8),
            Self::U8 => IrType::Int(IntWidth::U8),
            Self::I16 => IrType::Int(IntWidth::I16),
            Self::U16 => IrType::Int(IntWidth::U16),
            Self::I32 => IrType::Int(IntWidth::I32),
            Self::U32 => IrType::Int(IntWidth::U32),
            Self::I64 => IrType::Int(IntWidth::I64),
            Self::U64 => IrType::Int(IntWidth::U64),
        }
    }

    /// Maps an IR type to its supported atomic kind. Wider and non-scalar
    /// types return `None` rather than being silently truncated.
    pub const fn from_ir_type(ty: IrType) -> Option<Self> {
        match ty {
            IrType::Boolean => Some(Self::Boolean),
            IrType::Int(IntWidth::I8) => Some(Self::I8),
            IrType::Int(IntWidth::U8) => Some(Self::U8),
            IrType::Int(IntWidth::I16) => Some(Self::I16),
            IrType::Int(IntWidth::U16) => Some(Self::U16),
            IrType::Int(IntWidth::I32) => Some(Self::I32),
            IrType::Int(IntWidth::U32) => Some(Self::U32),
            IrType::Int(IntWidth::I64) => Some(Self::I64),
            IrType::Int(IntWidth::U64) => Some(Self::U64),
            _ => None,
        }
    }
}

/// Adds the exact runtime ABI declarations to an IR module.
///
/// Pointer pointee `Void` is used solely as an opaque C object type. Output
/// pointers have their concrete pointee type. The LLVM backend represents all
/// of these as opaque pointers while retaining their declared IR distinction.
/// Returns an error instead of allowing duplicate or conflicting symbols.
pub fn declare_abi(module: &mut Module) -> Result<(), &'static str> {
    const NAMES: [&str; 8] = [
        "zirk_rt_atomic_new",
        "zirk_rt_atomic_free",
        "zirk_rt_atomic_load",
        "zirk_rt_atomic_store",
        "zirk_rt_atomic_exchange",
        "zirk_rt_atomic_compare_exchange",
        "zirk_rt_atomic_fetch_add",
        "zirk_rt_atomic_fetch_sub",
    ];
    if NAMES
        .iter()
        .any(|name| module.externs.iter().any(|e| e.name == *name))
    {
        return Err("AtomicCell ABI declaration already exists in this module");
    }

    let opaque = module.intern_pointer_type(IrType::Void);
    let handle = IrType::Pointer(opaque);
    let handle_out = IrType::Pointer(module.intern_pointer_type(handle));
    let u64_out = IrType::Pointer(module.intern_pointer_type(IrType::Int(IntWidth::U64)));
    let bool_out = IrType::Pointer(module.intern_pointer_type(IrType::Boolean));
    let u32 = IrType::Int(IntWidth::U32);
    let u64 = IrType::Int(IntWidth::U64);
    let status = IrType::Int(IntWidth::I32);
    let declarations = [
        ExternFn {
            name: NAMES[0].into(),
            params: vec![u32, u64, handle_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[1].into(),
            params: vec![handle],
            return_type: IrType::Void,
        },
        ExternFn {
            name: NAMES[2].into(),
            params: vec![handle, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[3].into(),
            params: vec![handle, u64],
            return_type: status,
        },
        ExternFn {
            name: NAMES[4].into(),
            params: vec![handle, u64, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[5].into(),
            params: vec![handle, u64, u64, u32, u32, u64_out, bool_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[6].into(),
            params: vec![handle, u64, u64_out],
            return_type: status,
        },
        ExternFn {
            name: NAMES[7].into(),
            params: vec![handle, u64, u64_out],
            return_type: status,
        },
    ];
    module.externs.extend(declarations);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Block, BlockId, Function, Instruction, Operand, Slot, SlotId, Terminator, ValueId, verify,
    };
    use zirk_diagnostics::Span;

    #[test]
    fn only_runtime_supported_scalar_types_have_atomic_kinds() {
        let supported = [
            AtomicKind::Boolean,
            AtomicKind::I8,
            AtomicKind::U8,
            AtomicKind::I16,
            AtomicKind::U16,
            AtomicKind::I32,
            AtomicKind::U32,
            AtomicKind::I64,
            AtomicKind::U64,
        ];
        for kind in supported {
            assert_eq!(AtomicKind::from_ir_type(kind.ir_type()), Some(kind));
        }
        for unsupported in [
            IrType::Int(IntWidth::I128),
            IrType::Int(IntWidth::U128),
            IrType::Float(crate::FloatWidth::F32),
            IrType::Float(crate::FloatWidth::F64),
            IrType::String,
            IrType::Pointer(0),
        ] {
            assert_eq!(AtomicKind::from_ir_type(unsupported), None);
        }
    }

    #[test]
    fn abi_declarations_match_the_runtime_operation_contract() {
        let mut module = Module::default();
        declare_abi(&mut module).unwrap();
        assert_eq!(module.externs.len(), 8);
        assert_eq!(module.externs[0].name, "zirk_rt_atomic_new");
        assert_eq!(module.externs[0].params.len(), 3);
        assert_eq!(module.externs[1].return_type, IrType::Void);
        assert_eq!(module.externs[5].params.len(), 7);
        assert_eq!(module.externs[5].params[3], IrType::Int(IntWidth::U32));
        assert_eq!(module.externs[5].params[4], IrType::Int(IntWidth::U32));
        assert!(
            module
                .externs
                .iter()
                .filter(|e| e.name != "zirk_rt_atomic_free")
                .all(|e| e.return_type == IrType::Int(IntWidth::I32))
        );
        assert!(declare_abi(&mut module).is_err());
    }

    #[test]
    fn managed_abi_uses_distinct_gc_symbols_and_stable_signatures() {
        let mut module = Module::default();
        declare_managed_abi(&mut module).unwrap();
        assert_eq!(module.externs.len(), 12);
        let find = |name: &str| {
            module
                .externs
                .iter()
                .find(|decl| decl.name == name)
                .unwrap()
        };
        let status = IrType::Int(IntWidth::I32);
        assert_eq!(find("zirk_rt_atomic_managed_new").params.len(), 3);
        assert_eq!(find("zirk_rt_atomic_managed_load_ordered").params.len(), 3);
        assert_eq!(find("zirk_rt_atomic_managed_store_ordered").params.len(), 3);
        assert_eq!(
            find("zirk_rt_atomic_managed_compare_exchange").params.len(),
            7
        );
        assert!(module.externs.iter().all(|decl| decl.return_type == status));
        assert!(declare_managed_abi(&mut module).is_err());
        assert_eq!(AtomicOrder::Relaxed as u32, 0);
        assert_eq!(AtomicOrder::Acquire as u32, 1);
        assert_eq!(AtomicOrder::Release as u32, 2);
        assert_eq!(AtomicOrder::AcquireRelease as u32, 3);
        assert_eq!(AtomicOrder::SequentiallyConsistent as u32, 4);
    }

    #[test]
    fn managed_atomic_ir_requires_rooted_exact_kind_and_scalar_output_slots() {
        let span = Span::new(0, 1);
        let mut module = Module::default();
        let u64 = IrType::Int(IntWidth::U64);
        module.functions.push(Function {
            name: "managed_atomic".into(),
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
                    ty: u64,
                    span,
                },
            ],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: crate::InstKind::ConstInt(9),
                        ty: u64,
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: crate::InstKind::AtomicNew {
                            kind: AtomicKind::U64,
                            value: Operand(ValueId(0)),
                            object: SlotId(0),
                        },
                        ty: IrType::Int(IntWidth::I32),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: crate::InstKind::Load(SlotId(0)),
                        ty: IrType::Atomic(AtomicKind::U64),
                        span,
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: crate::InstKind::AtomicLoad {
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
            gc_roots: vec![SlotId(0)],
        });
        assert!(verify(&module).is_ok(), "{:#?}", verify(&module));

        module.functions[0].gc_roots.clear();
        assert!(
            verify(&module)
                .unwrap_err()
                .iter()
                .any(|error| error.detail.contains("not a GC root")),
            "Atomic constructor must write into an explicitly rooted slot"
        );
    }

    fn module_with_atomic_new_call(initial_type: IrType) -> Module {
        let mut module = Module::default();
        declare_abi(&mut module).unwrap();
        let handle = module.pointer_types[1];
        let pointer_to_handle = module.externs[0].params[2];
        module.functions.push(Function {
            name: "exercise_atomic_abi".into(),
            params: vec![],
            return_type: IrType::Void,
            slots: vec![Slot {
                name: "cell".into(),
                ty: handle,
                span: Span::new(0, 1),
            }],
            blocks: vec![Block {
                id: BlockId(0),
                instructions: vec![
                    Instruction {
                        result: Some(ValueId(0)),
                        kind: crate::InstKind::ConstInt(1),
                        ty: IrType::Int(IntWidth::U32),
                        span: Span::new(0, 1),
                    },
                    Instruction {
                        result: Some(ValueId(1)),
                        kind: crate::InstKind::ConstInt(7),
                        ty: initial_type,
                        span: Span::new(0, 1),
                    },
                    Instruction {
                        result: Some(ValueId(2)),
                        kind: crate::InstKind::PointerFromSlot(SlotId(0)),
                        ty: pointer_to_handle,
                        span: Span::new(0, 1),
                    },
                    Instruction {
                        result: Some(ValueId(3)),
                        kind: crate::InstKind::Call {
                            callee: "zirk_rt_atomic_new".into(),
                            args: vec![
                                Operand(ValueId(0)),
                                Operand(ValueId(1)),
                                Operand(ValueId(2)),
                            ],
                        },
                        ty: IrType::Int(IntWidth::I32),
                        span: Span::new(0, 1),
                    },
                ],
                terminator: Some(Terminator::Return(None)),
            }],
            entry: BlockId(0),
            span: Span::new(0, 1),
            gc_roots: vec![],
        });
        module
    }

    #[test]
    fn atomic_new_call_verifies_with_exact_scalar_and_aligned_slot_pointer_types() {
        let module = module_with_atomic_new_call(IrType::Int(IntWidth::U64));
        assert!(verify(&module).is_ok(), "{:#?}", verify(&module));
    }

    #[test]
    fn atomic_runtime_call_rejects_a_wrong_scalar_width() {
        let module = module_with_atomic_new_call(IrType::Int(IntWidth::I32));
        let errors = verify(&module).expect_err("ABI must reject a mismatched value width");
        assert!(
            errors
                .iter()
                .any(|error| error.detail.contains("expected UInt64"))
        );
    }

    #[test]
    fn atomic_new_rejects_unrepresented_value_types_with_type_diagnostics() {
        let mut aggregate = module_with_atomic_new_call(IrType::Int(IntWidth::U64));
        aggregate.values.push(crate::ValueLayout {
            name: "Pair".into(),
            fields: vec![
                crate::ObjectField {
                    name: "left".into(),
                    ty: IrType::Int(IntWidth::I32),
                },
                crate::ObjectField {
                    name: "right".into(),
                    ty: IrType::Int(IntWidth::I32),
                },
            ],
        });
        let mut pointer = module_with_atomic_new_call(IrType::Int(IntWidth::U64));
        let pointer_id = pointer.intern_pointer_type(IrType::Int(IntWidth::I32));

        let cases = [
            ("pointer", pointer, IrType::Pointer(pointer_id), "Pointer"),
            ("aggregate", aggregate, IrType::Value(0), "value"),
            (
                "float32",
                module_with_atomic_new_call(IrType::Int(IntWidth::U64)),
                IrType::Float(crate::FloatWidth::F32),
                "Float32",
            ),
            (
                "float64",
                module_with_atomic_new_call(IrType::Int(IntWidth::U64)),
                IrType::Float(crate::FloatWidth::F64),
                "Float64",
            ),
            (
                "int128",
                module_with_atomic_new_call(IrType::Int(IntWidth::U64)),
                IrType::Int(IntWidth::I128),
                "Int128",
            ),
            (
                "uint128",
                module_with_atomic_new_call(IrType::Int(IntWidth::U64)),
                IrType::Int(IntWidth::U128),
                "UInt128",
            ),
        ];

        for (name, mut module, unsupported, diagnostic_type) in cases {
            module.functions[0].blocks[0].instructions[1].ty = unsupported;
            let errors = verify(&module).unwrap_err();
            assert!(
                errors.iter().any(|error| {
                    error
                        .detail
                        .contains("argument 2 to extern `zirk_rt_atomic_new`")
                        && error.detail.contains(diagnostic_type)
                        && error.detail.contains("expected UInt64")
                }),
                "{name}: expected an argument type diagnostic, got {errors:#?}"
            );
        }
    }

    #[test]
    fn atomic_new_rejects_malformed_output_and_call_signatures() {
        let mut wrong_output = module_with_atomic_new_call(IrType::Int(IntWidth::U64));
        let wrong_output_type = wrong_output.intern_pointer_type(IrType::Int(IntWidth::U64));
        wrong_output.functions[0].blocks[0].instructions[2].ty = IrType::Pointer(wrong_output_type);

        let mut missing_argument = module_with_atomic_new_call(IrType::Int(IntWidth::U64));
        if let crate::InstKind::Call { args, .. } =
            &mut missing_argument.functions[0].blocks[0].instructions[3].kind
        {
            args.pop();
        }

        let mut wrong_return = module_with_atomic_new_call(IrType::Int(IntWidth::U64));
        wrong_return.functions[0].blocks[0].instructions[3].ty = IrType::Int(IntWidth::I64);

        let cases = [
            (
                "output pointer",
                wrong_output,
                "argument 3 to extern `zirk_rt_atomic_new`",
                "expected Pointer",
            ),
            (
                "arity",
                missing_argument,
                "calls `zirk_rt_atomic_new` with 2 argument(s), expected 3",
                "",
            ),
            (
                "return signature",
                wrong_return,
                "Call declares type Int64 and should be Int32",
                "",
            ),
        ];
        for (name, module, diagnostic, additional) in cases {
            let errors = verify(&module).unwrap_err();
            assert!(
                errors.iter().any(|error| {
                    error.detail.contains(diagnostic)
                        && (additional.is_empty() || error.detail.contains(additional))
                }),
                "{name}: expected signature diagnostic, got {errors:#?}"
            );
        }
    }
}
