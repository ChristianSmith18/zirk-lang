//! # zirk-ast
//!
//! **Responsibility:** define the nodes of the Zirk syntax tree and how they
//! map back to source locations.
//!
//! **Boundary:** this crate only defines the *shape* of the tree. It does not
//! build it (that is `zirk-parser`), nor interpret or validate it (that is
//! `zirk-sema`).
//!
//! Per `ZIRK_COMPILER_SPEC.md` section 3, the internal semantic AST is private
//! and may evolve with the compiler. External tooling does not consume it
//! directly: it uses the public Syntax API, which arrives in a later phase and
//! is a different contract from this crate.
//!
//! # Shape of the tree
//!
//! One type per construct is used instead of a homogeneous tree of generic
//! nodes. The decision and its alternative live in the `design.md` of
//! `fase-1-pipeline-minimo`, decision D1: a homogeneous CST-style tree would be
//! better for the Phase 9 LSP, but it imposes complexity across every layer
//! eight phases before it pays off. When it arrives it is introduced as an
//! extra layer beneath this AST, not in its place.
//!
//! **Every node carries its span, without exception.** A node without a
//! location cannot produce the diagnostic `ZIRK_COMPILER_SPEC.md` section 8
//! requires.

use zirk_diagnostics::Span;

/// A parsed source file.
///
/// A program is one file of a crate. Which files make up the crate, and how
/// their names resolve to each other, is decided by module resolution, not
/// here.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub uses: Vec<UseDecl>,
    pub enums: Vec<EnumDecl>,
    pub classes: Vec<ClassDecl>,
    pub contracts: Vec<ContractDecl>,
    pub functions: Vec<FnDecl>,
    pub type_aliases: Vec<TypeAliasDecl>,
    /// `extern "C" fn name(...): T;` (roadmap Phase 4e, `ADR-015`).
    pub externs: Vec<ExternFnDecl>,
    pub span: Span,
}

/// `type UserLookup = Result<User, LookupError>;`
///
/// Transparent: the alias and its target are the same static type
/// (`ZIRK_LANGUAGE_SPEC.md` section 7), so it carries no representation of its
/// own — resolving a reference to it resolves `target` instead.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDecl {
    pub name: Ident,
    pub target: TypeRef,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
    pub span: Span,
}

/// `import { A, B -> C } from "./path";`
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    pub names: Vec<ImportName>,
    pub source: ImportSource,
    pub span: Span,
}

/// One name inside an `import` list, with its optional alias.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportName {
    pub name: Ident,
    /// Present for `name -> alias`.
    pub alias: Option<Ident>,
    pub span: Span,
}

impl ImportName {
    /// The name this import binds in the importing file.
    pub fn bound_name(&self) -> &Ident {
        self.alias.as_ref().unwrap_or(&self.name)
    }
}

/// Where an `import` reads from.
///
/// The quotes are what distinguishes them, per `ZIRK_LANGUAGE_SPEC.md`
/// section 10: local paths are quoted, standard modules are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    /// `"./domain/user"`, without the `.zrk` extension.
    Local { path: String, span: Span },
    /// `std.io`
    Std { path: String, span: Span },
}

impl ImportSource {
    pub fn span(&self) -> Span {
        match self {
            ImportSource::Local { span, .. } | ImportSource::Std { span, .. } => *span,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            ImportSource::Local { path, .. } | ImportSource::Std { path, .. } => path,
        }
    }
}

/// `use stdout;`
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub name: Ident,
    pub span: Span,
}

/// `enum Direction { North, South }`, or `enum Shape { Circle(Int32), Point }`
/// with associated data (`ZIRK_LANGUAGE_SPEC.md` section 7).
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<EnumVariant>,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
    pub span: Span,
}

/// One variant of an `enum`.
///
/// A traditional variant is a bare name, optionally mapped to a string or
/// numeric value with `-> value`; its default value is the name itself. An
/// algebraic variant instead carries zero or more associated types in
/// `(...)`, and is unpacked only by `match`. A variant is one shape or the
/// other, never both: `associated` and `mapping` are not simultaneously
/// non-empty/`Some`.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    /// `(name: Type, name: Type, ...)`, empty for a traditional variant.
    pub associated: Vec<AssociatedField>,
    /// `-> value`, only ever on a variant with no associated data.
    pub mapping: Option<Expr>,
    pub span: Span,
}

/// One named, typed value an algebraic variant carries, as in `progress:
/// Float64` inside `Loading(progress: Float64)`.
#[derive(Debug, Clone, PartialEq)]
pub struct AssociatedField {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

/// `class User { ... }` or `record Point { ... }`.
///
/// One AST shape for the two: they share fields, methods and construction
/// machinery, and differ only in which rules the checker applies to a given
/// [`ClassKind`] — a record rejects `extends`, a custom `construct`, and
/// mutation, and is written as a compact declaration (`Self::to_record_style`
/// folds it into the same shape the parser gives a `record`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name: Ident,
    pub kind: ClassKind,
    pub type_params: Vec<TypeParam>,
    /// The contracts this class says it satisfies.
    pub implements: Vec<TypeRef>,
    /// The class this one extends, if any.
    ///
    /// At most one: `ZIRK_LANGUAGE_SPEC.md` section 7 admits a single base
    /// class, and several contracts. Always `None` for a record,
    /// which the grammar accepts and the checker rejects — the same
    /// treatment as any other rule tied to `kind`.
    pub extends: Option<Ident>,
    pub fields: Vec<FieldDecl>,
    /// Every `construct` the class declares. More than one is allowed when
    /// their effective signatures differ (`ZIRK_LANGUAGE_SPEC.md` section 7).
    /// Always empty for a record: construction is always the
    /// implicit named constructor over `fields`.
    pub constructors: Vec<ConstructDecl>,
    pub methods: Vec<MethodDecl>,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
    pub span: Span,
}

