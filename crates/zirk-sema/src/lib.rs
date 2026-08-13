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
//! # Error recovery
//!
//! The checker does not stop at the first error. Whatever it cannot determine
//! becomes [`Type::Unknown`], which is compatible with everything, so one real
//! error does not produce a dozen derived ones.

mod checker;
mod scope;
mod types;

pub use checker::{CheckedProgram, check};
pub use scope::{Binding, Scopes, Signature};
pub use types::{PendingType, Type, pending_type};

/// Diagnostic codes of the checker.
pub mod codes {
    use zirk_diagnostics::Code;

    /// A name that resolves to no declaration.
    pub const UNDECLARED_NAME: Code = Code::new("E0401");
    /// A type that does not exist, or belongs to a later phase.
    pub const UNKNOWN_TYPE: Code = Code::new("E0402");
    /// Types that do not match and admit no implicit conversion.
    pub const TYPE_MISMATCH: Code = Code::new("E0403");
    /// Reassignment of a variable declared `inmut`.
    pub const ASSIGN_TO_IMMUTABLE: Code = Code::new("E0404");
    /// A variable read before it holds a value.
    pub const USE_BEFORE_INITIALIZATION: Code = Code::new("E0405");
    /// A call whose argument count does not match the signature.
    pub const WRONG_ARGUMENT_COUNT: Code = Code::new("E0406");
    /// A non-`Void` function with a path that does not return.
    pub const MISSING_RETURN: Code = Code::new("E0407");
    /// An integer literal outside the range of its type.
    pub const INTEGER_OUT_OF_RANGE: Code = Code::new("E0408");
    /// The program declares no `main`.
    pub const MISSING_ENTRYPOINT: Code = Code::new("E0409");
    /// `main` declared with the wrong signature.
    pub const INVALID_ENTRYPOINT: Code = Code::new("E0410");
    /// Two functions sharing a name: there is no overloading.
    pub const DUPLICATE_FUNCTION: Code = Code::new("E0411");
    /// A variable declared with type `Void`.
    pub const VOID_VARIABLE: Code = Code::new("E0412");
}
