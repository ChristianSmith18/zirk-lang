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
//! # State
//!
//! Empty by design. Phase 0 set up the workspace skeleton without implementing
//! Zirk syntax; the minimal IR arrives in Phase 1.