/// What kind of nominal type a [`ClassDecl`] declares, per
/// `ZIRK_LANGUAGE_SPEC.md` section 7's three-way split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// Identity, state, inheritance, a custom `construct`.
    Class,
    /// A nominal value: named-only construction, structural equality, no
    /// identity, no inheritance, no mutation.
    Record,
    /// `abstract class Name { ... }`: a nominal set of required attributes
    /// and `abstract fn` signatures, with no constructor, method body,
    /// allocated state or layout contribution of its own. A concrete class
    /// adopts it with `implements`, the same as an interface or a trait.
    Abstract,
}

impl ClassKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ClassKind::Class => "class",
            ClassKind::Record => "record",
            ClassKind::Abstract => "abstract class",
        }
    }
}

/// `interface Serializable { ... }` or `trait Printable { ... }`
///
/// One node for both: `ZIRK_LANGUAGE_SPEC.md` section 7 separates them by a
/// single thing — a trait may carry implementation. Everything else about them
/// is identical, and modelling them apart would duplicate every rule to say
/// the same. The keyword is kept because the spec distinguishes them and so
/// must the diagnostic: writing a body in an `interface` is a mistake that
/// deserves to be named as one.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractDecl {
    pub name: Ident,
    pub kind: ContractKind,
    pub type_params: Vec<TypeParam>,
    /// Contracts this contract extends or re-exports (`interface A implements B`).
    pub implements: Vec<TypeRef>,
    pub methods: Vec<MethodDecl>,
    /// Marked `share`, so other files of the crate may name it.
    pub shared: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractKind {
    /// Signatures only.
    Interface,
    /// Signatures, and bodies for the ones it chooses to supply.
    Trait,
}

impl ContractKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ContractKind::Interface => "interface",
            ContractKind::Trait => "trait",
        }
    }
}

/// A field of a class.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: Ident,
    pub ty: TypeRef,
    pub visibility: Visibility,
    pub mutability: Mutability,
    /// Whether the source wrote either modifier.
    ///
    /// An unmodified field is `public mut`, and both spellings mean the same
    /// thing — but only one of them says it out loud, and a diagnostic about
    /// a redundant modifier needs to tell them apart.
    pub explicit_modifiers: bool,
    pub span: Span,
}

/// `construct(...) { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct ConstructDecl {
    pub params: Vec<Param>,
    pub body: Block,
    pub visibility: Visibility,
    pub span: Span,
}

/// A method of a class.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDecl {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    /// Written `override fn`, which replacing an inherited method requires
    /// (`ZIRK_LANGUAGE_SPEC.md` section 7).
    pub is_override: bool,
    /// Written `mut fn`, marking a method that mutates its receiver.
    pub is_mut: bool,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    /// `throws Type (| Type)*` (roadmap Phase 4b) — reuses `TypeRef`'s own
    /// union syntax (`union_with`) rather than a `Vec<TypeRef>` of its own,
    /// since `A | B` after `throws` is exactly the same grammar `parse_type`
    /// already handles for an ordinary type.
    pub throws: Option<TypeRef>,
    /// Absent on an `abstract` method, which declares a signature and no body.
    pub body: Option<Block>,
    pub visibility: Visibility,
    pub is_abstract: bool,
    pub span: Span,
}

/// Access level of a class member, per `ZIRK_LANGUAGE_SPEC.md` section 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
}

impl Visibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Protected => "protected",
        }
    }
}

/// Function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: Ident,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    /// Return type. Mandatory in this phase.
    pub return_type: TypeRef,
    /// `throws Type (| Type)*` (roadmap Phase 4b) — see `MethodDecl::throws`.
    pub throws: Option<TypeRef>,
    pub body: Block,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
    /// `unsafe fn`, whose whole body runs as if wrapped in `unsafe {}`
    /// (roadmap Phase 4e).
    pub is_unsafe: bool,
    pub span: Span,
}

/// `extern "C" fn name(params): ReturnType;` — a bodyless native declaration
/// (roadmap Phase 4e, `ADR-015-declaracion-extern.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternFnDecl {
    pub name: Ident,
    /// The calling-convention literal as written, e.g. `"C"`. Kept even
    /// though only `"C"` is accepted, so the checker/parser can name the
    /// offending literal in its own diagnostic instead of losing it here.
    pub convention: StrLit,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    pub span: Span,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
    /// `name?: T`, which makes the parameter nullable and defaultable to null.
    pub optional: bool,
    /// `name: T = expr`.
    pub default: Option<Expr>,
    /// `...name: T`, which collects the remaining arguments.
    pub variadic: bool,
    pub span: Span,
}

/// An identifier together with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

