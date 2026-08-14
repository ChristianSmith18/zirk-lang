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
            .unwrap_or_else(|| {
                let end = self.source.text().len() as u32;
                self.source.span(end, end)
            })
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

        // Declarations belong at the top level, and reaching here means one
        // appeared inside a function body.
        if self.check_keyword(Keyword::Import)
            || self.check_keyword(Keyword::Share)
            || self.check_keyword(Keyword::Use)
            || self.check_keyword(Keyword::Enum)
        {
            let word = match self.peek() {
                TokenKind::Keyword(k) => k.as_str(),
                _ => unreachable!("checked above"),
            };
            self.error(
                codes::MODULES_UNAVAILABLE,
                span,
                format!("`{word}` cannot appear inside a function"),
                "imports, `use`, `share` and `enum` are declarations of the file",
                Some("move it to the top level, outside every function".into()),
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
        let mut imports = Vec::new();
        let mut uses = Vec::new();
        let mut enums = Vec::new();
        let mut functions = Vec::new();

        while !self.at_eof() {
            if self.check_keyword(Keyword::Import) {
                if let Some(i) = self.parse_import() {
                    imports.push(i);
                }
                continue;
            }

            if self.check_keyword(Keyword::Use) {
                if let Some(u) = self.parse_use() {
                    uses.push(u);
                }
                continue;
            }

            // `share` modifies the declaration that follows it.
            let shared_at = self.check_keyword(Keyword::Share).then(|| {
                let span = self.peek_span();
                self.pos += 1;
                span
            });

            if self.check_keyword(Keyword::Fn) {
                if let Some(f) = self.parse_fn(shared_at.is_some()) {
                    functions.push(f);
                }
                continue;
            }

            if self.check_keyword(Keyword::Enum) {
                if let Some(e) = self.parse_enum(shared_at.is_some()) {
                    enums.push(e);
                }
                continue;
            }

            if self.report_if_from_another_phase() {
                continue;
            }

            let span = shared_at.unwrap_or_else(|| self.peek_span());
            let found = self.peek().description();
            let (cause, help) = if shared_at.is_some() {
                (
                    format!("found {found} after `share`"),
                    "`share` applies to a `fn` or an `enum`",
                )
            } else {
                (
                    format!("found {found} at the top level of the file"),
                    "a file contains `import`, `use`, `enum` and `fn` declarations",
                )
            };
            self.error(
                codes::UNEXPECTED_TOKEN,
                span,
                "expected a declaration",
                cause,
                Some(help.into()),
            );
            self.synchronize();
            if !self.check_keyword(Keyword::Fn) && !self.at_eof() {
                self.pos += 1;
            }
        }

        let end = self.peek_span();
        Program {
            imports,
            uses,
            enums,
            functions,
            span: start.to(end),
        }
    }

    /// `import { A, B -> C } from "./path";` or `... from std.io;`
    fn parse_import(&mut self) -> Option<ImportDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Import);

        self.expect(&TokenKind::LBrace, "after `import`");

        let mut names = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let name = self.expect_identifier("as an imported name")?;

            // `name -> alias` renames it in the importing file.
            let alias = if self.eat(&TokenKind::Arrow) {
                Some(self.expect_identifier("after `->`")?)
            } else {
                None
            };

            let span = name.span.to(alias.as_ref().map_or(name.span, |a| a.span));
            names.push(ImportName { name, alias, span });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "to close the import list");

        if !self.expect(&TokenKind::Keyword(Keyword::From), "after the import list") {
            self.synchronize();
            return None;
        }

        let source = self.parse_import_source()?;
        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(ImportDecl {
            names,
            source,
            span: start.to(end),
        })
    }

    /// The quotes are what tell the two forms apart, per
    /// `ZIRK_LANGUAGE_SPEC.md` section 10.
    fn parse_import_source(&mut self) -> Option<ImportSource> {
        let span = self.peek_span();

        if let TokenKind::Str(path) = self.peek().clone() {
            self.pos += 1;
            return Some(ImportSource::Local { path, span });
        }

        // A standard module is a dotted name: `std.io`.
        if matches!(self.peek(), TokenKind::Identifier(_)) {
            let mut path = String::new();
            let mut end;

            loop {
                let Some(segment) = self.expect_identifier("in the module name") else {
                    return None;
                };
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(&segment.name);
                end = segment.span;

                if !self.eat(&TokenKind::Dot) {
                    break;
                }
            }

            return Some(ImportSource::Std {
                path,
                span: span.to(end),
            });
        }

        let found = self.peek().description();
        self.error(
            codes::INVALID_IMPORT_SOURCE,
            span,
            "expected the source of the import",
            format!("found {found}"),
            Some("a local path goes in quotes (`\"./user\"`); a standard module does not (`std.io`)".into()),
        );
        self.synchronize();
        None
    }

    /// `use stdout;`
    fn parse_use(&mut self) -> Option<UseDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Use);

        let name = self.expect_identifier("after `use`")?;
        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(UseDecl {
            name,
            span: start.to(end),
        })
    }

    /// `enum Direction { North, South }`
    ///
    /// Without associated data: that is Phase 3, per decision D1.
    fn parse_enum(&mut self, shared: bool) -> Option<EnumDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Enum);

        let name = self.expect_identifier("after `enum`")?;
        self.expect(&TokenKind::LBrace, "after the enum name");

        let mut variants = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let Some(variant) = self.expect_identifier("as an enum variant") else {
                break;
            };

            // Associated data is what Phase 3 adds on top of this.
            if matches!(self.peek(), TokenKind::LParen) {
                let span = self.peek_span();
                self.error(
                    codes::NOT_IMPLEMENTED,
                    span,
                    "enum variants with associated data are not implemented yet",
                    "this phase covers enums as a closed set of names",
                    Some("algebraic enums arrive in Phase 3".into()),
                );
                self.synchronize();
                return None;
            }

            variants.push(variant);

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "to close the enum body");

        Some(EnumDecl {
            name,
            variants,
            shared,
            span: start.to(end),
        })
    }

    fn parse_fn(&mut self, shared: bool) -> Option<FnDecl> {
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
            shared,
            span,
        })
    }

    /// Parameters, with the optional, default and variadic forms of
    /// `ZIRK_LANGUAGE_SPEC.md` section 6.
    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        let mut variadic_at: Option<Span> = None;

        while !matches!(self.peek(), TokenKind::RParen) && !self.at_eof() {
            let start = self.peek_span();

            // A variadic that is not last would make the remaining parameters
            // unreachable by position.
            if let Some(previous) = variadic_at {
                self.error(
                    codes::VARIADIC_NOT_LAST,
                    previous,
                    "the variadic parameter must be the last one",
                    "the parameters after it could never receive an argument",
                    Some("move `...` to the end of the list".into()),
                );
                break;
            }

            let variadic = self.eat(&TokenKind::DotDotDot);

            let Some(name) = self.expect_identifier("as a parameter name") else {
                break;
            };

            // `name?: T` marks the parameter optional.
            let optional = self.eat(&TokenKind::Question);

            if !self.expect(&TokenKind::Colon, "after the parameter name") {
                break;
            }
            let Some(ty) = self.parse_type() else { break };

            let default = if self.eat(&TokenKind::Assign) {
                let Some(expr) = self.parse_expr() else { break };
                Some(expr)
            } else {
                None
            };

            let end = default.as_ref().map_or(ty.span, |d| d.span());
            if variadic {
                variadic_at = Some(start.to(end));
            }

            params.push(Param {
                name,
                ty,
                optional,
                default,
                variadic,
                span: start.to(end),
            });

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

            // `T?` is `T | Null`, per `ZIRK_LANGUAGE_SPEC.md` section 4.
            if matches!(self.peek(), TokenKind::Question) {
                let end = self.peek_span();
                self.pos += 1;
                return Some(TypeRef::nullable(name, span.to(end)));
            }

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
        if self.check_keyword(Keyword::While) {
            return self.parse_while();
        }
        if self.check_keyword(Keyword::Loop) {
            return self.parse_loop();
        }
        if self.check_keyword(Keyword::For) {
            return self.parse_for();
        }
        if self.check_keyword(Keyword::Break) || self.check_keyword(Keyword::Continue) {
            return self.parse_jump();
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

    /// `while cond { }`
    fn parse_while(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::While);

        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        let span = start.to(body.span);

        Some(Stmt::Loop(LoopStmt {
            kind: LoopKind::While,
            init: None,
            condition: Some(condition),
            step: None,
            body,
            span,
        }))
    }

    /// `loop { }`
    fn parse_loop(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Loop);

        let body = self.parse_block()?;
        let span = start.to(body.span);

        Some(Stmt::Loop(LoopStmt {
            kind: LoopKind::Loop,
            init: None,
            condition: None,
            step: None,
            body,
            span,
        }))
    }

    /// `for (init; cond; step) { }` and `for x in iterable { }`.
    ///
    /// Both start with `for`, and which one it is only becomes clear after the
    /// binding: the `in` form has no parentheses.
    fn parse_for(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::For);

        // `for x in ...`: an identifier followed by `in`.
        if let TokenKind::Identifier(name) = self.peek().clone() {
            let binding_span = self.peek_span();
            if self.tokens.get(self.pos + 1).map(|t| &t.kind)
                == Some(&TokenKind::Keyword(Keyword::In))
            {
                self.pos += 2;
                let binding = Ident::new(name, binding_span);
                let iterable = self.parse_expr()?;
                let body = self.parse_block()?;
                let span = start.to(body.span);

                return Some(Stmt::ForIn(ForInStmt {
                    binding,
                    iterable,
                    body,
                    span,
                }));
            }
        }

        self.expect(&TokenKind::LParen, "after `for`");

        let init = if matches!(self.peek(), TokenKind::Semicolon) {
            self.pos += 1;
            None
        } else {
            let stmt = self.parse_stmt()?;
            Some(Box::new(stmt))
        };

        let condition = if matches!(self.peek(), TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        self.expect(&TokenKind::Semicolon, "after the loop condition");

        let step = if matches!(self.peek(), TokenKind::RParen) {
            None
        } else {
            let stmt = self.parse_simple_stmt()?;
            Some(Box::new(stmt))
        };
        self.expect(&TokenKind::RParen, "to close the `for` header");

        let body = self.parse_block()?;
        let span = start.to(body.span);

        Some(Stmt::Loop(LoopStmt {
            kind: LoopKind::For,
            init,
            condition,
            step,
            body,
            span,
        }))
    }

    /// A statement without a trailing semicolon, as used by the `for` step.
    fn parse_simple_stmt(&mut self) -> Option<Stmt> {
        self.parse_expr_or_assign()
    }

    fn parse_jump(&mut self) -> Option<Stmt> {
        let span = self.peek_span();
        let is_break = self.check_keyword(Keyword::Break);
        self.pos += 1;
        self.eat(&TokenKind::Semicolon);

        let jump = JumpStmt { span };
        Some(if is_break {
            Stmt::Break(jump)
        } else {
            Stmt::Continue(jump)
        })
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
    ///
    /// Compound assignment and increment are desugared here, per decision D9:
    /// `i += 1` and `i++` both produce the tree of `i = i + 1`, so nothing
    /// downstream needs to know they exist.
    fn parse_expr_or_assign(&mut self) -> Option<Stmt> {
        let start = self.peek_span();

        // Prefix increment: `++i`.
        if let Some(op) = increment_op(self.peek()) {
            let op_span = self.peek_span();
            self.pos += 1;
            let target = self.expect_identifier("after the increment operator")?;
            let end = self.peek_span();
            self.eat(&TokenKind::Semicolon);
            return Some(self.desugar_compound(target, op, one(op_span), op_span, start.to(end)));
        }

        // The inner form, so a trailing `++` reaches the postfix branch below
        // instead of being reported as a value-position increment.
        let expr = self.parse_expr_inner()?;

        // Postfix increment: `i++`.
        if let Some(op) = increment_op(self.peek()) {
            let op_span = self.peek_span();
            self.pos += 1;
            let end = self.peek_span();
            self.eat(&TokenKind::Semicolon);

            let Some(target) = as_assignable(&expr) else {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    expr.span(),
                    "only a variable can be incremented",
                    "the operand of `++` or `--` must be a name",
                    None,
                );
                return None;
            };
            return Some(self.desugar_compound(target, op, one(op_span), op_span, start.to(end)));
        }

        // Compound assignment: `i += 1`.
        if let Some(op) = compound_op(self.peek()) {
            let op_span = self.peek_span();
            self.pos += 1;
            let value = self.parse_expr()?;
            let end = self.peek_span();
            self.eat(&TokenKind::Semicolon);

            let Some(target) = as_assignable(&expr) else {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    expr.span(),
                    "the left-hand side of a compound assignment must be a variable",
                    "only a name can be assigned to",
                    None,
                );
                return None;
            };
            return Some(self.desugar_compound(target, op, value, op_span, start.to(end)));
        }

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

    /// Builds the `target = target <op> value` that a compound form stands for.
    fn desugar_compound(
        &mut self,
        target: Ident,
        op: BinaryOp,
        value: Expr,
        op_span: Span,
        span: Span,
    ) -> Stmt {
        let read = Expr::Path(target.clone());
        let combined = Expr::Binary(BinaryExpr {
            op,
            span: read.span().to(value.span()),
            left: Box::new(read),
            right: Box::new(value),
            op_span,
        });

        Stmt::Assign(AssignStmt {
            target,
            value: combined,
            span,
        })
    }

    // --- Expressions ------------------------------------------------------

    /// An expression in value position.
    ///
    /// Differs from [`Self::parse_expr_inner`] only in rejecting a trailing
    /// `++`/`--`: `i++` is a statement, and reaching here means it was written
    /// where a value is expected. Decision D9.
    fn parse_expr(&mut self) -> Option<Expr> {
        let expr = self.parse_expr_inner()?;

        if increment_op(self.peek()).is_some() {
            let symbol = self.peek().symbol();
            let span = self.peek_span();
            self.error(
                codes::INCREMENT_AS_EXPRESSION,
                span,
                format!("`{symbol}` cannot be used where a value is expected"),
                "in this phase increment and decrement are statements, not expressions",
                Some("write it on its own line, then use the variable".into()),
            );
            self.pos += 1;
            return None;
        }

        Some(expr)
    }

    fn parse_expr_inner(&mut self) -> Option<Expr> {
        // A lambda is recognized before anything else: `(` would otherwise be
        // read as a parenthesized expression.
        if self.at_lambda() {
            return self.parse_lambda();
        }
        if self.check_keyword(Keyword::If) {
            return self.parse_if().map(|i| Expr::If(Box::new(i)));
        }
        if self.check_keyword(Keyword::Match) {
            return self.parse_match();
        }

        self.parse_range()
    }

    /// Whether the tokens ahead start a lambda rather than a parenthesized
    /// expression.
    ///
    /// Both begin with `(`, and only the matching `)` followed by `:` tells
    /// them apart — the return type is mandatory, which is what makes this
    /// decidable with a scan instead of backtracking.
    fn at_lambda(&self) -> bool {
        if !matches!(self.peek(), TokenKind::LParen) {
            return false;
        }

        let mut depth = 0usize;
        let mut i = self.pos;

        while let Some(token) = self.tokens.get(i) {
            match token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(i + 1).map(|t| &t.kind),
                            Some(TokenKind::Colon)
                        );
                    }
                }
                TokenKind::Eof => return false,
                _ => {}
            }
            i += 1;
        }

        false
    }

    fn parse_lambda(&mut self) -> Option<Expr> {
        let start = self.peek_span();

        self.expect(&TokenKind::LParen, "to open the lambda parameters");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the lambda parameters");

        self.expect(&TokenKind::Colon, "before the lambda return type");
        let return_type = self.parse_type()?;

        self.expect(&TokenKind::FatArrow, "before the lambda body");

        let body = if matches!(self.peek(), TokenKind::LBrace) {
            LambdaBody::Block(self.parse_block()?)
        } else {
            LambdaBody::Expr(self.parse_expr()?)
        };

        let end = match &body {
            LambdaBody::Block(b) => b.span,
            LambdaBody::Expr(e) => e.span(),
        };

        Some(Expr::Lambda(LambdaExpr {
            params,
            return_type,
            body: Box::new(body),
            span: start.to(end),
        }))
    }

    fn parse_match(&mut self) -> Option<Expr> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Match);

        // `match with` scopes a `Resource<E>`, which needs Phase 4.
        if self.check_keyword(Keyword::With) {
            let span = self.peek_span();
            self.error(
                codes::NOT_IMPLEMENTED,
                span,
                "`match with` is not implemented yet",
                "scoped acquisition needs `Resource<E>`, which arrives in Phase 4",
                Some("see docs/init/ZIRK_ROADMAP.md for the scope of each phase".into()),
            );
            self.synchronize();
            return None;
        }

        let scrutinee = self.parse_range()?;
        self.expect(&TokenKind::LBrace, "before the match arms");

        let mut arms = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let before = self.pos;

            if let Some(arm) = self.parse_match_arm() {
                arms.push(arm);
            }

            // Arms may be separated by `,` or `;`, or by nothing at all when
            // the body is a block.
            self.eat(&TokenKind::Comma);
            self.eat(&TokenKind::Semicolon);

            if self.pos == before {
                self.pos += 1;
            }
        }

        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "to close the match arms");

        if arms.is_empty() {
            self.error(
                codes::EMPTY_MATCH,
                start.to(end),
                "a `match` needs at least one arm",
                "a match with no arms could never produce a result",
                Some("add an arm, or `_ => { }` to ignore every case".into()),
            );
            return None;
        }

        Some(Expr::Match(MatchExpr {
            scrutinee: Box::new(scrutinee),
            arms,
            span: start.to(end),
        }))
    }

    fn parse_match_arm(&mut self) -> Option<MatchArm> {
        let start = self.peek_span();
        let pattern = self.parse_pattern()?;

        self.expect(&TokenKind::FatArrow, "after the pattern");

        let body = if matches!(self.peek(), TokenKind::LBrace) {
            ArmBody::Block(self.parse_block()?)
        } else {
            ArmBody::Expr(self.parse_expr()?)
        };

        Some(MatchArm {
            span: start.to(body.span()),
            pattern,
            body,
        })
    }

    fn parse_pattern(&mut self) -> Option<Pattern> {
        let span = self.peek_span();

        match self.peek().clone() {
            TokenKind::Integer(value) => {
                self.pos += 1;
                Some(Pattern::Int(IntLit { value, span }))
            }
            // A negative literal is one pattern, not a unary operator applied
            // to one: patterns have no operators.
            TokenKind::Minus => {
                self.pos += 1;
                let TokenKind::Integer(value) = self.peek().clone() else {
                    let found = self.peek().description();
                    self.error(
                        codes::UNEXPECTED_TOKEN,
                        self.peek_span(),
                        "expected an integer literal after `-`",
                        format!("found {found}"),
                        None,
                    );
                    return None;
                };
                let end = self.peek_span();
                self.pos += 1;
                Some(Pattern::Int(IntLit {
                    value: -value,
                    span: span.to(end),
                }))
            }
            TokenKind::Str(value) => {
                self.pos += 1;
                Some(Pattern::Str(StrLit { value, span }))
            }
            TokenKind::Keyword(Keyword::True) => {
                self.pos += 1;
                Some(Pattern::Bool(BoolLit { value: true, span }))
            }
            TokenKind::Keyword(Keyword::False) => {
                self.pos += 1;
                Some(Pattern::Bool(BoolLit { value: false, span }))
            }
            TokenKind::Keyword(Keyword::Null) => {
                self.pos += 1;
                Some(Pattern::Null(NullLit { span }))
            }
            TokenKind::Identifier(name) => {
                self.pos += 1;

                // `Direction.North` names a variant; a bare name binds.
                if matches!(self.peek(), TokenKind::Dot) {
                    self.pos += 1;
                    let variant = self.expect_identifier("after the enum name")?;
                    return Some(Pattern::Variant(VariantPattern {
                        span: span.to(variant.span),
                        enum_name: Ident::new(name, span),
                        variant,
                    }));
                }

                if name == "_" {
                    return Some(Pattern::Wildcard(span));
                }
                Some(Pattern::Binding(Ident::new(name, span)))
            }
            _ => {
                let found = self.peek().description();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    span,
                    "expected a pattern",
                    format!("found {found}"),
                    Some("a pattern is a literal, an enum variant, a name, or `_`".into()),
                );
                None
            }
        }
    }

    /// `a..b` and `a..=b`, which bind looser than every binary operator.
    fn parse_range(&mut self) -> Option<Expr> {
        let start = self.parse_binary(0)?;

        let inclusive = match self.peek() {
            TokenKind::DotDot => false,
            TokenKind::DotDotEq => true,
            _ => return Some(start),
        };
        self.pos += 1;

        let end = self.parse_binary(0)?;

        Some(Expr::Range(RangeExpr {
            span: start.span().to(end.span()),
            start: Box::new(start),
            end: Box::new(end),
            inclusive,
        }))
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
            TokenKind::Keyword(Keyword::Null) => {
                self.pos += 1;
                Some(Expr::Null(NullLit { span }))
            }
            // Increment is a statement in this phase: as an expression its
            // prefix/postfix distinction would need an evaluation order for
            // side effects that no normative document defines. Decision D9.
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                let symbol = self.peek().symbol();
                self.error(
                    codes::INCREMENT_AS_EXPRESSION,
                    span,
                    format!("`{symbol}` cannot be used where a value is expected"),
                    "in this phase increment and decrement are statements, not expressions",
                    Some("write it on its own line, then use the variable".into()),
                );
                self.pos += 1;
                None
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

        // `?.` needs a member to access, and no type of this phase has
        // members. Decision D8.
        if matches!(self.peek(), TokenKind::QuestionDot) {
            let span = self.peek_span();
            self.error(
                codes::NOT_IMPLEMENTED,
                span,
                "`?.` is not implemented yet",
                "safe access needs a type with members, and classes arrive in Phase 3",
                Some("`T?` and `??` do work in this phase".into()),
            );
            self.synchronize();
            return None;
        }

        // `Direction.North` names a variant. Only an enum is reachable this
        // way in this phase: no other type has anything after a `.`.
        if matches!(self.peek(), TokenKind::Dot) {
            self.pos += 1;
            let variant = self.expect_identifier("after the enum name")?;
            return Some(Expr::Variant(VariantExpr {
                span: ident.span.to(variant.span),
                enum_name: ident,
                variant,
            }));
        }

        if matches!(self.peek(), TokenKind::LParen) {
            let args = self.parse_args()?;
            let end = self.peek_span();
            self.expect(&TokenKind::RParen, "to close the argument list");

            return Some(Expr::Call(CallExpr {
                span: ident.span.to(end),
                callee: Box::new(Expr::Path(ident)),
                args,
            }));
        }

        Some(Expr::Path(ident))
    }

    /// The argument list of a call, positional or named.
    ///
    /// Leaves the closing `)` for the caller to consume, so it can include it
    /// in the span of the call.
    fn parse_args(&mut self) -> Option<Vec<Arg>> {
        self.expect(&TokenKind::LParen, "to open the argument list");
        let mut args = Vec::new();

        while !matches!(self.peek(), TokenKind::RParen) && !self.at_eof() {
            let start = self.peek_span();

            // `name: value` matches by name. A bare identifier followed by `:`
            // is unambiguous here: a type annotation cannot appear in a call.
            let name = match self.peek().clone() {
                TokenKind::Identifier(name)
                    if self.tokens.get(self.pos + 1).map(|t| &t.kind)
                        == Some(&TokenKind::Colon) =>
                {
                    let span = self.peek_span();
                    self.pos += 2;
                    Some(Ident::new(name, span))
                }
                _ => None,
            };

            let value = self.parse_expr()?;
            args.push(Arg {
                span: start.to(value.span()),
                name,
                value,
            });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        Some(args)
    }
}

