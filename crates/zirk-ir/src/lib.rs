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

mod ir;
mod lower;
mod verify;

pub use ir::{
    BinaryOp, Block, BlockId, ClosureLayout, ContractTable, Function, InstKind, Instruction,
    IrType, Module, Nullable, ObjectField, ObjectLayout, Operand, Slot, SlotId, StringId,
    Terminator, UnaryOp, ValueId,
};
pub use lower::{constructor_symbol, contract_method_symbol, lower, method_symbol};
pub use verify::{IrError, verify};
