//! # zirk-sema
//!
//! **Responsibility:** module and name resolution, type checking and flow
//! analysis over the `zirk-ast` tree.
//!
//! It covers stages 3 and 4 of the pipeline in `ZIRK_COMPILER_SPEC.md` section
//! 2: this is where the rules of `ZIRK_LANGUAGE_SPEC.md` about mutability
//! (`mut` / `inmut` / `inmut::strict`), nullability and inference are applied.
//!
//! **Boundary:** it produces a typed, verified tree; it neither generates code
//! nor decides how data is represented in memory. How a value materializes is
//! the business of `zirk-ir` and the runtime.
//!
//! # State
//!
//! Empty by design. Phase 0 set up the workspace skeleton without implementing
//! Zirk syntax; type checking arrives in Phase 1 with a minimal subset.
