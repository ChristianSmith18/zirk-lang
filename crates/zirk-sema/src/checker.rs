//! Name resolution, type checking and flow analysis.
//!
//! The checker does not stop at the first error: it assigns [`Type::UNKNOWN`]
//! to whatever it cannot determine and keeps going, so one compilation reports
//! every problem instead of one per run. `Unknown` is compatible with
//! everything, which is what prevents one real error from producing a dozen
//! derived ones.

use crate::codes;
use crate::scope::{Binding, ParamInfo, Scopes, Signature};
use crate::types::{Base, EnumType, FnType, Type, TypeNames, describe, pending_type};
use std::collections::HashMap;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};

/// The result of checking, for later stages.
#[derive(Debug, Default)]
pub struct CheckedProgram {
    pub functions: HashMap<String, Signature>,
    /// Declared enums, indexed by the id their [`Base::Enum`] carries.
    pub enums: Vec<EnumType>,
    /// Function types, indexed by the id their [`Base::Function`] carries.
    pub fn_types: Vec<FnType>,
    /// What each lambda captures, keyed by the lambda's span.
    ///
    /// Lowering needs this to build the environment, and a span identifies a
    /// lambda uniquely without inventing a numbering the parser would have to
    /// maintain.
    pub lambdas: HashMap<Span, LambdaInfo>,
    /// The resolved type of each `match` scrutinee, keyed by the match's span.
    ///
    /// Lowering compares against a discriminant for enums and against the value
    /// itself otherwise, and that choice is made here.
    pub matches: HashMap<Span, Type>,
}

/// What the checker learned about one lambda.
#[derive(Debug, Clone)]
pub struct LambdaInfo {
    /// Names captured from the enclosing scope, in a stable order.
    pub captures: Vec<Capture>,
    pub fn_type: u32,
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub name: String,
    pub ty: Type,
}

/// Checks a program, accumulating diagnostics in the sink.
pub fn check(source: &SourceFile, program: &Program, sink: &mut DiagnosticSink) -> CheckedProgram {
    Checker::new(source, sink).run(program)
}

/// Names types for diagnostics, given the checker's tables.
struct Names<'t> {
    enums: &'t [EnumType],
    fn_types: &'t [FnType],
}

impl TypeNames for Names<'_> {
    fn enum_name(&self, id: u32) -> String {
        self.enums
            .get(id as usize)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "<enum>".into())
    }

    fn function_type(&self, id: u32) -> String {
        let Some(f) = self.fn_types.get(id as usize) else {
            return "<function>".into();
        };
        let params: Vec<String> = f.params.iter().map(|t| describe(*t, self)).collect();
        format!("({}) => {}", params.join(", "), describe(f.returns, self))
    }
}

struct Checker<'a> {
    source: &'a SourceFile,
    sink: &'a mut DiagnosticSink,
    scopes: Scopes,
    functions: HashMap<String, Signature>,
    enums: Vec<EnumType>,
    fn_types: Vec<FnType>,
    lambdas: HashMap<Span, LambdaInfo>,
    matches: HashMap<Span, Type>,
    /// Return type of the function or lambda being checked.
    current_return: Type,
    /// How many loops enclose the statement being checked.
    ///
    /// Zero means `break` and `continue` have nothing to jump out of.
    loop_depth: u32,
    /// Captures collected for the lambda being checked, innermost last.
    capture_stack: Vec<Vec<Capture>>,
}

