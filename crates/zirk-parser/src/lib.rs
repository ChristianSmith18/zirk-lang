//! # zirk-parser
//!
//! **Responsibility:** build the `zirk-ast` syntax tree from `zirk-lexer`
//! tokens, and report grammar errors.
//!
//! **Boundary:** the parser decides whether the program is well *formed*, not
//! whether it makes *sense*. `Int32 + String` is valid syntax and a type error:
//! `zirk-sema` resolves that.
//!
//! The language parser is not reused for `init.zrk`: `ZIRK_LANGUAGE_SPEC.md`
//! section 10 defines it as a declarative DSL with its own parser, arriving in
//! Phase 6.
//!
//! # Constructs from later phases
//!
//! Given `class`, `for`, `match` or any construct that exists in the language
//! but not in this subset, the parser emits a diagnostic that names it and says
//! which phase it arrives in — not "unexpected token". This is decision D6 of
//! the design, and it is what makes the subset comprehensible instead of a
//! different language that happens to resemble Zirk.

mod parser;

pub use parser::parse;

/// Diagnostic codes of the parser.
pub mod codes {
    use zirk_diagnostics::Code;

    /// A token was found where something else was expected.
    pub const UNEXPECTED_TOKEN: Code = Code::new("E0301");
    /// A language construct that is not implemented yet.
    pub const NOT_IMPLEMENTED: Code = Code::new("E0302");
    /// A function is missing its return type.
    pub const MISSING_RETURN_TYPE: Code = Code::new("E0303");
    /// A declaration with neither type nor initializer: its type is unknowable.
    pub const UNTYPED_DECLARATION: Code = Code::new("E0304");
    /// The body of a construct must be enclosed in braces.
    pub const MISSING_BRACES: Code = Code::new("E0305");
    /// Multi-file modules, which arrive in a later phase.
    pub const MODULES_UNAVAILABLE: Code = Code::new("E0306");
    /// A variadic parameter is not the last one in the list.
    pub const VARIADIC_NOT_LAST: Code = Code::new("E0307");
    /// Increment or decrement used where a value is expected.
    pub const INCREMENT_AS_EXPRESSION: Code = Code::new("E0308");
    /// A `match` arm list that is empty.
    pub const EMPTY_MATCH: Code = Code::new("E0309");
    /// An `import` whose source is neither a quoted path nor a standard module.
    pub const INVALID_IMPORT_SOURCE: Code = Code::new("E0310");
    /// Nesting deep enough to threaten the parser's own stack.
    pub const NESTING_TOO_DEEP: Code = Code::new("E0311");
    /// `abstract` used on a member of a class that is not itself `abstract`.
    pub const ABSTRACT_OUTSIDE_ABSTRACT_CLASS: Code = Code::new("E0312");
    /// A `try` with neither a `catch` nor a `finally` (roadmap Phase 4b).
    pub const EMPTY_TRY: Code = Code::new("E0313");
    /// An `extern "C" fn` declaration written with a body (roadmap Phase 4e).
    pub const EXTERN_HAS_BODY: Code = Code::new("E0314");
    /// An `extern` calling-convention literal other than `"C"` (roadmap Phase 4e).
    pub const EXTERN_BAD_CONVENTION: Code = Code::new("E0315");
}