/// A syntactic reference to a type.
///
/// This is what the user **wrote**, not the resolved type: `zirk-sema` turns it
/// into a type of the system and emits the diagnostic if it does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRef {
    pub name: String,
    /// Type arguments, as in `Box<Int32>`.
    pub arguments: Vec<TypeRef>,
    /// Written `T?`, which per `ZIRK_LANGUAGE_SPEC.md` section 4 is `T | Null`.
    pub nullable: bool,
    /// The rest of a union's alternatives, as in `String | Int32`: this
    /// `TypeRef` is the first (`name`, `arguments`, `nullable` describe it
    /// alone), and each further `A |` adds one entry here. Empty for an
    /// ordinary, non-union type.
    pub union_with: Vec<TypeRef>,
    /// `Fn(P...) => R` / `Function(P...) => R` (roadmap Phase 4d), when this
    /// reference names a callable type instead of an ordinary one. `name` is
    /// still set (`"Fn"` or `"Function"`, whichever was written) purely for
    /// diagnostics; every consumer that cares about the callable shape reads
    /// this field first.
    pub function: Option<Box<FnTypeRef>>,
    pub span: Span,
}

impl TypeRef {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            arguments: Vec::new(),
            nullable: false,
            union_with: Vec::new(),
            function: None,
            span,
        }
    }

    pub fn nullable(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            arguments: Vec::new(),
            nullable: true,
            union_with: Vec::new(),
            function: None,
            span,
        }
    }

    /// `Fn(P...) => R`, written as `name` (`"Fn"` or `"Function"`).
    pub fn function(name: impl Into<String>, function: FnTypeRef, span: Span) -> Self {
        Self {
            name: name.into(),
            arguments: Vec::new(),
            nullable: false,
            union_with: Vec::new(),
            function: Some(Box::new(function)),
            span,
        }
    }
}

/// The parameter list and result of a written `Fn(P...) => R` type — see
/// [`TypeRef::function`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnTypeRef {
    pub params: Vec<FnTypeParamRef>,
    pub returns: Box<TypeRef>,
}

/// One parameter of a written callable type: `T`, `name: T`, `name?: T`, or
/// `...name: T`. `label` is `None` for a bare `T`, written purely for its
/// type — matching how invoking a value of this type always passes
/// arguments positionally (`Checker::check_call`'s closure-call path rejects
/// named arguments), so a label here documents intent without being
/// semantically load-bearing the way a declared function's own parameter
/// name is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnTypeParamRef {
    pub label: Option<Ident>,
    /// `name?: T`.
    pub optional: bool,
    /// `...name: T`.
    pub variadic: bool,
    pub ty: TypeRef,
    pub span: Span,
}

/// A declared type parameter, as in `<T from Serializable>` or `<T = Int32>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    pub name: Ident,
    /// Written `in T` or `out T`. Invariant when neither is written.
    pub variance: Variance,
    /// Constraints written with `from`, combined with `&`.
    ///
    /// Several because `from A & B` requires all of them at once, which is
    /// what lets a body use everything each one promises.
    pub constraints: Vec<TypeRef>,
    /// Optional default type for trailing parameters (`<T = Int32>`).
    pub default: Option<TypeRef>,
    pub span: Span,
}

/// Declared variance of a generic parameter, `ZIRK_LANGUAGE_SPEC.md` section 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    /// Neither `in` nor `out`: the default, and the only option a mutable
    /// attribute may use.
    Invariant,
    /// `in T`, restricted to contravariant input positions.
    In,
    /// `out T`, restricted to covariant output positions.
    Out,
}

/// A block of statements with its own scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

/// Mutability of a variable declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// `mut`: allows reassignment.
    Mutable,
    /// `inmut`: freezes the reference.
    Immutable,
    /// `inmut::strict`: freezes the reachable graph, not just the binding
    /// (D11) — no mutable aliases can be produced from it, and it cannot be
    /// acquired from a mutable alias that is still accessible.
    Strict,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `mut x: Int32 = 0;`
    Let(LetStmt),
    /// `mut first, second: String;` (roadmap Phase 4d, comma-grouped
    /// declarations) — a genuinely distinct shape from `Let`, not its N=1
    /// case reused, and not a tuple: `design.md` D1/D2.
    MultiLet(MultiLetStmt),
    /// `x = 1;`
    Assign(AssignStmt),
    /// `left, right = right, left;` (roadmap Phase 4d, simultaneous
    /// assignment) — one node carrying both comma-separated lists, arities
    /// preserved even when they differ (`design.md` D1).
    MultiAssign(MultiAssignStmt),
    /// `if cond { } else { }`
    If(IfStmt),
    /// `while cond { }`, `loop { }`, `do { } while cond;` and
    /// `for init; cond; step { }`, which share a shape once parsed.
    Loop(LoopStmt),
    /// `for x in iterable { }`
    ForIn(ForInStmt),
    /// `break;`
    Break(JumpStmt),
    /// `continue;`
    Continue(JumpStmt),
    /// `return expr;`
    Return(ReturnStmt),
    /// An expression evaluated for its effect, such as a call.
    Expr(ExprStmt),
    /// A nested block.
    Block(Block),
    /// `throw expr;` / `throw;` (rethrow, roadmap Phase 4b).
    Throw(ThrowStmt),
    /// `try { } catch Type(name) { } ... finally { }` (roadmap Phase 4b).
    Try(TryStmt),
    /// `unsafe { }` used for its effect (roadmap Phase 4e). Shares
    /// [`UnsafeBlock`] with [`Expr::Unsafe`] the same way `IfStmt` is shared
    /// between `Stmt::If` and `Expr::If`.
    Unsafe(UnsafeBlock),
    /// `commit { }` used for its effect (roadmap Phase 4e). Valid only nested
    /// inside `unsafe {}` — enforced by the checker (design D3), not here:
    /// the grammar parses it anywhere so the checker can produce its own
    /// contextual diagnostic instead of a raw parse error.
    Commit(CommitBlock),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(s) => s.span,
            Stmt::MultiLet(s) => s.span,
            Stmt::Assign(s) => s.span,
            Stmt::MultiAssign(s) => s.span,
            Stmt::If(s) => s.span,
            Stmt::Loop(s) => s.span,
            Stmt::ForIn(s) => s.span,
            Stmt::Break(s) | Stmt::Continue(s) => s.span,
            Stmt::Return(s) => s.span,
            Stmt::Expr(s) => s.span,
            Stmt::Block(b) => b.span,
            Stmt::Throw(s) => s.span,
            Stmt::Try(s) => s.span,
            Stmt::Unsafe(s) => s.span,
            Stmt::Commit(s) => s.span,
        }
    }
}

