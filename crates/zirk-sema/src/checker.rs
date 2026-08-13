//! Name resolution, type checking and flow analysis.
//!
//! The checker does not stop at the first error: it assigns [`Type::Unknown`]
//! to whatever it cannot determine and keeps going, so one compilation reports
//! every problem instead of one per run. `Unknown` is compatible with
//! everything, which is what prevents one real error from producing a dozen
//! derived ones.

use crate::codes;
use crate::scope::{Binding, Scopes, Signature};
use crate::types::{Type, pending_type};
use std::collections::HashMap;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};

/// The result of checking: the signatures found, for later stages.
#[derive(Debug, Default)]
pub struct CheckedProgram {
    pub functions: HashMap<String, Signature>,
}

/// Checks a program, accumulating diagnostics in the sink.
pub fn check(source: &SourceFile, program: &Program, sink: &mut DiagnosticSink) -> CheckedProgram {
    Checker::new(source, sink).run(program)
}

struct Checker<'a> {
    source: &'a SourceFile,
    sink: &'a mut DiagnosticSink,
    scopes: Scopes,
    functions: HashMap<String, Signature>,
    /// Return type of the function being checked.
    current_return: Type,
}

impl<'a> Checker<'a> {
    fn new(source: &'a SourceFile, sink: &'a mut DiagnosticSink) -> Self {
        Self {
            source,
            sink,
            scopes: Scopes::new(),
            functions: HashMap::new(),
            current_return: Type::Void,
        }
    }

    // --- Diagnostics ------------------------------------------------------

    fn error(
        &mut self,
        code: Code,
        span: Span,
        message: impl Into<String>,
        cause: impl Into<String>,
        help: Option<String>,
    ) {
        let mut d = Diagnostic::error(code, message)
            .at(self.source.location(span))
            .with_snippet(self.source.snippet(span))
            .with_cause(cause);
        if let Some(help) = help {
            d = d.with_help(help);
        }
        self.sink.emit(d);
    }

    // --- Program ----------------------------------------------------------

    fn run(mut self, program: &Program) -> CheckedProgram {
        // Signatures are collected first so a function can call another
        // declared later in the file.
        for f in &program.functions {
            let signature = Signature {
                name: f.name.name.clone(),
                params: f.params.iter().map(|p| self.resolve_type(&p.ty)).collect(),
                returns: self.resolve_type(&f.return_type),
                span: f.name.span,
            };

            if let Some(previous) = self.functions.get(&signature.name) {
                let line = self.source.location(previous.span).line;
                self.error(
                    codes::DUPLICATE_FUNCTION,
                    f.name.span,
                    format!("function `{}` is already defined", f.name.name),
                    format!("a previous definition exists on line {line}"),
                    Some("rename one of the two: there is no overloading".into()),
                );
            } else {
                self.functions.insert(signature.name.clone(), signature);
            }
        }

        self.check_entrypoint(program);

        for f in &program.functions {
            self.check_function(f);
        }

        CheckedProgram {
            functions: self.functions,
        }
    }

    /// `ZIRK_RUNTIME_SPEC.md` section 2 requires a `main` entrypoint.
    fn check_entrypoint(&mut self, program: &Program) {
        let Some(main) = self.functions.get("main").cloned() else {
            self.error(
                codes::MISSING_ENTRYPOINT,
                Span::empty(0),
                "the program has no entrypoint",
                "no `main` function was found in the file",
                Some("add `fn main(): Void { }`".into()),
            );
            return;
        };

        if !main.params.is_empty() || main.returns != Type::Void {
            let span = program
                .functions
                .iter()
                .find(|f| f.name.name == "main")
                .map(|f| f.name.span)
                .unwrap_or(main.span);

            self.error(
                codes::INVALID_ENTRYPOINT,
                span,
                "`main` has the wrong signature",
                "the entrypoint takes no parameters and returns `Void` in this phase",
                Some("write `fn main(): Void { ... }`".into()),
            );
        }
    }

