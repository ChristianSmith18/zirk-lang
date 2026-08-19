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

pub use checker::{Capture, CheckedProgram, LambdaInfo, check};
pub use scope::{Binding, ParamInfo, Scopes, Signature};
pub use types::{
    AssociatedFieldInfo, Base, ClassType, ContractMethod, ContractType, EnumType, EnumVariantInfo,
    FieldInfo, FnType, GenericContractInstance, GenericEnumInstance, GenericInstance, MethodInfo,
    PendingType, Type, TypeParamInfo, pending_type,
};

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
    /// `break` or `continue` outside any loop.
    pub const JUMP_OUTSIDE_LOOP: Code = Code::new("E0413");
    /// `for ... in` over a type this phase cannot iterate.
    pub const NOT_ITERABLE: Code = Code::new("E0414");
    /// A `match` that does not cover every case.
    pub const NON_EXHAUSTIVE_MATCH: Code = Code::new("E0415");
    /// A named argument that no parameter answers to.
    pub const UNKNOWN_ARGUMENT_NAME: Code = Code::new("E0416");
    /// Reassigning a variable captured by a closure.
    pub const CAPTURED_MUTATION: Code = Code::new("E0417");
    /// An operator applied where it has no effect.
    pub const REDUNDANT_OPERATOR: Code = Code::new("E0418");
    /// Calling something that is not a function.
    pub const NOT_CALLABLE: Code = Code::new("E0419");
    /// A name used as an enum variant that is not one.
    pub const UNKNOWN_VARIANT: Code = Code::new("E0420");
    /// A declaration whose name collides with another.
    pub const DUPLICATE_DECLARATION: Code = Code::new("E0421");
    /// An `if` used as a value without an `else`.
    pub const IF_WITHOUT_ELSE: Code = Code::new("E0422");
    /// A construct this phase checks but does not compile yet.
    pub const NOT_LOWERED: Code = Code::new("E0423");
    /// A field left without a value by a constructor.
    pub const UNINITIALIZED_FIELD: Code = Code::new("E0425");
    /// A member that the type does not have.
    pub const UNKNOWN_MEMBER: Code = Code::new("E0426");
    /// A member the type has but that this code may not see.
    pub const INACCESSIBLE_MEMBER: Code = Code::new("E0427");
    /// A class that does not supply what a contract requires.
    pub const MISSING_IMPLEMENTATION: Code = Code::new("E0429");
    /// A method that replaces an inherited one without saying so, or that says
    /// so without replacing anything.
    pub const MISSING_OVERRIDE: Code = Code::new("E0428");
    /// A construct of the language that this phase does not implement.
    ///
    /// Distinct from [`NOT_LOWERED`]: that one is checked and merely not
    /// compiled, while this one is not implemented at all and names the phase
    /// that brings it.
    pub const PENDING_FEATURE: Code = Code::new("E0424");
    /// An `inmut::strict` reference used where it would gain or come from a
    /// mutable alias of the same reachable graph (D11).
    pub const STRICT_ALIAS_VIOLATION: Code = Code::new("E0430");
    /// A lambda parameter that shares a name with a variable it would
    /// otherwise capture (D10): there is no ordinary shadowing, only
    /// `this.name` disambiguating a field from a colliding parameter.
    pub const ORDINARY_SHADOWING: Code = Code::new("E0431");
}