/// `unsafe { ... }`, in either statement or expression position (roadmap
/// Phase 4e). See design D3/D4 for the checker context this opens.
#[derive(Debug, Clone, PartialEq)]
pub struct UnsafeBlock {
    pub body: Block,
    pub span: Span,
}

/// `commit { ... }`, in either statement or expression position (roadmap
/// Phase 4e). Legal only nested inside an enclosing `unsafe {}` — a rule the
/// checker enforces (design D3), not the grammar.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitBlock {
    pub body: Block,
    pub span: Span,
}

/// `throw expr;` / `throw;` (roadmap Phase 4b).
///
/// `value: None` is the bare rethrow, legal only directly inside a `catch`
/// (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3: "legal only in a
/// catch and preserves exact identity, original throw point, cause,
/// suppressed list, and trace").
#[derive(Debug, Clone, PartialEq)]
pub struct ThrowStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// `try { } catch Type(name) { } ... finally { }` (roadmap Phase 4b).
///
/// At least one `catch` or a `finally` — a bare `try { }` with neither is
/// rejected by the parser the same way an `if` with no `else` used as an
/// expression is: syntactically total, semantically pointless.
#[derive(Debug, Clone, PartialEq)]
pub struct TryStmt {
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally: Option<Block>,
    pub span: Span,
}

/// One `catch Type(name) { }` arm of a `try`.
///
/// By class type only — no variant pattern (`catch NetworkError.Timeout(d)`)
/// yet, since a user exception has no way to declare one (roadmap Phase 4b's
/// own "fuera de alcance").
#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub ty: TypeRef,
    pub binding: Ident,
    pub body: Block,
    pub span: Span,
}

/// The loop forms that are not `for ... in`.
///
/// `while c { b }` is `LoopStmt { condition: Some(c), .. }`, `loop { b }` is
/// the same with no condition, and `for i; c; s { b }` adds the initialization
/// and the step. Keeping one node instead of four avoids repeating the same
/// lowering four times for what LLVM sees as one shape.
///
/// `do { b } while c;` shares that shape too: the only difference is which
/// block execution enters first, which is one edge in the lowering rather than
/// a node of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopStmt {
    /// The syntactic form written, kept for diagnostics.
    pub kind: LoopKind,
    /// Runs once before the loop, in a scope enclosing it. Only `for`.
    pub init: Option<Box<Stmt>>,
    /// Checked before each iteration. Absent in `loop`.
    pub condition: Option<Expr>,
    /// Runs after each iteration, and is where `continue` jumps to. Only `for`.
    pub step: Option<Box<Stmt>>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopKind {
    For,
    While,
    Loop,
    /// `do { } while cond;`, which checks its condition after the body.
    DoWhile,
}

impl LoopKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            LoopKind::For => "for",
            LoopKind::While => "while",
            LoopKind::Loop => "loop",
            LoopKind::DoWhile => "do ... while",
        }
    }

    /// Whether the body runs before the condition is ever checked.
    pub const fn body_runs_first(self) -> bool {
        matches!(self, LoopKind::DoWhile)
    }
}

/// `for binding in iterable { }`
#[derive(Debug, Clone, PartialEq)]
pub struct ForInStmt {
    pub binding: Ident,
    pub iterable: Expr,
    pub body: Block,
    pub span: Span,
}

/// `break;` or `continue;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpStmt {
    pub span: Span,
}

/// A variable declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub mutability: Mutability,
    pub pattern: Pattern,
    /// Explicit annotation. Absent when the type is inferred.
    pub ty: Option<TypeRef>,
    pub init: Option<Expr>,
    pub span: Span,
}

/// `mut first, second: String;` — a comma-separated list of simple binding
/// names sharing one type annotation and (optionally) one comma-separated
/// initializer list (roadmap Phase 4d).
///
/// Kept as its own node rather than `LetStmt` with `names.len() == 1`
/// (`design.md` D2): the single-name case keeps its existing diagnostics,
/// spans and lowering unchanged. `inits` is empty when no initializer list
/// is written; its length is preserved as parsed even when it does not match
/// `names.len()`, so the checker can emit the targeted arity diagnostic
/// (`design.md` D1/D5) instead of a generic one.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiLetStmt {
    pub mutability: Mutability,
    pub names: Vec<Ident>,
    /// The one type annotation shared by every name. Absent only when an
    /// initializer list is present to infer it from, the same rule
    /// `LetStmt` already applies to its own single name.
    pub ty: Option<TypeRef>,
    pub inits: Vec<Expr>,
    pub span: Span,
}

