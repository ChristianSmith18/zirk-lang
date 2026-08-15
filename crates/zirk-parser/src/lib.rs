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
}