impl<'a> Checker<'a> {
    fn new(source: &'a SourceFile, sink: &'a mut DiagnosticSink) -> Self {
        Self {
            source,
            sink,
            scopes: Scopes::new(),
            functions: HashMap::new(),
            enums: Vec::new(),
            fn_types: Vec::new(),
            lambdas: HashMap::new(),
            matches: HashMap::new(),
            current_return: Type::VOID,
            loop_depth: 0,
            capture_stack: Vec::new(),
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

    /// Renders a type for a diagnostic.
    fn name(&self, ty: Type) -> String {
        describe(
            ty,
            &Names {
                enums: &self.enums,
                fn_types: &self.fn_types,
            },
        )
    }

    // --- Program ----------------------------------------------------------

    fn run(mut self, program: &Program) -> CheckedProgram {
        // Module resolution is its own pass and does not exist yet, so an
        // `import` would bind no name and every use of it would be reported as
        // undeclared — an error that points at the wrong place.
        for import in &program.imports {
            self.not_lowered(
                import.span,
                "`import`",
                "put everything in one file until module resolution lands",
            );
        }
        for use_decl in &program.uses {
            self.not_lowered(
                use_decl.span,
                "`use`",
                "name what you need explicitly until module resolution lands",
            );
        }

        // Enums come first: a signature may name one.
        for e in &program.enums {
            self.declare_enum(e);
        }

        // Signatures next, so a function can call another declared later.
        for f in &program.functions {
            self.declare_function(f);
        }

        self.check_entrypoint(program);

        self.scopes.push();
        for f in &program.functions {
            self.check_function(f);
        }
        self.scopes.pop();

        CheckedProgram {
            functions: self.functions,
            enums: self.enums,
            fn_types: self.fn_types,
            lambdas: self.lambdas,
            matches: self.matches,
        }
    }

    fn declare_enum(&mut self, decl: &EnumDecl) {
        if self.enums.iter().any(|e| e.name == decl.name.name) {
            self.error(
                codes::DUPLICATE_DECLARATION,
                decl.name.span,
                format!("enum `{}` is already defined", decl.name.name),
                "a previous definition exists in this file",
                Some("rename one of the two".into()),
            );
            return;
        }

        let mut variants: Vec<String> = Vec::new();
        for variant in &decl.variants {
            if variants.contains(&variant.name) {
                self.error(
                    codes::DUPLICATE_DECLARATION,
                    variant.span,
                    format!("variant `{}` is repeated", variant.name),
                    format!("`{}` already declares it", decl.name.name),
                    None,
                );
                continue;
            }
            variants.push(variant.name.clone());
        }

        self.enums.push(EnumType {
            name: decl.name.name.clone(),
            variants,
        });
    }

    fn declare_function(&mut self, f: &FnDecl) {
        let params = f
            .params
            .iter()
            .map(|p| self.resolve_param(p))
            .collect::<Vec<_>>();

        let signature = Signature {
            name: f.name.name.clone(),
            params,
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

    /// Resolves one parameter, applying the rule that `name?: T` is nullable.
    fn resolve_param(&mut self, p: &Param) -> ParamInfo {
        let mut ty = self.resolve_type(&p.ty);

        // An optional parameter may be absent, and absence is `null`.
        if p.optional {
            ty = ty.as_nullable();
        }

        ParamInfo {
            name: p.name.name.clone(),
            ty,
            optional: p.optional,
            has_default: p.default.is_some(),
            variadic: p.variadic,
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

        if !main.params.is_empty() || main.returns != Type::VOID {
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
        let base = if let Some(ty) = Type::from_name(&reference.name) {
            Some(ty)
        } else {
            self.enums
                .iter()
                .position(|e| e.name == reference.name)
                .map(|i| Type::of(Base::Enum(i as u32)))
        };

        if let Some(ty) = base {
            // `Void?` has no value to be absent, so it is not a type.
            if reference.nullable && ty == Type::VOID {
                self.error(
                    codes::UNKNOWN_TYPE,
                    reference.span,
                    "`Void?` is not a type",
                    "`Void` is the absence of a value, so it cannot also be absent",
                    Some("write `Void`".into()),
                );
                return Type::VOID;
            }

            return if reference.nullable {
                ty.as_nullable()
            } else {
                ty
            };
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
                Some("the available types are Void, Int32, Boolean, String and declared enums".into()),
            ),
            None => self.error(
                codes::UNKNOWN_TYPE,
                reference.span,
                format!("unknown type: `{}`", reference.name),
                "no type with that name exists in the language",
                Some("the available types are Void, Int32, Boolean, String and declared enums".into()),
            ),
        }

        Type::UNKNOWN
    }

    /// Interns a function type, returning the id its `Base::Function` carries.
    fn intern_fn_type(&mut self, fn_type: FnType) -> u32 {
        if let Some(index) = self.fn_types.iter().position(|f| *f == fn_type) {
            return index as u32;
        }
        self.fn_types.push(fn_type);
        (self.fn_types.len() - 1) as u32
    }

    // --- Functions --------------------------------------------------------

    fn check_function(&mut self, f: &FnDecl) {
        let signature = self.functions.get(&f.name.name).cloned();
        self.current_return = signature
            .as_ref()
            .map(|s| s.returns)
            .unwrap_or(Type::UNKNOWN);

        // A function body cannot see the locals of another: the barrier is what
        // makes a name from outside a capture rather than a plain read.
        self.scopes.push_function();

        let params = signature.map(|s| s.params).unwrap_or_default();
        for (param, info) in f.params.iter().zip(&params) {
            // The default is evaluated at the call site, but it is checked here
            // where the declared type is known.
            if let Some(default) = &param.default {
                let actual = self.check_expr(default);
                self.expect_assignable(info.ty, actual, default.span(), "the default value");
            }

            self.scopes.declare(Binding {
                name: info.name.clone(),
                ty: info.ty,
                mutability: Mutability::Immutable,
                span: param.name.span,
                initialized: true,
            });
        }

        let always_returns = self.check_block(&f.body);
        self.scopes.pop();

        // Every path of a non-`Void` function must return a value.
        if self.current_return != Type::VOID && !self.current_return.is_unknown() && !always_returns
        {
            let returns = self.name(self.current_return);
            self.error(
                codes::MISSING_RETURN,
                f.body.span,
                format!("not every path of `{}` returns a value", f.name.name),
                format!("the function declares `{returns}` as its return type"),
                Some("add a `return` at the end of the function".into()),
            );
        }
    }

    /// Checks a block and reports whether every path through it returns.
    fn check_block(&mut self, block: &Block) -> bool {
        self.scopes.push();
        let returns = self.check_statements(&block.statements);
        self.scopes.pop();
        returns
    }

    fn check_statements(&mut self, statements: &[Stmt]) -> bool {
        let mut always_returns = false;
        for stmt in statements {
            if self.check_stmt(stmt) {
                always_returns = true;
            }
        }
        always_returns
    }

    // --- Statements -------------------------------------------------------

    /// Returns `true` when the statement guarantees the function has returned.
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
            Stmt::Loop(s) => self.check_loop(s),
            Stmt::ForIn(s) => {
                self.check_for_in(s);
                false
            }
            Stmt::Break(s) => {
                self.check_jump(s, "break");
                // `break` leaves the loop, and the loop's own analysis accounts
                // for it: from the enclosing block's point of view nothing is
                // guaranteed to have returned.
                false
            }
            Stmt::Continue(s) => {
                self.check_jump(s, "continue");
                false
            }
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
                let span = stmt.init.as_ref().map(|e| e.span()).unwrap_or(stmt.span);
                self.expect_assignable(declared, actual, span, "the initial value");
                declared
            }
            (Some(declared), None) => declared,
            // Inference is allowed where it is unambiguous
            // (`LANGUAGE_SPEC` section 2). With one integer type in the subset,
            // a literal always is.
            (None, Some(inferred)) => {
                // `mut x = null;` gives no base to infer: nullability alone is
                // not a type.
                if matches!(inferred.base, Base::Null) {
                    self.error(
                        codes::UNKNOWN_TYPE,
                        stmt.span,
                        format!("cannot infer the type of `{}`", stmt.name.name),
                        "`null` alone does not say which type is absent",
                        Some(format!("annotate it, as in `{}: String?`", stmt.name.name)),
                    );
                    Type::UNKNOWN
                } else {
                    inferred
                }
            }
            (None, None) => Type::UNKNOWN,
        };

        // `Void` has no representable value, so no variable can hold it.
        if ty == Type::VOID {
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

        let Some(resolved) = self.scopes.resolve(&stmt.target.name) else {
            self.undeclared(&stmt.target);
            return;
        };

        // A closure captures by value, so writing to a captured name would
        // silently update a copy. Decision D2.
        if resolved.captured {
            let line = self.source.location(resolved.binding.span).line;
            self.error(
                codes::CAPTURED_MUTATION,
                stmt.target.span,
                format!("cannot reassign `{}` inside a closure", stmt.target.name),
                format!("it is captured from line {line}, and capture is by value"),
                Some(
                    "a closure captures immutable values; return the new value instead of writing to it"
                        .into(),
                ),
            );
            return;
        }

        if resolved.binding.mutability == Mutability::Immutable {
            let line = self.source.location(resolved.binding.span).line;
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

        self.expect_assignable(
            resolved.binding.ty,
            value,
            stmt.value.span(),
            "the assigned value",
        );
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

    /// `while`, `loop` and the three-clause `for`, which share a node.
    fn check_loop(&mut self, stmt: &LoopStmt) -> bool {
        // The initializer belongs to a scope enclosing the loop: `for (mut i =
        // 0; ...)` declares `i` for the condition and step too, but not after.
        self.scopes.push();

        if let Some(init) = &stmt.init {
            self.check_stmt(init);
        }

        if let Some(condition) = &stmt.condition {
            let ty = self.check_expr(condition);
            let context = format!("the condition of a `{}`", stmt.kind.as_str());
            self.expect_boolean(ty, condition.span(), &context);
        }

        self.loop_depth += 1;
        let body_returns = self.check_block(&stmt.body);
        self.loop_depth -= 1;

        if let Some(step) = &stmt.step {
            self.check_stmt(step);
        }

        self.scopes.pop();

        // Only a `loop` with no exit guarantees the code after it is
        // unreachable, which is what lets a function end on one without a
        // `return`. A conditional loop may run zero times.
        stmt.condition.is_none() && !self.contains_break(&stmt.body) && body_returns
    }

    /// Whether a block can leave its loop through `break`.
    ///
    /// Nested loops are skipped: their `break` belongs to them.
    fn contains_break(&self, block: &Block) -> bool {
        fn in_stmt(stmt: &Stmt) -> bool {
            match stmt {
                Stmt::Break(_) => true,
                Stmt::Block(b) => b.statements.iter().any(in_stmt),
                Stmt::If(i) => {
                    i.then_branch.statements.iter().any(in_stmt)
                        || match &i.else_branch {
                            Some(ElseBranch::Block(b)) => b.statements.iter().any(in_stmt),
                            Some(ElseBranch::If(nested)) => in_stmt(&Stmt::If((**nested).clone())),
                            None => false,
                        }
                }
                // A `break` inside a nested loop exits that one, not this.
                Stmt::Loop(_) | Stmt::ForIn(_) => false,
                _ => false,
            }
        }

        block.statements.iter().any(in_stmt)
    }

    fn check_for_in(&mut self, stmt: &ForInStmt) {
        let iterable = self.check_expr(&stmt.iterable);
        let element = self.element_type(iterable, stmt.iterable.span());

        self.scopes.push();
        self.scopes.declare(Binding {
            name: stmt.binding.name.clone(),
            ty: element,
            // The loop variable is rebound each iteration, not assigned.
            mutability: Mutability::Immutable,
            span: stmt.binding.span,
            initialized: true,
        });

        self.loop_depth += 1;
        self.check_block(&stmt.body);
        self.loop_depth -= 1;

        self.scopes.pop();
    }

    /// What `for ... in` binds for each element.
    ///
    /// The set is closed: there is no user-extensible iteration protocol until
    /// traits arrive in Phase 3. Decision D3.
    fn element_type(&mut self, iterable: Type, span: Span) -> Type {
        match iterable.base {
            Base::Range => Type::INT32,
            // Iterating a `String` yields one-character strings: `Char` is a
            // Phase 3 type, and inventing one here would be inventing a type
            // the spec places elsewhere.
            Base::String => {
                self.not_lowered(
                    span,
                    "iterating a `String`",
                    "iterate a range, as in `for i in 0..n`",
                );
                Type::STRING
            }
            Base::Unknown => Type::UNKNOWN,
            _ => {
                let name = self.name(iterable);
                self.error(
                    codes::NOT_ITERABLE,
                    span,
                    format!("`{name}` cannot be iterated"),
                    "this phase iterates ranges and strings only",
                    Some(
                        "iteration over your own types arrives with the traits of Phase 3".into(),
                    ),
                );
                Type::UNKNOWN
            }
        }
    }

    fn check_jump(&mut self, stmt: &JumpStmt, word: &str) {
        if self.loop_depth == 0 {
            self.error(
                codes::JUMP_OUTSIDE_LOOP,
                stmt.span,
                format!("`{word}` is not inside a loop"),
                format!("`{word}` only makes sense within `for`, `while` or `loop`"),
                None,
            );
        }
    }

    fn check_return(&mut self, stmt: &ReturnStmt) {
        let actual = match &stmt.value {
            Some(expr) => self.check_expr(expr),
            None => Type::VOID,
        };

        if self.current_return == Type::VOID && stmt.value.is_some() {
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
            let expected = self.name(self.current_return);
            let found = self.name(actual);
            self.error(
                codes::TYPE_MISMATCH,
                stmt.value.as_ref().map(|e| e.span()).unwrap_or(stmt.span),
                "the returned type does not match the signature",
                format!("the function declares `{expected}` and this returns `{found}`"),
                None,
            );
        }
    }

    // --- Expressions ------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Int(lit) => self.check_int_literal(lit),
            Expr::Str(_) => Type::STRING,
            Expr::Bool(_) => Type::BOOLEAN,
            Expr::Null(_) => Type::NULL,
            Expr::Path(ident) => self.check_path(ident),
            Expr::Unary(e) => self.check_unary(e),
            Expr::Binary(e) => self.check_binary(e),
            Expr::Call(e) => self.check_call(e),
            Expr::Range(e) => self.check_range(e),
            Expr::If(e) => self.check_if_expr(e),
            Expr::Match(e) => self.check_match(e, true),
            Expr::Lambda(e) => self.check_lambda(e),
            Expr::Variant(e) => self.check_variant(e),
            Expr::Println(e) => {
                self.check_expr(&e.arg);
                Type::VOID
            }
        }
    }

    /// Integer literals are `Int32`, the only integer type in the subset.
    ///
    /// The value is kept in `i128` by the lexer precisely so overflow of the
    /// destination type is detected here rather than lost while parsing
    /// (`LANGUAGE_SPEC` section 3: ordinary overflow is a controlled error).
    fn check_int_literal(&mut self, lit: &IntLit) -> Type {
        if let Some((min, max)) = Type::INT32.integer_range()
            && (lit.value < min || lit.value > max)
        {
            self.error(
                codes::INTEGER_OUT_OF_RANGE,
                lit.span,
                format!("the literal {} does not fit in Int32", lit.value),
                format!("Int32 admits values from {min} to {max}"),
                None,
            );
            return Type::UNKNOWN;
        }
        Type::INT32
    }

    fn check_path(&mut self, ident: &Ident) -> Type {
        // A bare function name is a value: that is what lets a function be
        // passed where a closure is expected.
        if let Some(signature) = self.functions.get(&ident.name).cloned()
            && self.scopes.lookup(&ident.name).is_none()
        {
            let id = self.intern_fn_type(FnType {
                params: signature.param_types(),
                returns: signature.returns,
            });
            return Type::of(Base::Function(id));
        }

        let Some(resolved) = self.scopes.resolve(&ident.name) else {
            self.undeclared(ident);
            return Type::UNKNOWN;
        };

        if !resolved.binding.initialized {
            let line = self.source.location(resolved.binding.span).line;
            self.error(
                codes::USE_BEFORE_INITIALIZATION,
                ident.span,
                format!("`{}` is read before it holds a value", ident.name),
                format!("it was declared without an initializer on line {line}"),
                Some("assign a value to it before reading it".into()),
            );
        }

        // Reading a name from outside this lambda is what makes it a capture.
        if resolved.captured {
            self.record_capture(&resolved.binding);
        }

        resolved.binding.ty
    }

    fn record_capture(&mut self, binding: &Binding) {
        let Some(captures) = self.capture_stack.last_mut() else {
            return;
        };
        if captures.iter().any(|c| c.name == binding.name) {
            return;
        }
        captures.push(Capture {
            name: binding.name.clone(),
            ty: binding.ty,
        });
    }

    fn check_unary(&mut self, expr: &UnaryExpr) -> Type {
        let operand = self.check_expr(&expr.operand);

        match expr.op {
            UnaryOp::Neg => {
                if !operand.accepts(Type::INT32) {
                    let found = self.name(operand);
                    self.error(
                        codes::TYPE_MISMATCH,
                        expr.span,
                        "the `-` operator requires a number",
                        format!("it was applied to a value of type {found}"),
                        None,
                    );
                    return Type::UNKNOWN;
                }
                Type::INT32
            }
            UnaryOp::Not => {
                self.expect_boolean(operand, expr.span, "the operand of `!`");
                Type::BOOLEAN
            }
        }
    }

    fn check_binary(&mut self, expr: &BinaryExpr) -> Type {
        let left = self.check_expr(&expr.left);
        let right = self.check_expr(&expr.right);

        use BinaryOp::*;
        match expr.op {
            // `??` is the one operator whose whole purpose is nullability, so
            // it is checked before the rules that reject nullable operands.
            Coalesce => self.check_coalesce(left, right, expr),

            // Logical operators accept booleans only: there is no numeric
            // truthiness (`LANGUAGE_SPEC` section 3).
            And | Or => {
                self.expect_boolean(left, expr.left.span(), "the left operand");
                self.expect_boolean(right, expr.right.span(), "the right operand");
                Type::BOOLEAN
            }

            // Equality is structural and requires both sides to share a type.
            Eq | NotEq => {
                self.expect_same(left, right, expr);
                Type::BOOLEAN
            }

            // Comparison only makes sense on numbers in this subset.
            Lt | LtEq | Gt | GtEq => {
                self.expect_numeric(left, expr.left.span(), expr.op);
                self.expect_numeric(right, expr.right.span(), expr.op);
                Type::BOOLEAN
            }

            Add | Sub | Mul | Div | Rem => {
                self.expect_numeric(left, expr.left.span(), expr.op);
                self.expect_numeric(right, expr.right.span(), expr.op);
                if left.is_unknown() || right.is_unknown() {
                    Type::UNKNOWN
                } else {
                    Type::INT32
                }
            }
        }
    }

    fn check_coalesce(&mut self, left: Type, right: Type, expr: &BinaryExpr) -> Type {
        if !left.admits_null() && !left.is_unknown() {
            let found = self.name(left);
            self.error(
                codes::REDUNDANT_OPERATOR,
                expr.op_span,
                "`??` on a value that is never null",
                format!("the left operand has type `{found}`, which always holds a value"),
                Some("remove the `??` and its fallback".into()),
            );
            return left;
        }

        let Some(unified) = left.without_null().unify(right) else {
            let l = self.name(left);
            let r = self.name(right);
            self.error(
                codes::TYPE_MISMATCH,
                expr.op_span,
                format!("`??` cannot combine {l} with {r}"),
                "both sides must share a type: the fallback replaces the value",
                None,
            );
            return Type::UNKNOWN;
        };

        unified
    }

    fn check_range(&mut self, expr: &RangeExpr) -> Type {
        let start = self.check_expr(&expr.start);
        let end = self.check_expr(&expr.end);

        self.expect_numeric_value(start, expr.start.span(), "the start of a range");
        self.expect_numeric_value(end, expr.end.span(), "the end of a range");

        Type::RANGE
    }

    /// `if` used where a value is expected. Decision D7.
    fn check_if_expr(&mut self, stmt: &IfStmt) -> Type {
        let condition = self.check_expr(&stmt.condition);
        self.expect_boolean(condition, stmt.condition.span(), "the condition of an `if`");

        let then_type = self.check_block_value(&stmt.then_branch);

        let else_type = match &stmt.else_branch {
            Some(ElseBranch::Block(b)) => self.check_block_value(b),
            Some(ElseBranch::If(nested)) => self.check_if_expr(nested),
            None => {
                self.error(
                    codes::IF_WITHOUT_ELSE,
                    stmt.span,
                    "an `if` used as a value needs an `else`",
                    "without the alternative branch there is no value when the condition is false",
                    Some("add `else { ... }`, or use the `if` as a statement".into()),
                );
                return Type::UNKNOWN;
            }
        };

        match then_type.unify(else_type) {
            Some(ty) => ty,
            None => {
                let t = self.name(then_type);
                let e = self.name(else_type);
                self.error(
                    codes::TYPE_MISMATCH,
                    stmt.span,
                    "the branches of the `if` produce different types",
                    format!("one branch produces {t} and the other {e}"),
                    Some("both branches must agree for the `if` to be a value".into()),
                );
                Type::UNKNOWN
            }
        }
    }

    /// The value a block produces, which is that of its last statement.
    ///
    /// Semicolons are optional in Zirk (`LANGUAGE_SPEC` section 1), so unlike
    /// Rust their presence cannot mark the tail: the last statement is the
    /// value when it is an expression, and there is no value otherwise.
    fn check_block_value(&mut self, block: &Block) -> Type {
        self.scopes.push();

        let (last, rest) = match block.statements.split_last() {
            Some(split) => split,
            None => {
                self.scopes.pop();
                self.error(
                    codes::TYPE_MISMATCH,
                    block.span,
                    "an empty block produces no value",
                    "a block used as a value ends in an expression",
                    None,
                );
                return Type::UNKNOWN;
            }
        };

        self.check_statements(rest);

        let ty = match last {
            Stmt::Expr(e) => self.check_expr(&e.expr),
            other => {
                self.check_stmt(other);
                self.error(
                    codes::TYPE_MISMATCH,
                    other.span(),
                    "this block produces no value",
                    "a block used as a value ends in an expression",
                    Some("make the last statement the value the block produces".into()),
                );
                Type::UNKNOWN
            }
        };

        self.scopes.pop();
        ty
    }

    fn check_match(&mut self, expr: &MatchExpr, as_value: bool) -> Type {
        let scrutinee = self.check_expr(&expr.scrutinee);
        self.matches.insert(expr.span, scrutinee);

        let mut arm_types: Vec<(Type, Span)> = Vec::new();
        let mut covered: Vec<String> = Vec::new();
        let mut has_wildcard = false;

        for arm in &expr.arms {
            self.check_pattern(&arm.pattern, scrutinee, &mut covered, &mut has_wildcard);

            self.scopes.push();
            // A binding pattern names the scrutinee inside its arm.
            if let Pattern::Binding(ident) = &arm.pattern {
                self.scopes.declare(Binding {
                    name: ident.name.clone(),
                    ty: scrutinee,
                    mutability: Mutability::Immutable,
                    span: ident.span,
                    initialized: true,
                });
            }

            let ty = match &arm.body {
                ArmBody::Expr(e) => self.check_expr(e),
                ArmBody::Block(b) if as_value => self.check_block_value(b),
                ArmBody::Block(b) => {
                    self.check_block(b);
                    Type::VOID
                }
            };
            self.scopes.pop();

            arm_types.push((ty, arm.body.span()));
        }

        self.check_exhaustive(expr, scrutinee, &covered, has_wildcard);

        if !as_value {
            return Type::VOID;
        }

        // Every arm must agree, the same way both branches of an `if` must.
        let mut result = arm_types.first().map(|(t, _)| *t).unwrap_or(Type::UNKNOWN);
        for (ty, span) in arm_types.iter().skip(1) {
            match result.unify(*ty) {
                Some(unified) => result = unified,
                None => {
                    let a = self.name(result);
                    let b = self.name(*ty);
                    self.error(
                        codes::TYPE_MISMATCH,
                        *span,
                        "the arms of the `match` produce different types",
                        format!("an earlier arm produces {a} and this one {b}"),
                        Some("every arm must agree for the `match` to be a value".into()),
                    );
                    return Type::UNKNOWN;
                }
            }
        }

        result
    }

    fn check_pattern(
        &mut self,
        pattern: &Pattern,
        scrutinee: Type,
        covered: &mut Vec<String>,
        has_wildcard: &mut bool,
    ) {
        match pattern {
            Pattern::Wildcard(_) | Pattern::Binding(_) => *has_wildcard = true,
            Pattern::Int(lit) => {
                let ty = self.check_int_literal(lit);
                self.expect_pattern_type(scrutinee, ty, lit.span);
            }
            Pattern::Str(lit) => self.expect_pattern_type(scrutinee, Type::STRING, lit.span),
            Pattern::Bool(lit) => self.expect_pattern_type(scrutinee, Type::BOOLEAN, lit.span),
            Pattern::Null(lit) => {
                if !scrutinee.admits_null() && !scrutinee.is_unknown() {
                    let name = self.name(scrutinee);
                    self.error(
                        codes::TYPE_MISMATCH,
                        lit.span,
                        format!("`{name}` is never null"),
                        "this arm could never be taken",
                        None,
                    );
                }
            }
            Pattern::Variant(v) => self.check_variant_pattern(v, scrutinee, covered),
        }
    }

    fn check_variant_pattern(
        &mut self,
        pattern: &VariantPattern,
        scrutinee: Type,
        covered: &mut Vec<String>,
    ) {
        let Some(index) = self
            .enums
            .iter()
            .position(|e| e.name == pattern.enum_name.name)
        else {
            self.error(
                codes::UNKNOWN_TYPE,
                pattern.enum_name.span,
                format!("`{}` is not a declared enum", pattern.enum_name.name),
                "a variant pattern names the enum it belongs to",
                None,
            );
            return;
        };

        let enum_type = &self.enums[index];
        if !enum_type.variants.contains(&pattern.variant.name) {
            let enum_name = enum_type.name.clone();
            let known = enum_type.variants.join(", ");
            self.error(
                codes::UNKNOWN_VARIANT,
                pattern.variant.span,
                format!("`{enum_name}` has no variant `{}`", pattern.variant.name),
                format!("its variants are: {known}"),
                None,
            );
            return;
        }

        self.expect_pattern_type(scrutinee, Type::of(Base::Enum(index as u32)), pattern.span);

        if !covered.contains(&pattern.variant.name) {
            covered.push(pattern.variant.name.clone());
        }
    }

    fn expect_pattern_type(&mut self, scrutinee: Type, pattern: Type, span: Span) {
        // The scrutinee's nullability is irrelevant here: a `null` pattern is
        // what covers that half.
        if scrutinee.without_null().accepts(pattern) || scrutinee.is_unknown() {
            return;
        }

        let s = self.name(scrutinee);
        let p = self.name(pattern);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("this pattern matches {p}, but the value is {s}"),
            "a pattern must have the type of what it matches against",
            None,
        );
    }

    fn check_exhaustive(
        &mut self,
        expr: &MatchExpr,
        scrutinee: Type,
        covered: &[String],
        has_wildcard: bool,
    ) {
        if has_wildcard || scrutinee.is_unknown() {
            return;
        }

        // An enum is the only closed set of values this phase has, so it is the
        // only case where exhaustiveness can be met without `_`.
        if let Base::Enum(id) = scrutinee.base
            && !scrutinee.nullable
            && let Some(enum_type) = self.enums.get(id as usize)
        {
            let missing: Vec<String> = enum_type
                .variants
                .iter()
                .filter(|v| !covered.contains(v))
                .cloned()
                .collect();

            if missing.is_empty() {
                return;
            }

            let name = enum_type.name.clone();
            self.error(
                codes::NON_EXHAUSTIVE_MATCH,
                expr.span,
                format!("the `match` does not cover every case of `{name}`"),
                format!("these variants have no arm: {}", missing.join(", ")),
                Some("add an arm for each, or `_` for the rest".into()),
            );
            return;
        }

        let name = self.name(scrutinee);
        self.error(
            codes::NON_EXHAUSTIVE_MATCH,
            expr.span,
            format!("the `match` does not cover every value of `{name}`"),
            "only an enum has a set of values small enough to cover one by one",
            Some("end with `_ => ...` to cover the rest".into()),
        );
    }

    /// Reports a construct this phase parses and checks but cannot yet compile.
    ///
    /// The grammar and the type rules landed before the lowering did, and the
    /// gap is real: rather than let it reach a backend that would panic, the
    /// construct is reported the same way Phase 1 reported what it had not
    /// implemented (design D6). Each of these disappears as its lowering lands.
    fn not_lowered(&mut self, span: Span, what: &str, instead: &str) {
        self.error(
            codes::NOT_LOWERED,
            span,
            format!("{what} is not compilable yet"),
            "the grammar and the type rules for it exist, but its code generation does not",
            Some(instead.to_string()),
        );
    }

    /// `Direction.North` in expression position.
    fn check_variant(&mut self, expr: &VariantExpr) -> Type {
        let Some(index) = self.enums.iter().position(|e| e.name == expr.enum_name.name) else {
            self.error(
                codes::UNKNOWN_TYPE,
                expr.enum_name.span,
                format!("`{}` is not a declared enum", expr.enum_name.name),
                "only an enum has variants to name in this phase",
                None,
            );
            return Type::UNKNOWN;
        };

        let enum_type = &self.enums[index];
        if !enum_type.variants.contains(&expr.variant.name) {
            let enum_name = enum_type.name.clone();
            let known = enum_type.variants.join(", ");
            self.error(
                codes::UNKNOWN_VARIANT,
                expr.variant.span,
                format!("`{enum_name}` has no variant `{}`", expr.variant.name),
                format!("its variants are: {known}"),
                None,
            );
            return Type::UNKNOWN;
        }

        Type::of(Base::Enum(index as u32))
    }

    fn check_lambda(&mut self, expr: &LambdaExpr) -> Type {
        self.not_lowered(
            expr.span,
            "a lambda",
            "declare a `fn` at the top level and call it by name",
        );
        let params: Vec<ParamInfo> = expr.params.iter().map(|p| self.resolve_param(p)).collect();
        let returns = self.resolve_type(&expr.return_type);

        let enclosing_return = self.current_return;
        // A lambda body is not inside the enclosing loop: `break` in it has
        // nothing to leave.
        let enclosing_depth = self.loop_depth;
        self.current_return = returns;
        self.loop_depth = 0;
        self.capture_stack.push(Vec::new());

        self.scopes.push_function();
        for (param, info) in expr.params.iter().zip(&params) {
            if let Some(default) = &param.default {
                let actual = self.check_expr(default);
                self.expect_assignable(info.ty, actual, default.span(), "the default value");
            }
            self.scopes.declare(Binding {
                name: info.name.clone(),
                ty: info.ty,
                mutability: Mutability::Immutable,
                span: param.name.span,
                initialized: true,
            });
        }

        match &*expr.body {
            LambdaBody::Expr(e) => {
                let actual = self.check_expr(e);
                self.expect_assignable(returns, actual, e.span(), "the lambda body");
            }
            LambdaBody::Block(b) => {
                let always_returns = self.check_block(b);
                if returns != Type::VOID && !returns.is_unknown() && !always_returns {
                    let name = self.name(returns);
                    self.error(
                        codes::MISSING_RETURN,
                        b.span,
                        "not every path of the lambda returns a value",
                        format!("it declares `{name}` as its return type"),
                        Some("add a `return` at the end".into()),
                    );
                }
            }
        }
        self.scopes.pop();

        let captures = self.capture_stack.pop().unwrap_or_default();
        self.current_return = enclosing_return;
        self.loop_depth = enclosing_depth;

        // A capture of an inner lambda is also a capture of the outer one when
        // the name lives further out still.
        for capture in &captures {
            if let Some(resolved) = self.scopes.resolve(&capture.name)
                && resolved.captured
            {
                self.record_capture(&resolved.binding);
            }
        }

        let fn_type = self.intern_fn_type(FnType {
            params: params.iter().map(|p| p.ty).collect(),
            returns,
        });

        self.lambdas.insert(
            expr.span,
            LambdaInfo {
                captures,
                fn_type,
            },
        );

        Type::of(Base::Function(fn_type))
    }

    fn check_call(&mut self, expr: &CallExpr) -> Type {
        // A call to a name resolves against the declared functions first, so a
        // named function keeps its optional and variadic parameters. Calling a
        // value goes through its function type, which has none of that.
        if let Expr::Path(callee) = &*expr.callee
            && self.scopes.lookup(&callee.name).is_none()
            && let Some(signature) = self.functions.get(&callee.name).cloned()
        {
            return self.check_direct_call(expr, &signature);
        }

        let callee = self.check_expr(&expr.callee);
        let Base::Function(id) = callee.base else {
            if !callee.is_unknown() {
                let name = self.name(callee);
                self.error(
                    codes::NOT_CALLABLE,
                    expr.callee.span(),
                    format!("`{name}` is not a function"),
                    "only a function or a closure can be called",
                    None,
                );
            }
            for arg in &expr.args {
                self.check_expr(&arg.value);
            }
            return Type::UNKNOWN;
        };

        let fn_type = self.fn_types[id as usize].clone();
        let arguments: Vec<Type> = expr.args.iter().map(|a| self.check_expr(&a.value)).collect();

        // A closure value has no parameter names to match against.
        for arg in &expr.args {
            if let Some(name) = &arg.name {
                self.error(
                    codes::UNKNOWN_ARGUMENT_NAME,
                    name.span,
                    "a closure cannot take named arguments",
                    "only a declared function carries the parameter names",
                    Some("pass the arguments by position".into()),
                );
            }
        }

        if arguments.len() != fn_type.params.len() {
            self.error(
                codes::WRONG_ARGUMENT_COUNT,
                expr.span,
                format!(
                    "this closure expects {} argument(s) and received {}",
                    fn_type.params.len(),
                    arguments.len()
                ),
                "the number of arguments must match the closure's type",
                None,
            );
            return fn_type.returns;
        }

        for (index, (actual, expected)) in arguments.iter().zip(&fn_type.params).enumerate() {
            self.expect_assignable(
                *expected,
                *actual,
                expr.args[index].span,
                &format!("argument {}", index + 1),
            );
        }

        fn_type.returns
    }

    /// A call to a declared function, where names and defaults apply.
    fn check_direct_call(&mut self, expr: &CallExpr, signature: &Signature) -> Type {
        let name = signature.name.clone();
        let slots = self.match_arguments(expr, signature);

        for (index, slot) in slots.iter().enumerate() {
            let Some(param) = signature.params.get(index) else {
                continue;
            };

            match slot {
                ArgSlot::Given { ty, span } => {
                    let expected = if param.variadic { param.ty } else { param.ty };
                    self.expect_assignable(
                        expected,
                        *ty,
                        *span,
                        &format!("argument `{}`", param.name),
                    );
                }
                ArgSlot::Variadic(items) => {
                    for (ty, span) in items {
                        self.expect_assignable(
                            param.ty,
                            *ty,
                            *span,
                            &format!("a value of `...{}`", param.name),
                        );
                    }
                }
                // Absent and defaultable: nothing to check, the default was
                // checked where it was declared.
                ArgSlot::Default | ArgSlot::Absent => {}
                ArgSlot::Missing => {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        expr.span,
                        format!("`{name}` is missing an argument for `{}`", param.name),
                        "the parameter is neither optional nor has a default value",
                        None,
                    );
                }
            }
        }

        signature.returns
    }

    /// Assigns each argument of a call to the parameter it fills.
    ///
    /// Named arguments match by name, the rest by position, and what is left
    /// over goes to the variadic. Decision D4: this happens here so the IR only
    /// ever sees a call of fixed arity.
    fn match_arguments(&mut self, expr: &CallExpr, signature: &Signature) -> Vec<ArgSlot> {
        let mut slots: Vec<ArgSlot> = signature
            .params
            .iter()
            .map(|p| {
                if p.variadic {
                    ArgSlot::Variadic(Vec::new())
                } else if p.has_default {
                    ArgSlot::Default
                } else if p.optional {
                    ArgSlot::Absent
                } else {
                    ArgSlot::Missing
                }
            })
            .collect();

        let mut next_position = 0usize;

        for arg in &expr.args {
            let ty = self.check_expr(&arg.value);

            if let Some(name) = &arg.name {
                let Some(index) = signature.params.iter().position(|p| p.name == name.name) else {
                    let known: Vec<&str> =
                        signature.params.iter().map(|p| p.name.as_str()).collect();
                    self.error(
                        codes::UNKNOWN_ARGUMENT_NAME,
                        name.span,
                        format!("`{}` has no parameter named `{}`", signature.name, name.name),
                        format!("its parameters are: {}", known.join(", ")),
                        None,
                    );
                    continue;
                };

                if matches!(slots[index], ArgSlot::Given { .. }) {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        name.span,
                        format!("`{}` was already given a value", name.name),
                        "a parameter receives at most one argument",
                        None,
                    );
                    continue;
                }

                slots[index] = ArgSlot::Given {
                    ty,
                    span: arg.span,
                };
                continue;
            }

            // Positional arguments fill the next unnamed slot, and everything
            // past the last fixed parameter belongs to the variadic.
            while next_position < slots.len()
                && matches!(slots[next_position], ArgSlot::Given { .. })
            {
                next_position += 1;
            }

            match slots.get_mut(next_position) {
                Some(ArgSlot::Variadic(items)) => items.push((ty, arg.span)),
                Some(slot) => {
                    *slot = ArgSlot::Given {
                        ty,
                        span: arg.span,
                    };
                    next_position += 1;
                }
                None => {
                    self.error(
                        codes::WRONG_ARGUMENT_COUNT,
                        arg.span,
                        format!("`{}` received too many arguments", signature.name),
                        format!("it declares {} parameter(s)", signature.params.len()),
                        None,
                    );
                }
            }
        }

        slots
    }