    fn resolve_type(&mut self, reference: &TypeRef) -> Type {
        if let Some(ty) = Type::from_name(&reference.name) {
            return ty;
        }

        match pending_type(&reference.name) {
            Some(pending) => self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                format!("type `{}` is not implemented yet", reference.name),
                format!(
                    "the type exists in the language but arrives in Phase {}",
                    pending.phase
                ),
                Some("the available types are Void, Int32, Boolean and String".into()),
            ),
            None => self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                format!("unknown type: `{}`", reference.name),
                "no type with that name exists in the language",
                Some("the available types are Void, Int32, Boolean and String".into()),
            ),
        }

        Type::Unknown
    }

    // --- Functions --------------------------------------------------------

    fn check_function(&mut self, f: &FnDecl) {
        let signature = self.functions.get(&f.name.name).cloned();
        self.current_return = signature.map(|s| s.returns).unwrap_or(Type::Unknown);

        self.scopes.push();

        for param in &f.params {
            let ty = self.resolve_type(&param.ty);
            self.scopes.declare(Binding {
                name: param.name.name.clone(),
                ty,
                mutability: Mutability::Immutable,
                span: param.name.span,
                initialized: true,
            });
        }

        let always_returns = self.check_block(&f.body);
        self.scopes.pop();

        // Every path of a non-`Void` function must return a value.
        if self.current_return != Type::Void
            && self.current_return != Type::Unknown
            && !always_returns
        {
            self.error(
                codes::MISSING_RETURN,
                f.body.span,
                format!("not every path of `{}` returns a value", f.name.name),
                format!(
                    "the function declares `{}` as its return type",
                    self.current_return.as_str()
                ),
                Some("add a `return` at the end of the function".into()),
            );
        }
    }

    /// Checks a block and reports whether every path through it returns.
    fn check_block(&mut self, block: &Block) -> bool {
        self.scopes.push();
        let mut always_returns = false;

        for stmt in &block.statements {
            if self.check_stmt(stmt) {
                always_returns = true;
            }
        }

        self.scopes.pop();
        always_returns
    }

    // --- Statements -------------------------------------------------------

    /// Returns `true` when the statement guarantees a return.
    fn check_stmt(&mut self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Let(s) => {
                self.check_let(s);
                false
            }
            Stmt::Assign(s) => {
                self.check_assign(s);
                false
            }
            Stmt::If(s) => self.check_if(s),
            Stmt::Return(s) => {
                self.check_return(s);
                true
            }
            Stmt::Expr(s) => {
                self.check_expr(&s.expr);
                false
            }
            Stmt::Block(b) => self.check_block(b),
        }
    }

    fn check_let(&mut self, stmt: &LetStmt) {
        let annotated = stmt.ty.as_ref().map(|t| self.resolve_type(t));
        let initializer = stmt.init.as_ref().map(|e| self.check_expr(e));

        let ty = match (annotated, initializer) {
            (Some(declared), Some(actual)) => {
                if !declared.accepts(actual) {
                    let span = stmt.init.as_ref().map(|e| e.span()).unwrap_or(stmt.span);
                    self.error(
                        codes::TYPE_MISMATCH,
                        span,
                        "incompatible types",
                        format!(
                            "there is no implicit conversion from {} to {}",
                            actual.as_str(),
                            declared.as_str()
                        ),
                        Some(format!(
                            "the declared type is `{}`; the value is `{}`",
                            declared.as_str(),
                            actual.as_str()
                        )),
                    );
                }
                declared
            }
            (Some(declared), None) => declared,
            // Inference is allowed where it is unambiguous
            // (`LANGUAGE_SPEC` section 2). With one integer type in the subset,
            // a literal always is.
            (None, Some(inferred)) => inferred,
            (None, None) => Type::Unknown,
        };

        // `Void` has no representable value, so no variable can hold it.
        if ty == Type::Void {
            self.error(
                codes::VOID_VARIABLE,
                stmt.span,
                format!("variable `{}` cannot be of type Void", stmt.name.name),
                "`Void` represents the absence of a value, so it cannot be stored",
                None,
            );
        }

        self.scopes.declare(Binding {
            name: stmt.name.name.clone(),
            ty,
            mutability: stmt.mutability,
            span: stmt.name.span,
            initialized: stmt.init.is_some(),
        });
    }

    fn check_assign(&mut self, stmt: &AssignStmt) {
        let value = self.check_expr(&stmt.value);

        let Some(binding) = self.scopes.lookup(&stmt.target.name).cloned() else {
            self.undeclared(&stmt.target);
            return;
        };

        if binding.mutability == Mutability::Immutable {
            let line = self.source.location(binding.span).line;
            self.error(
                codes::ASSIGN_TO_IMMUTABLE,
                stmt.target.span,
                format!("cannot reassign `{}`", stmt.target.name),
                format!("it was declared with `inmut` on line {line}"),
                Some(format!(
                    "declare it with `mut {}` if it has to change",
                    stmt.target.name
                )),
            );
        }

        if !binding.ty.accepts(value) {
            self.error(
                codes::TYPE_MISMATCH,
                stmt.value.span(),
                "incompatible types",
                format!(
                    "there is no implicit conversion from {} to {}",
                    value.as_str(),
                    binding.ty.as_str()
                ),
                None,
            );
        }

        self.scopes.mark_initialized(&stmt.target.name);
    }

    fn check_if(&mut self, stmt: &IfStmt) -> bool {
        let condition = self.check_expr(&stmt.condition);
        self.expect_boolean(condition, stmt.condition.span(), "the condition of an `if`");

        let then_returns = self.check_block(&stmt.then_branch);

        // A conditional only guarantees a return when both branches do.
        match &stmt.else_branch {
            Some(ElseBranch::Block(b)) => then_returns & self.check_block(b),
            Some(ElseBranch::If(nested)) => then_returns & self.check_if(nested),
            None => false,
        }
    }

    fn check_return(&mut self, stmt: &ReturnStmt) {
        let actual = match &stmt.value {
            Some(expr) => self.check_expr(expr),
            None => Type::Void,
        };

        if self.current_return == Type::Void && stmt.value.is_some() {
            self.error(
                codes::TYPE_MISMATCH,
                stmt.value.as_ref().map(|e| e.span()).unwrap_or(stmt.span),
                "a Void function cannot return a value",
                "the function declares `Void` as its return type",
                Some("write `return;` with no value".into()),
            );
            return;
        }

        if !self.current_return.accepts(actual) {
            self.error(
                codes::TYPE_MISMATCH,
                stmt.value.as_ref().map(|e| e.span()).unwrap_or(stmt.span),
                "the returned type does not match the signature",
                format!(
                    "the function declares `{}` and this returns `{}`",
                    self.current_return.as_str(),
                    actual.as_str()
                ),
                None,
            );
        }
    }

    // --- Expressions ------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Int(lit) => self.check_int_literal(lit),
            Expr::Str(_) => Type::String,
            Expr::Bool(_) => Type::Boolean,
            Expr::Path(ident) => self.check_path(ident),
            Expr::Unary(e) => self.check_unary(e),
            Expr::Binary(e) => self.check_binary(e),
            Expr::Call(e) => self.check_call(e),
            Expr::Println(e) => {
                self.check_expr(&e.arg);
                Type::Void
            }
        }
    }

    /// Integer literals are `Int32`, the only integer type in the subset.
    ///
    /// The value is kept in `i128` by the lexer precisely so overflow of the
    /// destination type is detected here rather than lost while parsing
    /// (`LANGUAGE_SPEC` section 3: ordinary overflow is a controlled error).
    fn check_int_literal(&mut self, lit: &IntLit) -> Type {
        if let Some((min, max)) = Type::Int32.integer_range()
            && (lit.value < min || lit.value > max)
        {
            self.error(
                codes::INTEGER_OUT_OF_RANGE,
                lit.span,
                format!("the literal {} does not fit in Int32", lit.value),
                format!("Int32 admits values from {min} to {max}"),
                None,
            );
            return Type::Unknown;
        }
        Type::Int32
    }

    fn check_path(&mut self, ident: &Ident) -> Type {
        let Some(binding) = self.scopes.lookup(&ident.name).cloned() else {
            self.undeclared(ident);
            return Type::Unknown;
        };

        if !binding.initialized {
            let line = self.source.location(binding.span).line;
            self.error(
                codes::USE_BEFORE_INITIALIZATION,
                ident.span,
                format!("`{}` is read before it holds a value", ident.name),
                format!("it was declared without an initializer on line {line}"),
                Some("assign a value to it before reading it".into()),
            );
        }

        binding.ty
    }

    fn check_unary(&mut self, expr: &UnaryExpr) -> Type {
        let operand = self.check_expr(&expr.operand);

        match expr.op {
            UnaryOp::Neg => {
                if !operand.accepts(Type::Int32) {
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.span,
                        "the `-` operator requires a number",
                        format!("it was applied to a value of type {}", operand.as_str()),
                        None,
                    );
                    return Type::Unknown;
                }
                Type::Int32
            }
            UnaryOp::Not => {
                self.expect_boolean(operand, expr.span, "the operand of `!`");
                Type::Boolean
            }
        }
    }

    fn check_binary(&mut self, expr: &BinaryExpr) -> Type {
        let left = self.check_expr(&expr.left);
        let right = self.check_expr(&expr.right);

        use BinaryOp::*;
        match expr.op {
            // Logical operators accept booleans only: there is no numeric
            // truthiness (`LANGUAGE_SPEC` section 3).
            And | Or => {
                self.expect_boolean(left, expr.left.span(), "the left operand");
                self.expect_boolean(right, expr.right.span(), "the right operand");
                Type::Boolean
            }

            // Equality is structural and requires both sides to share a type.
            Eq | NotEq => {
                self.expect_same(left, right, expr);
                Type::Boolean
            }

            // Comparison only makes sense on numbers in this subset.
            Lt | LtEq | Gt | GtEq => {
                self.expect_numeric(left, expr.left.span(), expr.op);
                self.expect_numeric(right, expr.right.span(), expr.op);
                Type::Boolean
            }

            Add | Sub | Mul | Div | Rem => {
                self.expect_numeric(left, expr.left.span(), expr.op);
                self.expect_numeric(right, expr.right.span(), expr.op);
                if left == Type::Unknown || right == Type::Unknown {
                    Type::Unknown
                } else {
                    Type::Int32
                }
            }
        }
    }

    fn check_call(&mut self, expr: &CallExpr) -> Type {
        let arguments: Vec<Type> = expr.args.iter().map(|a| self.check_expr(a)).collect();

        let Some(signature) = self.functions.get(&expr.callee.name).cloned() else {
            self.error(
                codes::UNDECLARED_NAME,
                expr.callee.span,
                format!("function `{}` is not declared", expr.callee.name),
                "no function with that name exists in the file",
                None,
            );
            return Type::Unknown;
        };

        if arguments.len() != signature.params.len() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!(
                    "`{}` expects {} argument(s) and received {}",
                    expr.callee.name,
                    signature.params.len(),
                    arguments.len()
                ),
                "the number of arguments must match the signature",
                None,
            );
            return signature.returns;
        }

        for (index, (actual, expected)) in arguments.iter().zip(&signature.params).enumerate() {
            if !expected.accepts(*actual) {
                self.error(
                    codes::TYPE_MISMATCH,
                    expr.args[index].span(),
                    format!("argument {} has the wrong type", index + 1),
                    format!(
                        "`{}` expects {} and received {}",
                        expr.callee.name,
                        expected.as_str(),
                        actual.as_str()
                    ),
                    None,
                );
            }
        }

        signature.returns
    }

    // --- Shared checks ----------------------------------------------------

    fn expect_boolean(&mut self, actual: Type, span: Span, context: &str) {
        if actual.accepts(Type::Boolean) {
            return;
        }

        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("{context} must be Boolean"),
            format!("a value of type {} was found", actual.as_str()),
            // The most frequent mistake is expecting truthiness from another
            // language, so the help names the comparison explicitly.
            Some("there is no numeric truthiness: write an explicit comparison".into()),
        );
    }

    fn expect_numeric(&mut self, actual: Type, span: Span, op: BinaryOp) {
        if actual.accepts(Type::Int32) {
            return;
        }

        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("the `{}` operator requires numbers", op.as_str()),
            format!("a value of type {} was found", actual.as_str()),
            None,
        );
    }

    fn expect_same(&mut self, left: Type, right: Type, expr: &BinaryExpr) {
        if left.accepts(right) {
            return;
        }

        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            format!("cannot compare {} with {}", left.as_str(), right.as_str()),
            "structural equality requires both sides to share a type",
            None,
        );
    }

    fn undeclared(&mut self, ident: &Ident) {
        self.error(
            codes::UNDECLARED_NAME,
            ident.span,
            format!("`{}` is not declared", ident.name),
            "no variable or parameter with that name is in scope",
            None,
        );
    }
}
