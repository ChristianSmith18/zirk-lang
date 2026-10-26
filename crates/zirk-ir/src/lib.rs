//! # zirk-ir
//!
//! **Responsibility:** the typed, target-independent, versioned intermediate
//! representation defined by `ZIRK_COMPILER_SPEC.md` section 4.
//!
//! It is the boundary between frontend and backend: everything reaching codegen
//! passes through here. It is also what ships inside a `.zpkg` as
//! `portable.ir`, so its format is a contract, not an internal detail.
//!
//! **Boundary:** the IR knows nothing about LLVM. Translating it to LLVM IR is
//! the job of `zirk-codegen-llvm`, and that separation is what allows adding
//! another backend without changing public semantics.
//!
//! # Memory rule
//!
//! The IR **assumes no concrete memory model**. Every allocation is expressed
//! as an abstract operation resolved by the runtime. The memory strategy is
//! decided in Phase 4 (see `docs/decisions/ADR-003-memoria.md`) and the IR must
//! not anticipate it with tacit assumptions.
//!
//! # Form
//!
//! Typed three-address code over basic blocks, with locals as slots and no SSA
//! of our own. The decision and its alternative are in decision D2 of the
//! Phase 1 design.

pub mod atomic;
mod ir;
mod lower;
mod unions;
mod verify;

pub use atomic::{
    AtomicKind, AtomicOrder, ManagedAtomicOperation, declare_abi as declare_atomic_cell_abi,
    declare_managed_abi as declare_managed_atomic_abi,
};
pub use ir::{
    BinaryOp, Block, BlockId, BranchKind, ClosureLayout, ContractTable, EnumLayout, ExternFn,
    FloatWidth, Function, InstKind, Instruction, IntWidth, IrType, Module, MutexId, Nullable,
    ObjectField, ObjectLayout, Operand, ProjectGlobalLayout, ProjectGlobalWatchEvent, Slot, SlotId,
    StringId, Terminator, UnaryOp, ValueId, ValueLayout,
};
pub use lower::{constructor_symbol, contract_method_symbol, lower, method_symbol};
pub use unions::{
    MAX_INLINE_UNION_PAYLOAD_BYTES, PORTABLE_IR_SCHEMA_MAJOR,
    UNION_LAYOUT_DESCRIPTOR_SCHEMA_VERSION, UNION_METADATA_SCHEMA_VERSION,
    UNION_PACKAGE_METADATA_SCHEMA_VERSION, UnionAbiExport, UnionAlternativeDescriptor,
    UnionLayoutDescriptor, UnionMetadataError, UnionPackageMetadata, UnionTypeLayout,
    validate_package_union_imports,
};
pub use verify::{IrError, verify};
