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

/// `enum Direction { North, South }`
///
/// Without associated data: that is the extension Phase 3 adds, per decision
/// D1 of the design.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: Ident,
    pub variants: Vec<Ident>,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
    pub span: Span,
}

/// `class User { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name: Ident,
    /// The contracts this class says it satisfies.
    pub implements: Vec<Ident>,
    /// The class this one extends, if any.
    ///
    /// At most one: `ZIRK_LANGUAGE_SPEC.md` section 7 admits a single base
    /// class, and several contracts.
    pub extends: Option<Ident>,
    pub fields: Vec<FieldDecl>,
    /// Every `construct` the class declares. More than one is allowed when
    /// their effective signatures differ (`ZIRK_LANGUAGE_SPEC.md` section 7).
    pub constructors: Vec<ConstructDecl>,
    pub methods: Vec<MethodDecl>,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
    pub span: Span,
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
    pub params: Vec<Param>,
    pub return_type: TypeRef,
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
    pub params: Vec<Param>,
    /// Return type. Mandatory in this phase.
    pub return_type: TypeRef,
    pub body: Block,
    /// Marked `share`, so other files of the crate may import it.
    pub shared: bool,
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
    /// Written `T?`, which per `ZIRK_LANGUAGE_SPEC.md` section 4 is `T | Null`.
    pub nullable: bool,
    pub span: Span,
}

impl TypeRef {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            nullable: false,
            span,
        }
    }

    pub fn nullable(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            nullable: true,
            span,
        }
    }
}

/// A block of statements with its own scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

/// Mutability of a variable declaration.
///
/// `inmut::strict` belongs to a later phase: only the two subset forms exist
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// `mut`: allows reassignment.
    Mutable,
    /// `inmut`: freezes the reference.
    Immutable,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `mut x: Int32 = 0;`
    Let(LetStmt),
    /// `x = 1;`
    Assign(AssignStmt),
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
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(s) => s.span,
            Stmt::Assign(s) => s.span,
            Stmt::If(s) => s.span,
            Stmt::Loop(s) => s.span,
            Stmt::ForIn(s) => s.span,
            Stmt::Break(s) | Stmt::Continue(s) => s.span,
            Stmt::Return(s) => s.span,
            Stmt::Expr(s) => s.span,
            Stmt::Block(b) => b.span,
        }
    }
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
    pub name: Ident,
    /// Explicit annotation. Absent when the type is inferred.
    pub ty: Option<TypeRef>,
    pub init: Option<Expr>,
    pub span: Span,
}

/// Reassignment of an existing variable.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub target: AssignTarget,
    pub value: Expr,
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
}

impl AssignTarget {
    pub fn span(&self) -> Span {
        match self {
            AssignTarget::Name(i) => i.span,
            AssignTarget::Field(f) => f.span,
        }
    }

    /// The name being written, for a diagnostic that has to say one.
    pub fn name(&self) -> &str {
        match self {
            AssignTarget::Name(i) => &i.name,
            AssignTarget::Field(f) => &f.name.name,
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
    /// `object.field`, and `object?.field` for the safe form.
    ///
    /// Enum variants have their own node because `Direction.North` names a
    /// type rather than a value: there is no object to read a member from.
    Field(FieldExpr),
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
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Int(e) => e.span,
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
            Expr::Field(e) => e.span,
            Expr::Ternary(e) => e.span,
            Expr::Increment(e) => e.span,
            Expr::Match(e) => e.span,
            Expr::Lambda(e) => e.span,
            Expr::Variant(e) => e.span,
            Expr::Println(e) => e.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullLit {
    pub span: Span,
}

/// `start..end` or `start..=end`.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeExpr {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    /// `..=` includes the endpoint; `..` does not.
    pub inclusive: bool,
    pub span: Span,
}

/// `this`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThisExpr {
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

/// `match scrutinee { arms }`
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
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
        }
    }

    /// Whether the pattern matches every possible value.
    pub fn is_irrefutable(&self) -> bool {
        matches!(self, Pattern::Wildcard(_) | Pattern::Binding(_))
    }
}

/// `Direction.North` in expression position.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantExpr {
    pub enum_name: Ident,
    pub variant: Ident,
    pub span: Span,
}

/// `Direction.North` in pattern position.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantPattern {
    pub enum_name: Ident,
    pub variant: Ident,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrLit {
    /// Contents with escapes already resolved.
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
}

impl UnaryOp {
    pub const fn as_str(self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
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
    /// `??`, which yields the left operand unless it is null.
    Coalesce,
}

impl BinaryOp {
    pub const fn as_str(self) -> &'static str {
        use BinaryOp::*;
        match self {
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
