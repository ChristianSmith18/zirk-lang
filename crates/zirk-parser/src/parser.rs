//! Recursive-descent parser with precedence climbing.
//!
//! Precedence is implemented with a loop over levels rather than one function
//! per level: the `precedence()` table is the readable definition of what
//! `ZIRK_LANGUAGE_SPEC.md` section 4 fixes, and adding an operator means adding
//! a row rather than a function.

use crate::codes;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};
use zirk_lexer::{Keyword, Token, TokenKind};

/// Parses a sequence of tokens into a program.
///
/// Returns the tree even when there are errors: the caller decides whether to
/// continue by consulting `sink.has_errors()`.
pub fn parse(source: &SourceFile, tokens: &[Token], sink: &mut DiagnosticSink) -> Program {
    Parser::new(source, tokens, sink).parse_program()
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: &'a [Token],
    sink: &'a mut DiagnosticSink,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a SourceFile, tokens: &'a [Token], sink: &'a mut DiagnosticSink) -> Self {
        Self {
            source,
            tokens,
            sink,
            pos: 0,
        }
    }

    // --- Navigation -------------------------------------------------------

    fn peek(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|t| t.span)
            .unwrap_or_else(|| Span::empty(self.source.text().len() as u32))
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    /// Consumes the token if it matches the expected kind.
    fn eat(&mut self, expected: &TokenKind) -> bool {
        if self.peek() == expected {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, k: Keyword) -> bool {
        self.eat(&TokenKind::Keyword(k))
    }

    fn check_keyword(&self, k: Keyword) -> bool {
        self.peek() == &TokenKind::Keyword(k)
    }

    // --- Diagnostics ------------------------------------------------------

    fn build(&mut self, code: Code, span: Span, message: impl Into<String>) -> Diagnostic {
        Diagnostic::error(code, message)
            .at(self.source.location(span))
            .with_snippet(self.source.snippet(span))
    }

    fn error(
        &mut self,
        code: Code,
        span: Span,
        message: impl Into<String>,
        cause: impl Into<String>,
        help: Option<String>,
    ) {
        let mut d = self.build(code, span, message).with_cause(cause);
        if let Some(help) = help {
            d = d.with_help(help);
        }
        self.sink.emit(d);
    }

    /// Emits the not-implemented diagnostic when it applies.
    ///
    /// Returns `true` when the current token is a construct from a later phase,
    /// so the caller abandons that branch instead of trying to parse it.
    fn report_if_from_another_phase(&mut self) -> bool {
        let span = self.peek_span();

        // Modules get their own diagnostic: it is the most frequent question
        // from anyone trying the language for the first time.
        if self.check_keyword(Keyword::Import)
            || self.check_keyword(Keyword::Share)
            || self.check_keyword(Keyword::Use)
        {
            self.error(
                codes::MODULES_UNAVAILABLE,
                span,
                "multi-file modules are not available yet",
                "this version of the compiler processes a single file",
                Some(
                    "put everything in one file for now; `import` and `share` arrive in Phase 2"
                        .into(),
                ),
            );
            self.synchronize();
            return true;
        }

        let pending = match self.peek() {
            TokenKind::Keyword(k) => k.phase().map(|p| (k.as_str().to_string(), p)),
            other => other.phase().map(|p| (other.symbol().to_string(), p)),
        };

        let Some((text, phase)) = pending else {
            return false;
        };

        self.error(
            codes::NOT_IMPLEMENTED,
            span,
            format!("`{text}` is not implemented yet"),
            format!("the construct exists in the language but arrives in Phase {phase}"),
            Some("see docs/init/ZIRK_ROADMAP.md for the scope of each phase".into()),
        );
        self.synchronize();
        true
    }

    /// Advances to a point where resuming the parse makes sense.
    ///
    /// Without this, one error produces a cascade of derived errors that hides
    /// the real problem.
    fn synchronize(&mut self) {
        // Whole balanced blocks are skipped. Without this, reporting `class`
        // left the `}` of its body dangling, and that `}` produced a second
        // diagnostic —"expected a function declaration"— matching no error the
        // user actually made.
        let mut depth = 0usize;

        while !self.at_eof() {
            match self.peek() {
                TokenKind::LBrace => {
                    depth += 1;
                    self.pos += 1;
                }
                TokenKind::RBrace => {
                    if depth == 0 {
                        // It closes a block our caller opened: it belongs to them.
                        return;
                    }
                    depth -= 1;
                    self.pos += 1;
                    if depth == 0 {
                        return;
                    }
                }
                TokenKind::Semicolon if depth == 0 => {
                    self.pos += 1;
                    return;
                }
                TokenKind::Keyword(Keyword::Fn) if depth == 0 => return,
                _ => self.pos += 1,
            }
        }
    }

    /// Consumes the expected token or emits a diagnostic.
    fn expect(&mut self, expected: &TokenKind, context: &str) -> bool {
        if self.eat(expected) {
            return true;
        }

        let span = self.peek_span();
        let found = self.peek().description();
        self.error(
            codes::UNEXPECTED_TOKEN,
            span,
            format!("expected `{}` {context}", expected.symbol()),
            format!("found {found}"),
            None,
        );
        false
    }

    fn expect_identifier(&mut self, context: &str) -> Option<Ident> {
        let span = self.peek_span();
        if let TokenKind::Identifier(name) = self.peek().clone() {
            self.pos += 1;
            return Some(Ident::new(name, span));
        }

        let found = self.peek().description();
        self.error(
            codes::UNEXPECTED_TOKEN,
            span,
            format!("expected an identifier {context}"),
            format!("found {found}"),
            None,
        );
        None
    }

    // --- Program ----------------------------------------------------------

    fn parse_program(mut self) -> Program {
        let start = self.peek_span();
        let mut functions = Vec::new();

        while !self.at_eof() {
            if self.check_keyword(Keyword::Fn) {
                if let Some(f) = self.parse_fn() {
                    functions.push(f);
                }
                continue;
            }

            if self.report_if_from_another_phase() {
                continue;
            }

            let span = self.peek_span();
            let found = self.peek().description();
            self.error(
                codes::UNEXPECTED_TOKEN,
                span,
                "expected a function declaration",
                format!("found {found} at the top level of the file"),
                Some("in this phase a file contains only functions".into()),
            );
            self.synchronize();
            if !self.check_keyword(Keyword::Fn) && !self.at_eof() {
                self.pos += 1;
            }
        }

        let end = self.peek_span();
        Program {
            functions,
            span: start.to(end),
        }
    }

    fn parse_fn(&mut self) -> Option<FnDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Fn);

        let name = self.expect_identifier("after `fn`")?;

        self.expect(&TokenKind::LParen, "after the function name");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the parameter list");

        // The return type is mandatory in this phase.
        let return_type = if self.eat(&TokenKind::Colon) {
            self.parse_type()?
        } else {
            let span = self.peek_span();
            self.error(
                codes::MISSING_RETURN_TYPE,
                span,
                "missing return type on function",
                "every function declares its return type",
                Some(format!(
                    "write `fn {}(...): Void` if the function returns nothing",
                    name.name
                )),
            );
            self.synchronize();
            return None;
        };

        let body = self.parse_block()?;
        let span = start.to(body.span);

        Some(FnDecl {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();

        while !matches!(self.peek(), TokenKind::RParen) && !self.at_eof() {
            let start = self.peek_span();

            let Some(name) = self.expect_identifier("as a parameter name") else {
                break;
            };
            if !self.expect(&TokenKind::Colon, "after the parameter name") {
                break;
            }
            let Some(ty) = self.parse_type() else { break };

            let span = start.to(ty.span);
            params.push(Param { name, ty, span });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        params
    }

    fn parse_type(&mut self) -> Option<TypeRef> {
        let span = self.peek_span();
        if let TokenKind::Identifier(name) = self.peek().clone() {
            self.pos += 1;
            return Some(TypeRef::new(name, span));
        }

        let found = self.peek().description();
        self.error(
            codes::UNEXPECTED_TOKEN,
            span,
            "expected a type",
            format!("found {found}"),
            Some("the available types are Void, Int32, Boolean and String".into()),
        );
        None
    }

    // --- Statements -------------------------------------------------------

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.peek_span();

        if !self.eat(&TokenKind::LBrace) {
            let found = self.peek().description();
            self.error(
                codes::MISSING_BRACES,
                start,
                "expected a block enclosed in braces",
                format!("found {found}"),
                Some("bodies always go between `{` and `}`".into()),
            );
            return None;
        }

        let mut statements = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let before = self.pos;

            if let Some(stmt) = self.parse_stmt() {
                statements.push(stmt);
            }

            // Progress guarantee: if an error branch consumed nothing,
            // advancing prevents an infinite loop.
            if self.pos == before {
                self.pos += 1;
            }
        }

        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "to close the block");

        Some(Block {
            statements,
            span: start.to(end),
        })
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        if self.check_keyword(Keyword::Mut) || self.check_keyword(Keyword::Inmut) {
            return self.parse_let();
        }
        if self.check_keyword(Keyword::If) {
            return self.parse_if().map(Stmt::If);
        }
        if self.check_keyword(Keyword::Return) {
            return self.parse_return();
        }
        if matches!(self.peek(), TokenKind::LBrace) {
            return self.parse_block().map(Stmt::Block);
        }
        if self.report_if_from_another_phase() {
            return None;
        }

        self.parse_expr_or_assign()
    }

    fn parse_let(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        let mutability = if self.eat_keyword(Keyword::Mut) {
            Mutability::Mutable
        } else {
            self.eat_keyword(Keyword::Inmut);
            Mutability::Immutable
        };

        // `inmut::strict` belongs to a later phase.
        if matches!(self.peek(), TokenKind::ColonColon) {
            let span = self.peek_span();
            self.error(
                codes::NOT_IMPLEMENTED,
                span,
                "`inmut::strict` is not implemented yet",
                "deep immutability arrives in a later phase",
                Some("use `inmut` for now".into()),
            );
            self.synchronize();
            return None;
        }

        let name = self.expect_identifier("after `mut` or `inmut`")?;

        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.eat(&TokenKind::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        // With neither a type nor an initializer there is no way to know it.
        if ty.is_none() && init.is_none() {
            let span = start.to(name.span);
            self.error(
                codes::UNTYPED_DECLARATION,
                span,
                format!("cannot determine the type of `{}`", name.name),
                "the declaration has neither a type annotation nor an initial value",
                Some(format!(
                    "write `{}: Int32` or give it an initial value",
                    name.name
                )),
            );
        }

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Let(LetStmt {
            mutability,
            name,
            ty,
            init,
            span: start.to(end),
        }))
    }

    fn parse_if(&mut self) -> Option<IfStmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::If);

        let condition = self.parse_expr()?;
        let then_branch = self.parse_block()?;

        let else_branch = if self.eat_keyword(Keyword::Else) {
            if self.check_keyword(Keyword::If) {
                Some(ElseBranch::If(Box::new(self.parse_if()?)))
            } else {
                Some(ElseBranch::Block(self.parse_block()?))
            }
        } else {
            None
        };

        let end = match &else_branch {
            Some(ElseBranch::Block(b)) => b.span,
            Some(ElseBranch::If(i)) => i.span,
            None => then_branch.span,
        };

        Some(IfStmt {
            condition,
            then_branch,
            else_branch,
            span: start.to(end),
        })
    }

    fn parse_return(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Return);

        let value = if matches!(self.peek(), TokenKind::Semicolon | TokenKind::RBrace) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Return(ReturnStmt {
            value,
            span: start.to(end),
        }))
    }

    /// A statement starting with an expression: an assignment or a call.
    fn parse_expr_or_assign(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        let expr = self.parse_expr()?;

        if matches!(self.peek(), TokenKind::Assign) {
            self.pos += 1;
            let value = self.parse_expr()?;
            let end = self.peek_span();
            self.eat(&TokenKind::Semicolon);

            let Expr::Path(target) = expr else {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    start,
                    "the left-hand side of an assignment must be a variable",
                    "only a name can be assigned to",
                    None,
                );
                return None;
            };

            return Some(Stmt::Assign(AssignStmt {
                target,
                value,
                span: start.to(end),
            }));
        }

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Expr(ExprStmt {
            span: start.to(end),
            expr,
        }))
    }

    // --- Expressions ------------------------------------------------------

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_binary(0)
    }

    /// Precedence climbing.
    ///
    /// Every operator in the subset is left-associative, so the level of the
    /// right-hand side is always `level + 1`.
    fn parse_binary(&mut self, min_level: u8) -> Option<Expr> {
        let mut left = self.parse_unary()?;

        loop {
            let Some((op, level)) = precedence(self.peek()) else {
                break;
            };
            if level < min_level {
                break;
            }

            let op_span = self.peek_span();
            self.pos += 1;

            let right = self.parse_binary(level + 1)?;
            let span = left.span().to(right.span());

            left = Expr::Binary(BinaryExpr {
                op,
                left: Box::new(left),
                right: Box::new(right),
                op_span,
                span,
            });
        }

        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let start = self.peek_span();

        let op = match self.peek() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Not => Some(UnaryOp::Not),
            _ => None,
        };

        if let Some(op) = op {
            self.pos += 1;
            let operand = self.parse_unary()?;
            let span = start.to(operand.span());
            return Some(Expr::Unary(UnaryExpr {
                op,
                operand: Box::new(operand),
                span,
            }));
        }

        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let span = self.peek_span();

        match self.peek().clone() {
            TokenKind::Integer(value) => {
                self.pos += 1;
                Some(Expr::Int(IntLit { value, span }))
            }
            TokenKind::Str(value) => {
                self.pos += 1;
                Some(Expr::Str(StrLit { value, span }))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.pos += 1;
                Some(Expr::Bool(BoolLit { value: true, span }))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.pos += 1;
                Some(Expr::Bool(BoolLit { value: false, span }))
            }
            TokenKind::LParen => {
                self.pos += 1;
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "to close the parenthesis");
                Some(expr)
            }
            TokenKind::Identifier(name) => {
                self.pos += 1;
                self.parse_after_ident(Ident::new(name, span))
            }
            _ => {
                if self.report_if_from_another_phase() {
                    return None;
                }
                let found = self.peek().description();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    span,
                    "expected an expression",
                    format!("found {found}"),
                    None,
                );

                // The token that cannot start an expression is consumed.
                // Leaving it makes the next turn of the block find it again and
                // emit the same error twice. Closers are respected: they belong
                // to our caller.
                if !matches!(
                    self.peek(),
                    TokenKind::RBrace | TokenKind::RParen | TokenKind::Eof
                ) {
                    self.pos += 1;
                }
                None
            }
        }
    }

    /// What may follow an identifier: a call, `stdout.println`, or nothing.
    fn parse_after_ident(&mut self, ident: Ident) -> Option<Expr> {
        // `stdout.println(expr)` is a special syntactic form while neither
        // modules nor a standard library exist. See decision D4 of the design.
        if ident.name == "stdout" && matches!(self.peek(), TokenKind::Dot) {
            self.pos += 1;
            let method_span = self.peek_span();
            let method = self.expect_identifier("after `stdout.`")?;

            if method.name != "println" {
                self.error(
                    codes::NOT_IMPLEMENTED,
                    method_span,
                    format!("`stdout.{}` is not available yet", method.name),
                    "only `stdout.println` exists in this phase",
                    Some("the full standard library arrives in Phase 7".into()),
                );
                return None;
            }

            self.expect(&TokenKind::LParen, "after `println`");
            let arg = self.parse_expr()?;
            let end = self.peek_span();
            self.expect(&TokenKind::RParen, "to close the call");

            return Some(Expr::Println(PrintlnExpr {
                arg: Box::new(arg),
                span: ident.span.to(end),
            }));
        }

        if matches!(self.peek(), TokenKind::LParen) {
            self.pos += 1;
            let mut args = Vec::new();

            while !matches!(self.peek(), TokenKind::RParen) && !self.at_eof() {
                args.push(self.parse_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }

            let end = self.peek_span();
            self.expect(&TokenKind::RParen, "to close the argument list");

            return Some(Expr::Call(CallExpr {
                span: ident.span.to(end),
                callee: ident,
                args,
            }));
        }

        Some(Expr::Path(ident))
    }
}

/// Binary operator and its precedence level.
///
/// A higher number binds tighter. This is the direct translation of the table
/// in `ZIRK_LANGUAGE_SPEC.md` section 4.
fn precedence(kind: &TokenKind) -> Option<(BinaryOp, u8)> {
    use BinaryOp::*;
    use TokenKind as T;

    Some(match kind {
        T::OrOr => (Or, 1),
        T::AndAnd => (And, 2),
        T::Eq => (BinaryOp::Eq, 3),
        T::NotEq => (NotEq, 3),
        T::Lt => (Lt, 4),
        T::LtEq => (LtEq, 4),
        T::Gt => (Gt, 4),
        T::GtEq => (GtEq, 4),
        T::Plus => (Add, 5),
        T::Minus => (Sub, 5),
        T::Star => (Mul, 6),
        T::Slash => (Div, 6),
        T::Percent => (Rem, 6),
        _ => return None,
    })
}