/// Reassignment of an existing variable.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub target: AssignTarget,
    pub value: Expr,
    pub span: Span,
}

/// `left, right = right, left;` — simultaneous assignment to a
/// comma-separated list of assignable places from a comma-separated list of
/// source expressions (roadmap Phase 4d).
///
/// Both lists' lengths are preserved as parsed even when they differ, so the
/// checker can emit the targeted arity diagnostic (`design.md` D1/D5)
/// instead of a generic one. Lowering evaluates every `values[i]` before
/// writing any `targets[i]` (`design.md` D3), which is what makes the swap
/// scenario correct.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiAssignStmt {
    pub targets: Vec<AssignTarget>,
    pub values: Vec<Expr>,
    pub span: Span,
}

/// The place an assignment writes to.
///
/// Not every expression is one: `f() = 1` names no storage. Keeping the
/// admissible forms in their own type is what lets the parser reject the rest
/// where it reads them, instead of every later layer having to ask again.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    /// `count = 1`
    Name(Ident),
    /// `this.name = value` and `user.name = value`
    Field(FieldExpr),
    /// `view[0] = value` (roadmap Phase 4e, `fase-4e-native-slice`, design
    /// D5) — the same "place" classification `Field` already receives.
    Index(IndexExpr),
}

impl AssignTarget {
    pub fn span(&self) -> Span {
        match self {
            AssignTarget::Name(i) => i.span,
            AssignTarget::Field(f) => f.span,
            AssignTarget::Index(i) => i.span,
        }
    }

    /// The name being written, for a diagnostic that has to say one.
    ///
    /// An index target has no single name to report — callers that need one
    /// (`i++`'s own diagnostic) never reach an index target: `check_increment`
    /// rejects it before this would be called, the same way it already
    /// rejects `Field`.
    pub fn name(&self) -> &str {
        match self {
            AssignTarget::Name(i) => &i.name,
            AssignTarget::Field(f) => &f.name.name,
            AssignTarget::Index(_) => "<index>",
        }
    }
}

/// Conditional as a statement.
///
/// `if` as an expression belongs to Phase 2, even though the spec allows it.
#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

/// Alternative branch of a conditional.
///
/// A block is distinguished from a chain so `else if` does not lose its shape
/// in the tree.
#[derive(Debug, Clone, PartialEq)]
pub enum ElseBranch {
    Block(Block),
    If(Box<IfStmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(IntLit),
    /// A fractional or scientific literal (roadmap Phase 3b).
    Float(FloatLit),
    /// A duration literal such as `250ms` or `1.5s`.
    Duration(DurationLit),
    /// A regex literal such as `re'[a-z]+'`.
    Regex(RegexLit),
    /// A character literal (roadmap Phase 3b).
    Char(CharLit),
    Str(StrLit),
    Bool(BoolLit),
    /// `null`, the sole value of the `Null` half of `T?`.
    Null(NullLit),
    Path(Ident),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Call(CallExpr),
    /// `0..10` and `0..=10`.
    Range(RangeExpr),
    /// `if c { a } else { b }` used where a value is expected.
    ///
    /// The node is the same one the statement form uses: what changes is the
    /// position, and with it whether the branches must produce a value.
    /// Decision D7 of the design.
    If(Box<IfStmt>),
    /// `cond ? a : b`.
    ///
    /// Kept apart from [`Expr::If`] even though both choose between two values:
    /// its branches are expressions rather than blocks, and a diagnostic that
    /// called it an `if` would name something the author did not write.
    Ternary(TernaryExpr),
    /// `i++`, `++i`, `i--` and `--i` where a value is expected.
    ///
    /// In statement position the parser still desugars these into the
    /// equivalent assignment: there is no value to observe, so the prefix and
    /// postfix forms are indistinguishable and the simpler tree wins.
    Increment(IncrementExpr),
    /// `this`, the instance a method or constructor runs on.
    This(ThisExpr),
    /// `super`, which reaches the base class rather than the object's own
    /// type. Only meaningful as `super(...)` or `super.method()`.
    Super(SuperExpr),
    /// `object.field`, and `object?.field` for the safe form.
    ///
    /// Enum variants have their own node because `Direction.North` names a
    /// type rather than a value: there is no object to read a member from.
    Field(FieldExpr),
    /// `receiver[index]` (roadmap Phase 4e, `fase-4e-native-slice`, design
    /// D5) — see [`IndexExpr`].
    Index(IndexExpr),
    /// `receiver[start:end:step]` (roadmap Phase 7) — see [`SliceExpr`].
    Slice(SliceExpr),
    /// `match x { p => v, ... }`, in either position.
    ///
    /// There is no separate statement node: in statement position the parser
    /// wraps this in `Stmt::Expr`, and the checker is what decides whether the
    /// arms must produce a value.
    Match(MatchExpr),
    /// `(a: Int32): Int32 => a + 1`
    Lambda(LambdaExpr),
    /// `Direction.North`, the value of an enum variant.
    Variant(VariantExpr),
    /// `stdout.println(expr)`.
    ///
    /// A special syntactic form recognized by the compiler while neither
    /// modules nor a standard library exist. Deliberate debt, documented in
    /// decision D4 of the design, retired in Phase 7.
    Println(PrintlnExpr),
    /// `expr as Type` or `<Type>expr`, the postfix and prefix spellings of
    /// the same checkable cast (`ZIRK_LANGUAGE_SPEC.md` section 11).
    Cast(CastExpr),
    /// `"text {expr} more text"` (roadmap Phase 3b).
    Interpolated(InterpolatedStrExpr),
    /// `unsafe { ... }` used where a value is expected (roadmap Phase 4e),
    /// e.g. `mut result = unsafe { ptr.read() };`. Shares [`UnsafeBlock`]
    /// with the statement form the same way `Expr::If` shares `IfStmt`.
    Unsafe(Box<UnsafeBlock>),
    /// `commit { ... }` used where a value is expected (roadmap Phase 4e).
    Commit(Box<CommitBlock>),
    /// `transfer(expr)` — ownership transfer of a `TransferableResource`.
    Transfer(TransferExpr),
    /// `(a, b, ...)` — a tuple literal (roadmap Phase 3b).
    Tuple(TupleExpr),
}

/// `expr as Type`, `expr as? Type`, or `<Type>expr`.
#[derive(Debug, Clone, PartialEq)]
pub struct CastExpr {
    pub expr: Box<Expr>,
    pub target: TypeRef,
    /// `true` for the nullable form `as?`.
    pub optional: bool,
    pub span: Span,
}

/// `transfer(expr)` — ownership transfer of a `TransferableResource`.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int(e) => e.span,
            Expr::Float(e) => e.span,
            Expr::Duration(e) => e.span,
            Expr::Regex(e) => e.span,
            Expr::Char(e) => e.span,
            Expr::Str(e) => e.span,
            Expr::Bool(e) => e.span,
            Expr::Null(e) => e.span,
            Expr::Path(i) => i.span,
            Expr::Unary(e) => e.span,
            Expr::Binary(e) => e.span,
            Expr::Call(e) => e.span,
            Expr::Range(e) => e.span,
            Expr::If(e) => e.span,
            Expr::This(e) => e.span,
            Expr::Super(e) => e.span,
            Expr::Field(e) => e.span,
            Expr::Index(e) => e.span,
            Expr::Slice(e) => e.span,
            Expr::Ternary(e) => e.span,
            Expr::Increment(e) => e.span,
            Expr::Match(e) => e.span,
            Expr::Lambda(e) => e.span,
            Expr::Variant(e) => e.span,
            Expr::Println(e) => e.span,
            Expr::Cast(e) => e.span,
            Expr::Interpolated(e) => e.span,
            Expr::Unsafe(e) => e.span,
            Expr::Commit(e) => e.span,
            Expr::Transfer(e) => e.span,
            Expr::Tuple(e) => e.span,
        }
    }
}

