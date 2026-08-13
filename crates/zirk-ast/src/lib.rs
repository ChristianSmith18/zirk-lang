//! # zirk-ast
//!
//! **Responsabilidad:** definir los nodos del árbol de sintaxis de Zirk y su
//! correspondencia con ubicaciones del source.
//!
//! **Límite:** este crate solo define la *forma* del árbol. No lo construye
//! (eso es `zirk-parser`), no lo interpreta ni lo valida (eso es `zirk-sema`).
//!
//! Según `ZIRK_COMPILER_SPEC.md` sección 3, la AST semántica interna es privada
//! y puede evolucionar con el compilador. Las herramientas externas no la
//! consumen directamente: usan la Syntax API pública, que llega en una fase
//! posterior y es un contrato distinto de este crate.
//!
//! # Forma del árbol
//!
//! Se usa un tipo por construcción en vez de un árbol homogéneo de nodos
//! genéricos. La decisión y su alternativa están en el `design.md` de
//! `fase-1-pipeline-minimo`, decisión D1: un árbol homogéneo estilo CST sería
//! mejor para el LSP de la Fase 9, pero impone complejidad en todas las capas
//! ocho fases antes de que rinda. Cuando llegue, se introduce como capa
//! adicional bajo esta AST, no en su lugar.
//!
//! **Todo nodo lleva su span, sin excepción.** Un nodo sin ubicación no puede
//! producir el diagnóstico que exige `ZIRK_COMPILER_SPEC.md` sección 8.

use zirk_diagnostics::Span;

/// Un archivo fuente parseado.
///
/// En esta fase un programa es un único archivo con declaraciones de función:
/// los módulos multi-archivo llegan en la Fase 2.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub functions: Vec<FnDecl>,
    pub span: Span,
}

/// Declaración de función.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    /// Tipo de retorno. Obligatorio en esta fase.
    pub return_type: TypeRef,
    pub body: Block,
    pub span: Span,
}

/// Parámetro de una función.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeRef,
    pub span: Span,
}

/// Identificador con su ubicación.
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

/// Referencia sintáctica a un tipo.
///
/// Es lo que el usuario **escribió**, no el tipo resuelto: `zirk-sema` lo
/// convierte en un tipo del sistema y emite el diagnóstico si no existe.
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

/// Bloque de sentencias con su propio scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

/// Mutabilidad de una declaración de variable.
///
/// `inmut::strict` es de una fase posterior: acá solo existen las dos formas
/// del subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// `mut`: permite reasignar.
    Mutable,
    /// `inmut`: inmoviliza la referencia.
    Immutable,
}

/// Sentencia.
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
    /// Una expresión evaluada por su efecto, como una llamada.
    Expr(ExprStmt),
    /// Un bloque anidado.
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

/// Declaración de variable.
#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub mutability: Mutability,
    pub name: Ident,
    /// Anotación explícita. Ausente cuando el tipo se infiere.
    pub ty: Option<TypeRef>,
    pub init: Option<Expr>,
    pub span: Span,
}

/// Reasignación de una variable existente.
#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub target: Ident,
    pub value: Expr,
    pub span: Span,
}

/// Condicional como sentencia.
///
/// `if` como expresión es de la Fase 2, aunque el spec lo permita.
#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

/// Rama alternativa de un condicional.
///
/// Se distingue el bloque del encadenamiento para que `else if` no pierda su
/// forma en el árbol.
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

/// Expresión.
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
    /// Forma sintáctica especial reconocida por el compilador mientras no
    /// existan módulos ni stdlib. Es deuda deliberada, documentada en la
    /// decisión D4 del design, que se retira en la Fase 7.
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
    /// Valor ya normalizado, sin separadores. Se guarda en `i128` para poder
    /// detectar el desbordamiento del tipo destino en `zirk-sema` en vez de
    /// perderlo al parsear.
    pub value: i128,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrLit {
    /// Contenido con los escapes ya resueltos.
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
    /// Ubicación del operador, para señalarlo en los diagnósticos de tipos.
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

    /// Si el operador produce un `Boolean` sin importar el tipo de sus
    /// operandos.
    pub const fn produce_booleano(self) -> bool {
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
    fn toda_sentencia_expone_su_span() {
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
    fn toda_expresion_expone_su_span() {
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
    fn los_comparadores_y_logicos_producen_booleano() {
        for op in [
            BinaryOp::Eq,
            BinaryOp::Lt,
            BinaryOp::GtEq,
            BinaryOp::And,
            BinaryOp::Or,
        ] {
            assert!(op.produce_booleano(), "para `{}`", op.as_str());
        }
    }

    #[test]
    fn los_aritmeticos_no_producen_booleano() {
        for op in [BinaryOp::Add, BinaryOp::Sub, BinaryOp::Mul, BinaryOp::Rem] {
            assert!(!op.produce_booleano(), "para `{}`", op.as_str());
        }
    }
}