/// The assignment target an expression stands for, if it is a plain name.
fn as_assignable(expr: &Expr) -> Option<Ident> {
    match expr {
        Expr::Path(ident) => Some(ident.clone()),
        _ => None,
    }
}

/// The operator a compound assignment stands for.
fn compound_op(kind: &TokenKind) -> Option<BinaryOp> {
    Some(match kind {
        TokenKind::PlusEq => BinaryOp::Add,
        TokenKind::MinusEq => BinaryOp::Sub,
        TokenKind::StarEq => BinaryOp::Mul,
        TokenKind::SlashEq => BinaryOp::Div,
        TokenKind::PercentEq => BinaryOp::Rem,
        _ => return None,
    })
}

/// The operator an increment or decrement stands for.
fn increment_op(kind: &TokenKind) -> Option<BinaryOp> {
    Some(match kind {
        TokenKind::PlusPlus => BinaryOp::Add,
        TokenKind::MinusMinus => BinaryOp::Sub,
        _ => return None,
    })
}

/// The literal `1` that `++` and `--` add or subtract.
fn one(span: Span) -> Expr {
    Expr::Int(IntLit { value: 1, span })
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
        // `??` sits between comparison and arithmetic: tighter than `==` so
        // `name ?? "x" == "x"` compares the resolved value rather than
        // coalescing against a boolean, and looser than `+` so the fallback
        // may be an expression. C# places it below `||` and that ordering is a
        // known source of surprise.
        T::QuestionQuestion => (Coalesce, 5),
        T::Plus => (Add, 6),
        T::Minus => (Sub, 6),
        T::Star => (Mul, 7),
        T::Slash => (Div, 7),
        T::Percent => (Rem, 7),
        _ => return None,
    })
}