/// One piece of an interpolated string literal, in the order it was written.
#[derive(Debug, Clone, PartialEq)]
pub enum InterpolatedPart {
    /// Text between `{...}` sections, with escapes already resolved.
    Literal(String),
    /// One `{expr}` section.
    Expr(Expr),
}

/// `"text {expr} more text"`.
#[derive(Debug, Clone, PartialEq)]
pub struct InterpolatedStrExpr {
    pub parts: Vec<InterpolatedPart>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullLit {
    pub span: Span,
}

/// `start..end`, `start..=end` or `start..end..step` (roadmap Phase 7,
/// `Range<T>`).
#[derive(Debug, Clone, PartialEq)]
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    /// The distance between elements, `None` when the source did not write
    /// one (which means `1`).
    pub step: Option<Box<Expr>>,
    /// `..=` includes the endpoint; `..` does not.
    pub inclusive: bool,
    pub span: Span,
}

/// `(a, b, ...)` — a tuple literal (roadmap Phase 3b).
#[derive(Debug, Clone, PartialEq)]
pub struct TupleExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// `this`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThisExpr {
    pub span: Span,
}

/// `super` or `TraitName.super`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperExpr {
    /// When `Some`, this is the `TraitName` in `TraitName.super`.
    pub trait_name: Option<Ident>,
    pub span: Span,
}

/// `object.field` or `object?.field`
#[derive(Debug, Clone, PartialEq)]
pub struct FieldExpr {
    pub object: Box<Expr>,
    pub name: Ident,
    /// Written `?.`: the whole access produces `null` when the object is
    /// absent, instead of reading through it.
    pub safe: bool,
    pub span: Span,
}

/// `receiver[index]` (roadmap Phase 4e, `fase-4e-native-slice`, design D5) —
/// a general postfix index expression. The grammar accepts any receiver;
/// only the checker decides which receiver types actually support it
/// (today, `NativeSlice<T>`/`NativeSliceMut<T>`), through an explicitly
/// extensible dispatch table Phase 7's `Array<T>`/`List<T>` can register
/// into later.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexExpr {
    pub receiver: Box<Expr>,
    pub index: Box<Expr>,
    pub span: Span,
}

/// `receiver[start:end:step]` (roadmap Phase 7, `String` slicing) — each
/// part is optional, so `s[:]`, `s[::2]` and `s[1:]` all parse.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceExpr {
    pub receiver: Box<Expr>,
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub step: Option<Box<Expr>>,
    pub span: Span,
}

