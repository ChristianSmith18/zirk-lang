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
/// In this phase a program is a single file of function declarations:
/// multi-file modules arrive in Phase 2.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<FnDecl>,
    pub span: Span,
}

/// Function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    /// Return type. Mandatory in this phase.
    pub return_type: TypeRef,
    pub body: Block,
    pub span: Span,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
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
    pub span: Span,
}

impl TypeRef {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
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
            Stmt::Return(s) => s.span,
            Stmt::Expr(s) => s.span,
            Stmt::Block(b) => b.span,
        }
    }
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
    pub target: Ident,
    pub value: Expr,
    pub span: Span,
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
    Path(Ident),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Call(CallExpr),
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
            Expr::Path(i) => i.span,
            Expr::Unary(e) => e.span,
            Expr::Binary(e) => e.span,
            Expr::Call(e) => e.span,
            Expr::Println(e) => e.span,
        }
    }
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
    pub callee: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
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
