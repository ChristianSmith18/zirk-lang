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
    FieldInfo, FloatWidth, FnType, GenericContractInstance, GenericEnumInstance, GenericInstance,
    IntWidth, MethodInfo, PendingType, Type, TypeParamInfo, is_ffi_safe, pending_type,
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
    /// A `Char` literal whose content is not exactly one Unicode grapheme
    /// (roadmap Phase 3b) — `ZIRK_LANGUAGE_SPEC.md` section 3.
    pub const INVALID_CHAR_LITERAL: Code = Code::new("E0432");
    /// A `Result<T,E>` produced by a statement and never consumed (roadmap
    /// Phase 4a) — `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 2's
    /// mandatory consumption. `_ = expr;` discards it explicitly.
    pub const DISCARDED_RESULT: Code = Code::new("E0433");
    /// An exception a `throw`, a rethrow, or a call to a `throws` function
    /// or method can produce, neither caught by a local `try` nor declared
    /// in the enclosing function's own `throws` (roadmap Phase 4b) —
    /// `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3's "an
    /// explicit exception must be caught or declared".
    pub const UNCAUGHT_THROW: Code = Code::new("E0434");
    /// A `throw`/`return`/`break`/`continue` written directly inside a
    /// `finally` block (roadmap Phase 4b) — it "cannot directly ... replace
    /// an active outcome" (section 3). A blunt, sound over-approximation of
    /// that rule for this pass: forbidden regardless of whether an outcome
    /// is actually active, not just when replacing one.
    pub const FINALLY_REPLACES_OUTCOME: Code = Code::new("E0435");
    /// A `catch` clause a name does not resolve to a class that implements
    /// `Throwable`, or one an earlier `catch` in the same `try` already
    /// covers, making it unreachable (roadmap Phase 4b).
    pub const UNREACHABLE_CATCH: Code = Code::new("E0436");
    /// A bare `throw;` outside a `catch` (roadmap Phase 4b) — legal only as
    /// a rethrow of the value the enclosing `catch` bound.
    pub const RETHROW_OUTSIDE_CATCH: Code = Code::new("E0437");
    /// A `match ... with binding` (roadmap Phase 4c,
    /// `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 4) whose
    /// scrutinee is not a `Result<R,Err>`, or whose `binding` names no
    /// arm's own pattern binding, or whose bound value does not implement
    /// `Resource<E>`.
    pub const INVALID_RESOURCE_MATCH: Code = Code::new("E0438");
    /// A `Fn(...) => R`-typed position (a function's declared return type, or
    /// a local's own storage) would need to hold two *differently-captured*
    /// closure literals at once (roadmap Phase 4d, design D14) — general
    /// callable-type polymorphism across distinct capture sets needs the
    /// captures heap-boxed behind a uniform representation, which this pass
    /// does not build (design D13, deferred). Distinct from
    /// [`TYPE_MISMATCH`] so the message can explain *why*, not just that the
    /// types differ.
    pub const AMBIGUOUS_CAPTURING_CALLABLE: Code = Code::new("E0439");
    /// A recursive lambda's own binding (`ZIRK_LANGUAGE_SPEC.md` section 6,
    /// roadmap Phase 4d) referenced inside its own body anywhere other than
    /// as the direct callee of a call — assigned to a variable, passed as
    /// an argument, compared with `is`, or reached from inside a nested
    /// lambda. Lowering has no closure *value* for the binding to give at
    /// any of those points (`zirk-ir`'s own `lower_lambda` doc comment): the
    /// value the recursive name refers to does not exist yet at the moment
    /// this literal is still being built, which is exactly why a direct
    /// self-*call* is rewritten into an ordinary recursive call instead of
    /// reading a captured value. Caught here so a program that would panic
    /// during lowering is rejected with a diagnostic instead.
    pub const RECURSIVE_BINDING_NOT_A_VALUE: Code = Code::new("E0440");
    /// A comma-grouped declaration (roadmap Phase 4d) whose initializer-list
    /// arity does not match its binding-name-list arity — distinct from the
    /// generic [`TYPE_MISMATCH`] per the grammar spec's "targeted
    /// count-mismatch diagnostic".
    pub const MULTI_LET_ARITY_MISMATCH: Code = Code::new("E0441");
    /// A simultaneous assignment (roadmap Phase 4d) whose source-list arity
    /// does not match its destination-list arity — distinct from
    /// [`TYPE_MISMATCH`], same reason as [`MULTI_LET_ARITY_MISMATCH`].
    pub const MULTI_ASSIGN_ARITY_MISMATCH: Code = Code::new("E0442");
    /// Two destinations of a simultaneous assignment resolve to the same
    /// place (design D4's cross-destination duplicate check).
    pub const DUPLICATE_ASSIGN_TARGET: Code = Code::new("E0443");
    /// A type named in a `Pointer<T>` position, or an `extern "C" fn`
    /// parameter/return, that has no stable C-ABI layout (roadmap Phase 4e,
    /// design D2, `ADR-015`).
    pub const NOT_FFI_SAFE: Code = Code::new("E0444");
    /// A `Pointer<T>` operation requiring `unsafe` used outside one (roadmap
    /// Phase 4e, design D3).
    pub const POINTER_OP_OUTSIDE_UNSAFE: Code = Code::new("E0445");
    /// `commit {}` used outside an enclosing `unsafe {}` (roadmap Phase 4e,
    /// design D3).
    pub const COMMIT_OUTSIDE_UNSAFE: Code = Code::new("E0446");
    /// A call to an `extern "C" fn` missing its required `unsafe` and/or
    /// `commit` boundary (roadmap Phase 4e, design D3, `ADR-015`).
    pub const EXTERN_CALL_OUTSIDE_UNSAFE_COMMIT: Code = Code::new("E0447");
    /// A `Pointer<T>` value returned, stored into a field, or captured by a
    /// closure (roadmap Phase 4e, design D4 — the blanket escape rule).
    pub const POINTER_ESCAPES: Code = Code::new("E0448");
}