/// `condition ? when_true : when_false`
#[derive(Debug, Clone, PartialEq)]
pub struct TernaryExpr {
    pub condition: Box<Expr>,
    pub when_true: Box<Expr>,
    pub when_false: Box<Expr>,
    /// The `?`, so a diagnostic can point at the operator and not the whole
    /// expression.
    pub op_span: Span,
    pub span: Span,
}

/// `i++`, `++i`, `i--` or `--i` in expression position.
#[derive(Debug, Clone, PartialEq)]
pub struct IncrementExpr {
    /// The place being incremented. It must be assignable and mutable.
    pub target: AssignTarget,
    pub op: IncrementOp,
    pub fix: IncrementFix,
    pub op_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementOp {
    /// `++`
    Increment,
    /// `--`
    Decrement,
}

impl IncrementOp {
    pub const fn as_str(self) -> &'static str {
        match self {
            IncrementOp::Increment => "++",
            IncrementOp::Decrement => "--",
        }
    }

    /// The binary operation the increment stands for.
    pub const fn as_binary(self) -> BinaryOp {
        match self {
            IncrementOp::Increment => BinaryOp::Add,
            IncrementOp::Decrement => BinaryOp::Sub,
        }
    }
}

/// Which value the expression produces, per `ZIRK_LANGUAGE_SPEC.md` section 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncrementFix {
    /// `++i`: the operand is updated first and the expression is the new value.
    Prefix,
    /// `i++`: the expression is the previous value and the operand is updated
    /// afterwards.
    Postfix,
}

/// One acquisition clause of a (possibly grouped) `match ... with`.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchAcquisition {
    pub expr: Box<Expr>,
    pub binding: Ident,
}

/// `match scrutinee { arms }`, or `match scrutinee with binding { arms }`
/// (roadmap Phase 4c, `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section
/// 4): the `with` form owns whichever arm's pattern binds `with_binding`'s
/// name and closes it — calling its `Resource<E>` `close()` — on every exit
/// from that arm, normal or not.
///
/// The grouped form stores its clauses in `acquisitions`, a single `body` and
/// an optional `error` branch. When `acquisitions` is empty the single form is
/// used and `scrutinee`/`with_binding`/`arms` carry the original structure.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub with_binding: Option<Ident>,
    pub arms: Vec<MatchArm>,
    pub acquisitions: Vec<MatchAcquisition>,
    pub body: Option<Box<ArmBody>>,
    pub error: Option<Box<MatchArm>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: ArmBody,
    pub span: Span,
}

/// The body of a `match` arm.
#[derive(Debug, Clone, PartialEq)]
pub enum ArmBody {
    Expr(Expr),
    Block(Block),
}

impl ArmBody {
    pub fn span(&self) -> Span {
        match self {
            ArmBody::Expr(e) => e.span(),
            ArmBody::Block(b) => b.span,
        }
    }
}

/// A pattern of a `match` arm.
///
/// Destructuring and patterns over unions need records and unions, which are
/// Phase 3. This phase covers what a closed set of constructors needs in order
/// for exhaustiveness to mean something.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard(Span),
    /// A name, which binds the scrutinee.
    Binding(Ident),
    /// `Direction.North`
    Variant(VariantPattern),
    Int(IntLit),
    Str(StrLit),
    Bool(BoolLit),
    Null(NullLit),
    Tuple(TuplePattern),
    /// `re'pattern' name?` — matches a `String` scrutinee against the regex
    /// and optionally binds the resulting `Regex.Match` to `name`.
    Regex(RegexPattern),
}

/// `re'pattern' name?` in pattern position (roadmap Phase 7, `zirk-regex`
/// "Regex integration in match statements").
#[derive(Debug, Clone, PartialEq)]
pub struct RegexPattern {
    /// The source pattern, escapes intact (same payload a `RegexLit`
    /// expression carries).
    pub pattern: String,
    /// The name the `Regex.Match` binds to inside the arm, if written.
    pub binding: Option<Ident>,
    pub span: Span,
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(s) => *s,
            Pattern::Binding(i) => i.span,
            Pattern::Variant(v) => v.span,
            Pattern::Int(l) => l.span,
            Pattern::Str(l) => l.span,
            Pattern::Bool(l) => l.span,
            Pattern::Null(l) => l.span,
            Pattern::Tuple(t) => t.span,
            Pattern::Regex(r) => r.span,
        }
    }

    /// Whether the pattern matches every possible value.
    pub fn is_irrefutable(&self) -> bool {
        match self {
            Pattern::Wildcard(_) | Pattern::Binding(_) => true,
            Pattern::Tuple(t) => t.elements.iter().all(|p| p.is_irrefutable()),
            _ => false,
        }
    }

    /// The name bound by a simple binding pattern, if any.
    pub fn binding_name(&self) -> Option<&str> {
        match self {
            Pattern::Binding(i) => Some(&i.name),
            _ => None,
        }
    }
}

/// `Direction.North` in expression position.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantExpr {
    pub enum_name: Ident,
    pub variant: Ident,
    pub span: Span,
}

/// `Direction.North` in pattern position, or `Shape.Circle(radius)`
/// destructuring an algebraic variant's associated data.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantPattern {
    pub enum_name: Ident,
    pub variant: Ident,
    /// One pattern per associated field, in declaration order. Empty for a
    /// traditional variant or a bare algebraic variant with no data.
    pub bindings: Vec<Pattern>,
    pub span: Span,
}