    // --- Shared checks ----------------------------------------------------

    /// Checks that a value may be stored where a type is expected.
    fn expect_assignable(&mut self, expected: Type, actual: Type, span: Span, context: &str) {
        if expected.accepts(actual) {
            return;
        }

        let e = self.name(expected);
        let a = self.name(actual);

        // Narrowing from `T?` to `T` is the one mismatch with a specific fix.
        let help = if actual.admits_null() && !expected.admits_null() {
            Some(format!(
                "the value may be absent: use `?? <fallback>` to provide a `{e}`"
            ))
        } else {
            Some(format!("{context} is `{a}` and `{e}` was expected"))
        };

        self.error(
            codes::TYPE_MISMATCH,
            span,
            "incompatible types",
            format!("there is no implicit conversion from {a} to {e}"),
            help,
        );
    }

    fn expect_boolean(&mut self, actual: Type, span: Span, context: &str) {
        if actual.accepts(Type::BOOLEAN) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("{context} must be Boolean"),
            format!("a value of type {found} was found"),
            // The most frequent mistake is expecting truthiness from another
            // language, so the help names the comparison explicitly.
            Some("there is no numeric truthiness: write an explicit comparison".into()),
        );
    }

    fn expect_numeric(&mut self, actual: Type, span: Span, op: BinaryOp) {
        if actual.accepts(Type::INT32) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("the `{}` operator requires numbers", op.as_str()),
            format!("a value of type {found} was found"),
            None,
        );
    }

    fn expect_numeric_value(&mut self, actual: Type, span: Span, context: &str) {
        if actual.accepts(Type::INT32) {
            return;
        }

        let found = self.name(actual);
        self.error(
            codes::TYPE_MISMATCH,
            span,
            format!("{context} must be Int32"),
            format!("a value of type {found} was found"),
            None,
        );
    }

    fn expect_same(&mut self, left: Type, right: Type, expr: &BinaryExpr) {
        if left.unify(right).is_some() {
            return;
        }

        let l = self.name(left);
        let r = self.name(right);
        self.error(
            codes::TYPE_MISMATCH,
            expr.op_span,
            format!("cannot compare {l} with {r}"),
            "structural equality requires both sides to share a type",
            None,
        );
    }

    fn undeclared(&mut self, ident: &Ident) {
        self.error(
            codes::UNDECLARED_NAME,
            ident.span,
            format!("`{}` is not declared", ident.name),
            "no variable, parameter or function with that name is in scope",
            None,
        );
    }
}

/// What fills one parameter slot at a call site.
enum ArgSlot {
    /// An argument was supplied.
    Given { ty: Type, span: Span },
    /// Nothing was supplied and the declaration provides a default.
    Default,
    /// Nothing was supplied and the parameter is optional, so it is null.
    Absent,
    /// Nothing was supplied and the parameter required something.
    Missing,
    /// The values collected by a `...` parameter.
    Variadic(Vec<(Type, Span)>),
}