/// `(a, b)` in pattern position — one pattern per element, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct TuplePattern {
    pub elements: Vec<Pattern>,
    pub span: Span,
}

/// A lambda, which is a function value.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaExpr {
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    pub body: Box<LambdaBody>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expr(Expr),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntLit {
    /// Value already normalized, without separators. Stored as `i128` so
    /// overflow of the destination type can be detected in `zirk-sema` instead
    /// of being lost while parsing.
    pub value: i128,
    pub span: Span,
}

/// A duration literal, stored as nanoseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurationLit {
    pub nanos: i64,
    pub span: Span,
}

/// A regex literal, stored as the source pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexLit {
    pub pattern: String,
    pub span: Span,
}

/// A fractional or scientific literal (roadmap Phase 3b).
///
/// The text is kept as written rather than parsed to `f64` here, for the
/// same reason `zirk_lexer::NumberLit` does: the checker chooses the width,
/// and a `Float128` value may exceed what a host `f64` represents exactly —
/// parsing here would decide, in the wrong layer, a precision the target
/// type may not lose. `zirk-codegen-llvm` parses this same text once, at
/// its own destination width, through LLVM's own literal parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloatLit {
    /// Digits, decimal point and exponent as written, without `_` or a width
    /// suffix.
    pub text: String,
    /// The explicit suffix, such as the `f32` of `1.5f32`, if the literal
    /// wrote one.
    pub width: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrLit {
    /// Contents with escapes already resolved.
    pub value: String,
    pub span: Span,
}

/// A character literal (roadmap Phase 3b): exactly one Unicode grapheme.
///
/// Whether the content is exactly one grapheme is not decided by the lexer
/// (`zirk_lexer::character()`'s own doc comment) — that is `zirk-sema`'s
/// `check_char_literal`, which needs Unicode segmentation the lexer does not
/// have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharLit {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoolLit {
    pub value: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub operand: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-x`
    Neg,
    /// `!x`
    Not,
    /// `~x`
    BitNot,
}

impl UnaryOp {
    pub const fn as_str(self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::BitNot => "~",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
    /// Location of the operator, so type diagnostics can point at it.
    pub op_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    /// `is`, which asks whether two references name the same instance.
    ///
    /// Distinct from `==`: that one compares content, and two objects may hold
    /// the same content while being different objects — which is exactly the
    /// difference this operator exists to make visible.
    Is,
    /// `??`, which yields the left operand unless it is null.
    Coalesce,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
}

impl BinaryOp {
    pub const fn as_str(self) -> &'static str {
        use BinaryOp::*;
        match self {
            Is => "is",
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Rem => "%",
            Eq => "==",
            NotEq => "!=",
            Lt => "<",
            LtEq => "<=",
            Gt => ">",
            GtEq => ">=",
            And => "&&",
            Or => "||",
            Coalesce => "??",
            BitAnd => "&",
            BitOr => "|",
            BitXor => "^",
            Shl => "<<",
            Shr => ">>",
        }
    }

    /// Whether the operator produces a `Boolean` regardless of its operand
    /// types.
    pub const fn yields_boolean(self) -> bool {
        use BinaryOp::*;
        matches!(self, Eq | NotEq | Lt | LtEq | Gt | GtEq | And | Or)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    /// What is being called.
    ///
    /// An expression rather than a name, because a closure held in a variable
    /// is called the same way a function is.
    pub callee: Box<Expr>,
    pub args: Vec<Arg>,
    pub span: Span,
}

/// One argument of a call.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    /// Present for `name: value`, which matches by name instead of position.
    pub name: Option<Ident>,
    pub value: Expr,
    pub span: Span,
}

impl Arg {
    /// A plain positional argument.
    pub fn positional(value: Expr) -> Self {
        Self {
            name: None,
            span: value.span(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrintlnExpr {
    pub arg: Box<Expr>,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Span = Span::new(0, 1);

    #[test]
    fn every_statement_exposes_its_span() {
        let stmts = vec![
            Stmt::Return(ReturnStmt {
                value: None,
                span: S,
            }),
            Stmt::Block(Block {
                statements: vec![],
                span: S,
            }),
            Stmt::Expr(ExprStmt {
                expr: Expr::Bool(BoolLit {
                    value: true,
                    span: S,
                }),
                span: S,
            }),
        ];

        for stmt in stmts {
            assert_eq!(stmt.span(), S);
        }
    }

    #[test]
    fn every_expression_exposes_its_span() {
        let exprs = vec![
            Expr::Int(IntLit { value: 1, span: S }),
            Expr::Str(StrLit {
                value: "a".into(),
                span: S,
            }),
            Expr::Bool(BoolLit {
                value: false,
                span: S,
            }),
            Expr::Path(Ident::new("x", S)),
        ];

        for expr in exprs {
            assert_eq!(expr.span(), S);
        }
    }

    #[test]
    fn comparison_and_logical_operators_yield_boolean() {
        for op in [
            BinaryOp::Eq,
            BinaryOp::Lt,
            BinaryOp::GtEq,
            BinaryOp::And,
            BinaryOp::Or,
        ] {
            assert!(op.yields_boolean(), "for `{}`", op.as_str());
        }
    }

    #[test]
    fn arithmetic_operators_do_not_yield_boolean() {
        for op in [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Rem] {
            assert!(!op.yields_boolean(), "for `{}`", op.as_str());
        }
    }
}
