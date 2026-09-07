//! Recursive-descent parser with precedence climbing.
//!
//! Precedence is implemented with a loop over levels rather than one function
//! per level: the `precedence()` table is the readable definition of what
//! `ZIRK_LANGUAGE_SPEC.md` section 4 fixes, and adding an operator means adding
//! a row rather than a function.

use crate::codes;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, SourceFile, Span};
use zirk_lexer::{DurationUnit, Keyword, StrPart, Token, TokenKind, tokenize};

/// Parses a sequence of tokens into a program.
///
/// Returns the tree even when there are errors: the caller decides whether to
/// continue by consulting `sink.has_errors()`.
pub fn parse(source: &SourceFile, tokens: &[Token], sink: &mut DiagnosticSink) -> Program {
    Parser::new(source, tokens, sink).parse_program()
}

/// How deeply expressions and blocks may nest.
///
/// The parser is recursive descent, so nesting costs stack, and running out of
/// it aborts the process with no diagnostic at all — the worst possible way to
/// fail. The limit turns that into an error that says what happened.
///
/// The number is set by the smallest stack the parser can be called on, not by
/// the language: measured on a 2 MB thread — what a test thread gets, and less
/// than the main thread's 8 MB — a debug build runs out somewhere between 260
/// and 280 levels. 128 keeps a factor of two on the worst case while sitting
/// far above anything written by hand.
///
/// Raising it means giving the compiler a stack of its own to run on, the way
/// `rustc` spawns a thread for the job. That belongs with the compilation
/// driver, not here.
const MAX_NESTING: u32 = 96;

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: &'a [Token],
    sink: &'a mut DiagnosticSink,
    pos: usize,
    /// How many expressions and blocks enclose the one being parsed.
    depth: u32,
    /// What is left of a `>>`, `>>=` or `>=` token after [`Self::eat_gt`] has
    /// taken its first `>` to close one level of `Box<Box<T>>`.
    ///
    /// The lexer emits those as one token (`token.rs` explains why), so
    /// closing nested generics means handing back the rest without moving
    /// `pos`, which still points at the original token until this is spent.
    pending_gt: Option<TokenKind>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a SourceFile, tokens: &'a [Token], sink: &'a mut DiagnosticSink) -> Self {
        Self {
            source,
            tokens,
            sink,
            pos: 0,
            depth: 0,
            pending_gt: None,
        }
    }

    /// Enters one nesting level, reporting when the limit is reached.
    ///
    /// Returns `false` when the caller must give up rather than recurse.
    fn enter(&mut self) -> bool {
        self.depth += 1;
        if self.depth <= MAX_NESTING {
            return true;
        }

        // Only the first one is worth reporting: every enclosing level would
        // otherwise repeat it on the way out.
        if self.depth == MAX_NESTING + 1 {
            let span = self.peek_span();
            self.error(
                codes::NESTING_TOO_DEEP,
                span,
                "the code nests too deeply",
                format!("the parser stops at {MAX_NESTING} levels of nesting"),
                Some("split the expression into named parts".into()),
            );
        }
        false
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    // --- Navigation -------------------------------------------------------

    fn peek(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    /// The token `offset` positions ahead of [`Self::peek`], without
    /// consuming anything — needed to disambiguate `Fn(name: T, ...)`'s
    /// optional `name:`/`name?:` label from a bare `T` (`Fn(String) =>
    /// Void`), which the single-token [`Self::peek`] cannot tell apart.
    fn peek_at(&self, offset: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + offset)
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

    /// Splits a `>`-starting token into its first `>` and what remains.
    fn split_gt(kind: &TokenKind) -> Option<(TokenKind, Option<TokenKind>)> {
        match kind {
            TokenKind::Gt => Some((TokenKind::Gt, None)),
            TokenKind::Shr => Some((TokenKind::Gt, Some(TokenKind::Gt))),
            TokenKind::ShrEq => Some((TokenKind::Gt, Some(TokenKind::GtEq))),
            TokenKind::GtEq => Some((TokenKind::Gt, Some(TokenKind::Assign))),
            _ => None,
        }
    }

    /// Whether the next `>` a generic parameter or argument list needs is
    /// available, possibly as the head of a `>>`, `>>=` or `>=` token.
    fn check_gt(&self) -> bool {
        match &self.pending_gt {
            Some(kind) => Self::split_gt(kind).is_some(),
            None => Self::split_gt(self.peek()).is_some(),
        }
    }

    /// Consumes one `>`, splitting `>>`, `>>=` or `>=` when that is what is
    /// there. See [`Self::pending_gt`].
    fn eat_gt(&mut self) -> bool {
        let source = match self.pending_gt.take() {
            Some(kind) => kind,
            None => self.peek().clone(),
        };
        let Some((_, rest)) = Self::split_gt(&source) else {
            self.pending_gt = Some(source);
            return false;
        };
        match rest {
            Some(rest) => self.pending_gt = Some(rest),
            None => self.pos += 1,
        }
        true
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

    /// Like [`Self::expect`], but for keywords.
    ///
    /// It exists because `TokenKind::symbol` is empty for a keyword, so the
    /// generic message would read "expected `` after ...".
    fn expect_keyword(&mut self, expected: Keyword, context: &str) -> bool {
        if self.eat_keyword(expected) {
            return true;
        }

        let span = self.peek_span();
        let found = self.peek().description();
        self.error(
            codes::UNEXPECTED_TOKEN,
            span,
            format!("expected `{}` {context}", expected.as_str()),
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
        let mut classes = Vec::new();
        let mut contracts = Vec::new();
        let mut functions = Vec::new();
        let mut type_aliases = Vec::new();
        let mut externs = Vec::new();

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

            if self.check_keyword(Keyword::Extern) {
                if let Some(e) = self.parse_extern_fn() {
                    externs.push(e);
                }
                continue;
            }

            // `unsafe fn`: the modifier that makes the whole body run as if
            // wrapped in `unsafe {}` (roadmap Phase 4e). Only meaningful in
            // front of `fn` at the top level — `unsafe {}` blocks live in
            // statement/expression position instead.
            let unsafe_at = self.check_keyword(Keyword::Unsafe).then(|| {
                let span = self.peek_span();
                self.pos += 1;
                span
            });

            if self.check_keyword(Keyword::Fn) {
                if let Some(f) = self.parse_fn(shared_at.is_some(), unsafe_at.is_some()) {
                    functions.push(f);
                }
                continue;
            }

            if let Some(span) = unsafe_at {
                let found = self.peek().description();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    span,
                    "expected `fn` after `unsafe`",
                    format!("found {found} after `unsafe`"),
                    Some("`unsafe` modifies a top-level `fn` declaration".into()),
                );
                self.synchronize();
                continue;
            }

            if self.check_keyword(Keyword::Enum) {
                if let Some(e) = self.parse_enum(shared_at.is_some()) {
                    enums.push(e);
                }
                continue;
            }

            if self.check_keyword(Keyword::Class) {
                if let Some(c) = self.parse_class(shared_at.is_some(), ClassKind::Class) {
                    classes.push(c);
                }
                continue;
            }

            // `abstract class`: a requirement set, adopted with `implements`
            // rather than extended. Only meaningful directly in front of
            // `class` — elsewhere `abstract` still names a later phase.
            if self.check_keyword(Keyword::Abstract)
                && self.tokens.get(self.pos + 1).map(|t| &t.kind)
                    == Some(&TokenKind::Keyword(Keyword::Class))
            {
                self.pos += 1; // `abstract`
                if let Some(c) = self.parse_class(shared_at.is_some(), ClassKind::Abstract) {
                    classes.push(c);
                }
                continue;
            }

            if self.check_keyword(Keyword::Record) {
                if let Some(c) = self.parse_class(shared_at.is_some(), ClassKind::Record) {
                    classes.push(c);
                }
                continue;
            }

            if self.check_keyword(Keyword::Type) {
                if let Some(a) = self.parse_type_alias(shared_at.is_some()) {
                    type_aliases.push(a);
                }
                continue;
            }

            if self.check_keyword(Keyword::Interface) || self.check_keyword(Keyword::Trait) {
                if let Some(c) = self.parse_contract(shared_at.is_some()) {
                    contracts.push(c);
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
                    "`share` applies to a `fn`, an `enum`, a `class` or a contract",
                )
            } else {
                (
                    format!("found {found} at the top level of the file"),
                    "a file contains `import`, `use`, `enum`, `class`, `interface`, `trait` and `fn` declarations",
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
            classes,
            contracts,
            functions,
            type_aliases,
            externs,
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
                let segment = self.expect_identifier("in the module name")?;
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
            Some(
                "a local path goes in quotes (`\"./user\"`); a standard module does not (`std.io`)"
                    .into(),
            ),
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
    /// `class User { fields, constructs and methods }`
    ///
    /// The three kinds of member are told apart by what starts them: the
    /// `construct` keyword, a `fn`, or anything else — which is a field.
    fn parse_class(&mut self, shared: bool, kind: ClassKind) -> Option<ClassDecl> {
        let start = self.peek_span();
        let keyword = match kind {
            // `abstract` is consumed by the caller, ahead of `class` itself.
            ClassKind::Class | ClassKind::Abstract => Keyword::Class,
            ClassKind::Record => Keyword::Record,
        };
        self.eat_keyword(keyword);

        let context = if kind == ClassKind::Record {
            "after `record`"
        } else {
            "after `class`"
        };
        let name = self.expect_identifier(context)?;
        let type_params = self.parse_type_params();

        // A class extends at most one class. Several bases would need a rule
        // for which one a repeated member comes from, and the spec has none.
        let extends = if self.eat_keyword(Keyword::Extends) {
            let base = self.expect_identifier("after `extends`")?;
            if matches!(self.peek(), TokenKind::Comma) {
                let span = self.peek_span();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    span,
                    "a class extends a single class",
                    "multiple inheritance is not part of the language",
                    Some("combine contracts with `implements` instead".into()),
                );
                return None;
            }
            Some(base)
        } else {
            None
        };

        // Several contracts, at most one base class: the spec allows combining
        // any number of contracts precisely because they carry no state.
        let mut implements = Vec::new();
        if self.eat_keyword(Keyword::Implements) {
            loop {
                let Some(contract) = self.parse_type_atom() else {
                    break;
                };
                implements.push(contract);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(&TokenKind::LBrace, "after the class name");

        let mut fields = Vec::new();
        let mut constructors = Vec::new();
        let mut methods = Vec::new();

        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let Some(member) = self.parse_class_member(kind == ClassKind::Abstract) else {
                self.synchronize_member();
                continue;
            };
            match member {
                ClassMember::Field(f) => fields.push(f),
                ClassMember::Construct(c) => constructors.push(c),
                ClassMember::Method(m) => methods.push(m),
            }
        }

        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "to close the class body");

        Some(ClassDecl {
            name,
            kind,
            type_params,
            implements,
            extends,
            fields,
            constructors,
            methods,
            shared,
            span: start.to(end),
        })
    }

    /// `record Name(field: Type, field: Type, ...);`
    /// `interface Name { ... }` or `trait Name { ... }`
    fn parse_contract(&mut self, shared: bool) -> Option<ContractDecl> {
        let start = self.peek_span();
        let kind = if self.eat_keyword(Keyword::Interface) {
            ContractKind::Interface
        } else {
            self.eat_keyword(Keyword::Trait);
            ContractKind::Trait
        };

        let name = self.expect_identifier(&format!("after `{}`", kind.as_str()))?;
        let type_params = self.parse_type_params();

        let mut implements = Vec::new();
        if self.eat_keyword(Keyword::Implements) {
            loop {
                let Some(contract) = self.parse_type_atom() else {
                    break;
                };
                implements.push(contract);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(&TokenKind::LBrace, "after the contract name");

        let mut methods = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let member_start = self.peek_span();
            let visibility = self.parse_visibility();

            if !self.check_keyword(Keyword::Fn) {
                let span = self.peek_span();
                let found = self.peek().description();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    span,
                    format!("a {} declares methods", kind.as_str()),
                    format!("found {found}"),
                    Some("a contract has no fields: it describes behaviour, not state".into()),
                );
                self.synchronize_member();
                continue;
            }

            let Some(method) = self.parse_contract_method(
                kind,
                visibility.unwrap_or(Visibility::Public),
                member_start,
            ) else {
                self.synchronize_member();
                continue;
            };
            methods.push(method);
        }

        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "to close the contract body");

        Some(ContractDecl {
            name,
            kind,
            type_params,
            implements,
            methods,
            shared,
            span: start.to(end),
        })
    }

    /// One method of a contract: a signature, and a body only in a `trait`.
    fn parse_contract_method(
        &mut self,
        kind: ContractKind,
        visibility: Visibility,
        start: Span,
    ) -> Option<MethodDecl> {
        self.eat_keyword(Keyword::Fn);

        let name = self.expect_identifier("after `fn`")?;
        let type_params = self.parse_type_params();
        self.expect(&TokenKind::LParen, "after the method name");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the parameter list");
        let return_type = self.parse_return_type(&name)?;
        let throws = self.parse_throws_clause();

        let has_body = matches!(self.peek(), TokenKind::LBrace);

        if has_body && kind == ContractKind::Interface {
            let span = self.peek_span();
            self.error(
                codes::UNEXPECTED_TOKEN,
                span,
                "an `interface` method has no body",
                "an interface declares signatures; a `trait` is the one that may carry implementation",
                Some("write `trait` instead, or remove the body".into()),
            );
            return None;
        }

        let body = if has_body {
            Some(self.parse_block()?)
        } else {
            self.eat(&TokenKind::Semicolon);
            None
        };
        let end = body.as_ref().map(|b| b.span).unwrap_or(name.span);

        Some(MethodDecl {
            name,
            type_params,
            is_override: false,
            is_mut: false,
            is_static: false,
            params,
            return_type,
            throws,
            body,
            visibility,
            is_abstract: false,
            span: start.to(end),
        })
    }

    /// One member of a class body.
    fn parse_class_member(&mut self, allow_abstract: bool) -> Option<ClassMember> {
        let start = self.peek_span();

        // The modifiers come first and apply to whatever follows.
        let visibility = self.parse_visibility();

        // `static fn` and `static` fields live at class level, not in each
        // instance, and are reached through the type name.
        let is_static = self.eat_keyword(Keyword::Static);

        // Only meaningful inside an `abstract class`: every member there is
        // a signature, never a body, so the keyword itself does not carry
        // that on its own — it is written for each member all the same.
        let is_abstract = if allow_abstract {
            self.eat_keyword(Keyword::Abstract)
        } else if self.check_keyword(Keyword::Abstract) {
            let span = self.peek_span();
            self.error(
                codes::ABSTRACT_OUTSIDE_ABSTRACT_CLASS,
                span,
                "`abstract` is only valid inside an `abstract class`",
                "an ordinary class cannot declare abstract members",
                Some("mark the class itself `abstract class` to declare abstract methods".into()),
            );
            self.pos += 1;
            self.synchronize();
            return None;
        } else {
            false
        };
        let is_override = self.eat_keyword(Keyword::Override);

        // `mut fn` marks a mutating method. It must be distinguished from
        // `mut` as a field modifier, so we only consume it when it is
        // immediately followed by `fn`.
        let is_mut = if self.check_keyword(Keyword::Mut)
            && self.peek_at(1) == &TokenKind::Keyword(Keyword::Fn)
        {
            self.pos += 1;
            true
        } else {
            false
        };

        if self.check_keyword(Keyword::Construct) {
            return self
                .parse_construct(visibility.unwrap_or(Visibility::Public), start)
                .map(ClassMember::Construct);
        }

        if self.check_keyword(Keyword::Fn) {
            return self
                .parse_method(
                    visibility.unwrap_or(Visibility::Public),
                    is_abstract,
                    is_override,
                    is_mut,
                    is_static,
                    start,
                )
                .map(ClassMember::Method);
        }

        self.parse_field(visibility, is_static, start).map(ClassMember::Field)
    }

    /// `public`, `private` or `protected`, if one is written.
    fn parse_visibility(&mut self) -> Option<Visibility> {
        let visibility = match self.peek() {
            TokenKind::Keyword(Keyword::Public) => Visibility::Public,
            TokenKind::Keyword(Keyword::Private) => Visibility::Private,
            TokenKind::Keyword(Keyword::Protected) => Visibility::Protected,
            _ => return None,
        };
        self.pos += 1;
        Some(visibility)
    }

    /// A field: `[visibility] [static] [mut|inmut|inmut::strict] name: Type [= expr];`
    ///
    /// Writing neither modifier means `public mut`
    /// (`ZIRK_LANGUAGE_SPEC.md` section 7). Both spellings produce the same
    /// member; only the flag remembers which was written.
    fn parse_field(&mut self, visibility: Option<Visibility>, is_static: bool, start: Span) -> Option<FieldDecl> {
        let mutability = if self.eat_keyword(Keyword::Mut) {
            Some(Mutability::Mutable)
        } else if self.eat_keyword(Keyword::Inmut) {
            // `strict` is contextual, recognized right after `inmut::`, the
            // same way `parse_let` handles it for local declarations.
            if self.eat(&TokenKind::ColonColon) {
                let is_strict =
                    matches!(self.peek(), TokenKind::Identifier(name) if name == "strict");
                if is_strict {
                    self.pos += 1;
                    Some(Mutability::Strict)
                } else {
                    let span = self.peek_span();
                    self.error(
                        codes::UNEXPECTED_TOKEN,
                        span,
                        "expected `strict` after `inmut::`",
                        "`inmut::strict` is the only qualified form of `inmut`",
                        None,
                    );
                    self.synchronize();
                    return None;
                }
            } else {
                Some(Mutability::Immutable)
            }
        } else {
            None
        };

        let name = self.expect_identifier("as the name of a field")?;
        self.expect(&TokenKind::Colon, "after the field name");
        let ty = self.parse_type()?;
        let default = if self.eat(&TokenKind::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(FieldDecl {
            name,
            ty,
            visibility: visibility.unwrap_or(Visibility::Public),
            mutability: mutability.unwrap_or(Mutability::Mutable),
            explicit_modifiers: visibility.is_some() || mutability.is_some(),
            default,
            is_static,
            span: start.to(end),
        })
    }

    fn parse_construct(&mut self, visibility: Visibility, start: Span) -> Option<ConstructDecl> {
        self.eat_keyword(Keyword::Construct);

        self.expect(&TokenKind::LParen, "after `construct`");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the parameter list");
        let body = self.parse_block()?;
        let span = start.to(body.span);

        Some(ConstructDecl {
            params,
            body,
            visibility,
            span,
        })
    }

    fn parse_method(
        &mut self,
        visibility: Visibility,
        is_abstract: bool,
        is_override: bool,
        is_mut: bool,
        is_static: bool,
        start: Span,
    ) -> Option<MethodDecl> {
        self.eat_keyword(Keyword::Fn);

        let name = self.expect_identifier("after `fn`")?;
        let type_params = self.parse_type_params();
        self.expect(&TokenKind::LParen, "after the method name");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the parameter list");
        let return_type = self.parse_return_type(&name)?;
        let throws = self.parse_throws_clause();

        // An `abstract` method declares a signature and stops there.
        let body = if is_abstract {
            let end = self.peek_span();
            if matches!(self.peek(), TokenKind::LBrace) {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    end,
                    "an `abstract` method has no body",
                    "it declares the signature its subclasses must implement",
                    Some("remove the body, or remove `abstract`".into()),
                );
                return None;
            }
            self.eat(&TokenKind::Semicolon);
            None
        } else {
            Some(self.parse_block()?)
        };

        let end = body.as_ref().map(|b| b.span).unwrap_or(name.span);

        Some(MethodDecl {
            name,
            type_params,
            is_override,
            is_mut,
            is_static,
            params,
            return_type,
            throws,
            body,
            visibility,
            is_abstract,
            span: start.to(end),
        })
    }

    /// Skips to the end of a class member after an error inside one.
    fn synchronize_member(&mut self) {
        let mut depth = 0usize;
        while !self.at_eof() {
            match self.peek() {
                TokenKind::LBrace => {
                    depth += 1;
                    self.pos += 1;
                }
                TokenKind::RBrace => {
                    if depth == 0 {
                        // It closes the class body, which the caller consumes.
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
                _ => self.pos += 1,
            }
        }
    }

    fn parse_enum(&mut self, shared: bool) -> Option<EnumDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Enum);

        let name = self.expect_identifier("after `enum`")?;
        let type_params = self.parse_type_params();
        self.expect(&TokenKind::LBrace, "after the enum name");

        let mut variants = Vec::new();
        while !matches!(self.peek(), TokenKind::RBrace) && !self.at_eof() {
            let Some(variant) = self.parse_enum_variant() else {
                break;
            };
            variants.push(variant);

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        let end = self.peek_span();
        self.expect(&TokenKind::RBrace, "to close the enum body");

        Some(EnumDecl {
            name,
            type_params,
            variants,
            shared,
            span: start.to(end),
        })
    }

    /// One `enum` variant: a bare name, `Name(Type, ...)` with associated
    /// data, or `Name -> value` with an explicit mapping. Never more than one
    /// of those at once.
    fn parse_enum_variant(&mut self) -> Option<EnumVariant> {
        let start = self.peek_span();
        let name = self.expect_identifier("as an enum variant")?;

        if self.eat(&TokenKind::LParen) {
            let mut associated = Vec::new();
            if !matches!(self.peek(), TokenKind::RParen) {
                loop {
                    let field_start = self.peek_span();
                    let field_name = self.expect_identifier("as an associated field's name")?;
                    self.expect(&TokenKind::Colon, "after the associated field's name");
                    let ty = self.parse_type()?;
                    let field_end = ty.span;
                    associated.push(AssociatedField {
                        name: field_name,
                        ty,
                        span: field_start.to(field_end),
                    });
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            let end = self.peek_span();
            self.expect(&TokenKind::RParen, "to close the variant's associated data");
            return Some(EnumVariant {
                name,
                associated,
                mapping: None,
                span: start.to(end),
            });
        }

        if self.eat(&TokenKind::Arrow) {
            let mapping = self.parse_expr()?;
            let end = mapping.span();
            return Some(EnumVariant {
                name,
                associated: Vec::new(),
                mapping: Some(mapping),
                span: start.to(end),
            });
        }

        Some(EnumVariant {
            span: name.span,
            name,
            associated: Vec::new(),
            mapping: None,
        })
    }

    /// `type UserLookup = Result<User, LookupError>;`
    fn parse_type_alias(&mut self, shared: bool) -> Option<TypeAliasDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Type);

        let name = self.expect_identifier("after `type`")?;
        self.expect(&TokenKind::Assign, "after the alias name");
        let target = self.parse_type()?;
        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(TypeAliasDecl {
            name,
            target,
            shared,
            span: start.to(end),
        })
    }

    fn parse_fn(&mut self, shared: bool, is_unsafe: bool) -> Option<FnDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Fn);

        let name = self.expect_identifier("after `fn`")?;
        let type_params = self.parse_type_params();

        self.expect(&TokenKind::LParen, "after the function name");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the parameter list");

        let return_type = self.parse_return_type(&name)?;
        let throws = self.parse_throws_clause();

        let body = self.parse_block()?;
        let span = start.to(body.span);

        Some(FnDecl {
            name,
            type_params,
            params,
            return_type,
            throws,
            body,
            shared,
            is_unsafe,
            span,
        })
    }

    /// `extern "C" fn name(params): ReturnType;` — a bodyless native
    /// declaration (roadmap Phase 4e, `ADR-015-declaracion-extern.md`).
    fn parse_extern_fn(&mut self) -> Option<ExternFnDecl> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Extern);

        let convention_span = self.peek_span();
        let convention = if let TokenKind::Str(value) = self.peek().clone() {
            self.pos += 1;
            StrLit {
                value,
                span: convention_span,
            }
        } else {
            self.error(
                codes::UNEXPECTED_TOKEN,
                convention_span,
                "expected a calling-convention literal",
                format!("found {} after `extern`", self.peek().description()),
                Some("write `extern \"C\" fn ...`".into()),
            );
            self.synchronize();
            return None;
        };

        if convention.value != "C" {
            self.error(
                codes::EXTERN_BAD_CONVENTION,
                convention.span,
                "unsupported calling convention",
                format!("found `\"{}\"`", convention.value),
                Some("only `\"C\"` is accepted as an extern calling convention".into()),
            );
        }

        self.expect(
            &TokenKind::Keyword(Keyword::Fn),
            "after the convention literal",
        );
        let name = self.expect_identifier("after `fn`")?;

        self.expect(&TokenKind::LParen, "after the function name");
        let params = self.parse_params();
        self.expect(&TokenKind::RParen, "to close the parameter list");

        let return_type = self.parse_return_type(&name)?;

        let span = if matches!(self.peek(), TokenKind::LBrace) {
            let body_span = self.peek_span();
            // A body IS parsed (not omitted from the grammar) so the
            // diagnostic can point at it precisely, per tasks.md 3.4.
            let body = self.parse_block();
            self.error(
                codes::EXTERN_HAS_BODY,
                body.map(|b| b.span).unwrap_or(body_span),
                "an `extern` declaration cannot have a body",
                "found a block after the extern signature",
                Some("end the declaration with `;` instead".into()),
            );
            start.to(body_span)
        } else {
            let semi_span = self.peek_span();
            self.expect(&TokenKind::Semicolon, "after an extern declaration");
            start.to(semi_span)
        };

        Some(ExternFnDecl {
            name,
            convention,
            params,
            return_type,
            span,
        })
    }

    /// The mandatory return type of a function or method.
    fn parse_return_type(&mut self, name: &Ident) -> Option<TypeRef> {
        if self.eat(&TokenKind::Colon) {
            return self.parse_type();
        }

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
        None
    }

    /// `throws Type (| Type)*`, if present (roadmap Phase 4b) — absent for an
    /// ordinary function/method, which declares no exception effect.
    fn parse_throws_clause(&mut self) -> Option<TypeRef> {
        if !self.eat_keyword(Keyword::Throws) {
            return None;
        }
        self.parse_type()
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

    /// A type, admitting `A | B | ...` (`ZIRK_LANGUAGE_SPEC.md` section 4).
    ///
    /// One alternative is [`Self::parse_type_atom`]; this only adds the `|`
    /// loop around it, so a caller that never wants a union in its position
    /// (a generic argument, a `from` constraint) calls the atom directly
    /// instead.
    fn parse_type(&mut self) -> Option<TypeRef> {
        let start = self.peek_span();
        let first = self.parse_type_atom()?;
        if !matches!(self.peek(), TokenKind::Pipe) {
            return Some(first);
        }

        let mut union_with = Vec::new();
        while self.eat(&TokenKind::Pipe) {
            union_with.push(self.parse_type_atom()?);
        }
        let end = union_with.last().map(|t| t.span).unwrap_or(first.span);

        let mut ty = first;
        ty.union_with = union_with;
        ty.span = start.to(end);
        Some(ty)
    }

    /// One alternative of a type: a name, its `<...>` arguments if generic,
    /// and a trailing `?`. Never a union on its own — see [`Self::parse_type`].
    fn parse_type_atom(&mut self) -> Option<TypeRef> {
        let span = self.peek_span();
        let name = match self.peek().clone() {
            TokenKind::Identifier(name) => {
                self.pos += 1;
                name
            }
            // `Pin<T>` is a Phase 4e built-in generic type. The name is a
            // keyword, but it is still valid in type position.
            TokenKind::Keyword(Keyword::Pin) => {
                self.pos += 1;
                "Pin".to_string()
            }
            _ => {
                let found = self.peek().description();
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    span,
                    "expected a type",
                    format!("found {found}"),
                    Some("the available types are Void, Int32, Boolean and String".into()),
                );
                return None;
            }
        };

        // `Fn(P...) => R` / `Function(P...) => R` (roadmap Phase 4d):
        // a real callable type in every position a type may appear.
        if (name == "Fn" || name == "Function") && matches!(self.peek(), TokenKind::LParen) {
            return self.parse_function_type(name, span);
        }

        let arguments = if matches!(self.peek(), TokenKind::Lt) {
            self.parse_type_args()?
        } else {
            Vec::new()
        };

        // `T?` is `T | Null`, per `ZIRK_LANGUAGE_SPEC.md` section 4.
        if matches!(self.peek(), TokenKind::Question) {
            let end = self.peek_span();
            self.pos += 1;
            let mut ty = TypeRef::nullable(name, span.to(end));
            ty.arguments = arguments;
            return Some(ty);
        }

        let mut ty = TypeRef::new(name, span);
        ty.arguments = arguments;
        Some(ty)
    }

    /// `Fn(P...) => R` or `Function(P...) => R` in type position (roadmap
    /// Phase 4d) — `name` is already consumed, `(` is next.
    fn parse_function_type(&mut self, name: String, start: Span) -> Option<TypeRef> {
        self.pos += 1; // `(`

        let mut params = Vec::new();
        if !matches!(self.peek(), TokenKind::RParen) {
            loop {
                let Some(param) = self.parse_fn_type_param() else {
                    break;
                };
                params.push(param);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }

        if !self.expect(
            &TokenKind::RParen,
            "to close the callable type's parameters",
        ) {
            return None;
        }

        if !self.expect(&TokenKind::FatArrow, "after a callable type's parameters") {
            return None;
        }

        let returns = self.parse_type()?;
        let end = returns.span;

        Some(TypeRef::function(
            name,
            FnTypeRef {
                params,
                returns: Box::new(returns),
            },
            start.to(end),
        ))
    }

    /// One parameter of a written `Fn(...)` type: `T`, `name: T`, `name?: T`,
    /// or `...name: T`. A label needs two tokens of lookahead
    /// ([`Self::peek_at`]) to tell apart from a bare type: `String` alone is
    /// a type, `name: String` is a labeled one, and only the token after the
    /// identifier (`:`, or `?` then `:`) says which.
    fn parse_fn_type_param(&mut self) -> Option<FnTypeParamRef> {
        let start = self.peek_span();
        let variadic = self.eat(&TokenKind::DotDotDot);

        let labeled = matches!(self.peek(), TokenKind::Identifier(_))
            && (matches!(self.peek_at(1), TokenKind::Colon)
                || (matches!(self.peek_at(1), TokenKind::Question)
                    && matches!(self.peek_at(2), TokenKind::Colon)));

        let (label, optional) = if labeled {
            let name = self.expect_identifier("as a callable type's parameter label")?;
            let optional = self.eat(&TokenKind::Question);
            self.pos += 1; // `:`
            (Some(name), optional)
        } else {
            (None, false)
        };

        let ty = self.parse_type()?;
        let end = ty.span;

        Some(FnTypeParamRef {
            label,
            optional,
            variadic,
            ty,
            span: start.to(end),
        })
    }

    /// `<Int32, String>` in `Map<Int32, String>`, the `<` already consumed by
    /// the caller having peeked it.
    fn parse_type_args(&mut self) -> Option<Vec<TypeRef>> {
        self.pos += 1; // `<`

        let mut arguments = Vec::new();
        if !self.check_gt() {
            loop {
                // A union as a type argument is out of scope for now: it
                // would need `check_gt`-style splitting for the `|` inside
                // `Box<A | B>` too, which nothing exercises yet.
                arguments.push(self.parse_type_atom()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }

        if !self.eat_gt() {
            let span = self.peek_span();
            let found = self.peek().description();
            self.error(
                codes::UNEXPECTED_TOKEN,
                span,
                "expected `>` to close the type arguments",
                format!("found {found}"),
                None,
            );
            return None;
        }

        Some(arguments)
    }

    /// `<T from A & B, U>` on a class, function or method, absent when the
    /// declaration is not generic.
    fn parse_type_params(&mut self) -> Vec<TypeParam> {
        if !matches!(self.peek(), TokenKind::Lt) {
            return Vec::new();
        }
        self.pos += 1; // `<`

        let mut params = Vec::new();
        loop {
            let start = self.peek_span();
            let variance = if self.eat_keyword(Keyword::In) {
                Variance::In
            } else if self.eat_keyword(Keyword::Out) {
                Variance::Out
            } else {
                Variance::Invariant
            };
            let Some(name) = self.expect_identifier("as a type parameter") else {
                break;
            };

            let mut constraints = Vec::new();
            if self.eat_keyword(Keyword::From) {
                loop {
                    let Some(constraint) = self.parse_type_atom() else {
                        break;
                    };
                    constraints.push(constraint);
                    if !self.eat(&TokenKind::Amp) {
                        break;
                    }
                }
            }

            let default = if self.eat(&TokenKind::Assign) {
                let Some(ty) = self.parse_type_atom() else {
                    break;
                };
                Some(ty)
            } else {
                None
            };

            let end = default
                .as_ref()
                .or_else(|| constraints.last())
                .map(|c| c.span)
                .unwrap_or(name.span);
            params.push(TypeParam {
                name,
                variance,
                constraints,
                default,
                span: start.to(end),
            });

            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }

        if !self.eat_gt() {
            let span = self.peek_span();
            let found = self.peek().description();
            self.error(
                codes::UNEXPECTED_TOKEN,
                span,
                "expected `>` to close the type parameters",
                format!("found {found}"),
                None,
            );
        }

        params
    }

    // --- Statements -------------------------------------------------------

    fn parse_block(&mut self) -> Option<Block> {
        if !self.enter() {
            self.leave();
            return None;
        }
        let parsed = self.parse_block_nested();
        self.leave();
        parsed
    }

    fn parse_block_nested(&mut self) -> Option<Block> {
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
        if self.check_keyword(Keyword::Do) {
            return self.parse_do_while();
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
        if self.check_keyword(Keyword::Throw) {
            return self.parse_throw();
        }
        if self.check_keyword(Keyword::Try) {
            return self.parse_try();
        }
        if self.check_keyword(Keyword::Unsafe) {
            return self.parse_unsafe_block().map(Stmt::Unsafe);
        }
        if self.check_keyword(Keyword::Commit) {
            return self.parse_commit_block().map(Stmt::Commit);
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

    /// `for init; cond; step { }` and `for x in iterable { }`, each with
    /// optional parentheses around the header.
    ///
    /// Both start with `for`, and which one it is only becomes clear after the
    /// binding.
    ///
    /// Without parentheses, what closes the header is the `{` of the body.
    /// That works because Zirk has no brace-delimited literal in expression
    /// position — records are built as `Type(...)` — so the ambiguity that
    /// forces other languages to require the parentheses never arises.
    fn parse_for(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::For);

        // The parenthesis is optional, so it cannot be used to tell the two
        // forms apart: both are probed after consuming it.
        let parenthesized = self.eat(&TokenKind::LParen);

        // `for x in ...`: an identifier followed by `in`.
        if let TokenKind::Identifier(name) = self.peek().clone() {
            let binding_span = self.peek_span();
            if self.tokens.get(self.pos + 1).map(|t| &t.kind)
                == Some(&TokenKind::Keyword(Keyword::In))
            {
                self.pos += 2;
                let binding = Ident::new(name, binding_span);
                let iterable = self.parse_expr()?;
                if parenthesized {
                    self.expect(&TokenKind::RParen, "to close the `for` header");
                }
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

        // The step ends at the `{` of the body, or at the `)` when the header
        // was written with parentheses.
        let step = if self.header_ended(parenthesized) {
            None
        } else {
            let stmt = self.parse_simple_stmt()?;
            Some(Box::new(stmt))
        };
        if parenthesized {
            self.expect(&TokenKind::RParen, "to close the `for` header");
        }

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

    /// Whether the `for` header is over, which depends on how it was opened.
    fn header_ended(&self, parenthesized: bool) -> bool {
        if parenthesized {
            matches!(self.peek(), TokenKind::RParen)
        } else {
            matches!(self.peek(), TokenKind::LBrace)
        }
    }

    /// `do { } while cond;`
    ///
    /// The body always runs once: the condition is checked after it, which is
    /// the only difference from `while` and is one edge in the lowering.
    fn parse_do_while(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Do);

        let body = self.parse_block()?;
        self.expect_keyword(Keyword::While, "after the body of a `do`");
        let condition = self.parse_expr()?;
        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Loop(LoopStmt {
            kind: LoopKind::DoWhile,
            init: None,
            condition: Some(condition),
            step: None,
            body,
            span: start.to(end),
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
            // `strict` is contextual, recognized by position right after
            // `inmut::`, the same way `value` is only special right before
            // `class`.
            if self.eat(&TokenKind::ColonColon) {
                let is_strict =
                    matches!(self.peek(), TokenKind::Identifier(name) if name == "strict");
                if is_strict {
                    self.pos += 1;
                    Mutability::Strict
                } else {
                    let span = self.peek_span();
                    self.error(
                        codes::UNEXPECTED_TOKEN,
                        span,
                        "expected `strict` after `inmut::`",
                        "`inmut::strict` is the only qualified form of `inmut`",
                        None,
                    );
                    self.synchronize();
                    return None;
                }
            } else {
                Mutability::Immutable
            }
        };

        let pattern = self.parse_pattern()?;

        // A comma-grouped declaration (roadmap Phase 4d): `mut first, second:
        // String;`. Kept as its own node (design D1/D2) rather than folding
        // this into `LetStmt` — the single-name path below is left untouched.
        // Only a bare binding pattern can start a comma-grouped list.
        if let Pattern::Binding(first) = &pattern
            && matches!(self.peek(), TokenKind::Comma)
        {
            return self.parse_multi_let(start, mutability, first.clone());
        }

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
            let span = pattern.span();
            let name = pattern
                .binding_name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "_".to_string());
            self.error(
                codes::UNTYPED_DECLARATION,
                span,
                format!("cannot determine the type of `{}`", name),
                "the declaration has neither a type annotation nor an initial value",
                Some(format!(
                    "write `{}: Int32` or give it an initial value",
                    name
                )),
            );
        }

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Let(LetStmt {
            mutability,
            pattern,
            ty,
            init,
            span: start.to(end),
        }))
    }

    /// `mut first, second: String = a, b;` — the comma-grouped form of
    /// `Self::parse_let`, entered once a comma is seen after the first name.
    ///
    /// The initializer list's arity is preserved exactly as written, even
    /// when it does not match `names.len()`: the checker emits the targeted
    /// diagnostic (design D1/D5), not the parser.
    fn parse_multi_let(
        &mut self,
        start: Span,
        mutability: Mutability,
        first: Ident,
    ) -> Option<Stmt> {
        let mut names = vec![first];
        while self.eat(&TokenKind::Comma) {
            let name = self.expect_identifier("in a comma-separated declaration list")?;
            names.push(name);
        }

        let ty = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let inits = if self.eat(&TokenKind::Assign) {
            let mut list = vec![self.parse_expr()?];
            while self.eat(&TokenKind::Comma) {
                list.push(self.parse_expr()?);
            }
            list
        } else {
            Vec::new()
        };

        if ty.is_none() && inits.is_empty() {
            let span = start.to(names.last().expect("at least one name").span);
            self.error(
                codes::UNTYPED_DECLARATION,
                span,
                "cannot determine the type of this declaration",
                "the declaration has neither a type annotation nor an initial value",
                Some("write a shared `: Type` or give every name an initial value".into()),
            );
        }

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::MultiLet(MultiLetStmt {
            mutability,
            names,
            ty,
            inits,
            span: start.to(end),
        }))
    }

    fn parse_if(&mut self) -> Option<IfStmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::If);

        let condition = self.parse_expr()?;

        // The effect-only form of `LANGUAGE_SPEC` section 5: `if closed
        // return;` governs exactly one statement.
        let braceless = !matches!(self.peek(), TokenKind::LBrace);
        let then_branch = if braceless {
            self.parse_governed_statement()?
        } else {
            self.parse_block()?
        };

        // `else` over the braceless form would bring back the dangling-else
        // ambiguity, and the norm presents that form as an effect rather than
        // as a complete conditional.
        if braceless && self.check_keyword(Keyword::Else) {
            let span = self.peek_span();
            self.error(
                codes::UNEXPECTED_TOKEN,
                span,
                "an `if` without braces cannot have an `else`",
                "the form without braces governs one statement and nothing more",
                Some("wrap both branches in `{ }`".into()),
            );
            return None;
        }

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

    /// The single statement an `if` without braces governs.
    ///
    /// It is wrapped in a block so nothing downstream has to know the form
    /// exists: scoping, flow analysis and lowering see the same shape they see
    /// for `if closed { return; }`, which is what it means.
    fn parse_governed_statement(&mut self) -> Option<Block> {
        let stmt = self.parse_stmt()?;
        let span = stmt.span();

        Some(Block {
            statements: vec![stmt],
            span,
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

    /// `unsafe { ... }` (roadmap Phase 4e). Parses in both statement and
    /// expression position — see [`Stmt::Unsafe`]/[`Expr::Unsafe`].
    fn parse_unsafe_block(&mut self) -> Option<UnsafeBlock> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Unsafe);
        let body = self.parse_block()?;
        Some(UnsafeBlock {
            span: start.to(body.span),
            body,
        })
    }

    /// `commit { ... }` (roadmap Phase 4e). Parses anywhere `unsafe {}` does
    /// — whether it is nested inside an enclosing `unsafe` is the checker's
    /// job (design D3, tasks.md 3.3), not the grammar's.
    fn parse_commit_block(&mut self) -> Option<CommitBlock> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Commit);
        let body = self.parse_block()?;
        Some(CommitBlock {
            span: start.to(body.span),
            body,
        })
    }

    /// `throw expr;` / `throw;` (roadmap Phase 4b).
    ///
    /// Whether a bare `throw;` is legal (only directly inside a `catch`) is
    /// not a syntactic question — the checker decides it, the same way
    /// `break`/`continue` outside a loop parse fine and are rejected later.
    fn parse_throw(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Throw);

        let value = if matches!(self.peek(), TokenKind::Semicolon | TokenKind::RBrace) {
            None
        } else {
            Some(self.parse_expr()?)
        };

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::Throw(ThrowStmt {
            value,
            span: start.to(end),
        }))
    }

    /// `try { } catch Type(name) { } ... finally { }` (roadmap Phase 4b).
    fn parse_try(&mut self) -> Option<Stmt> {
        let start = self.peek_span();
        self.eat_keyword(Keyword::Try);

        let body = self.parse_block()?;

        let mut catches = Vec::new();
        while self.check_keyword(Keyword::Catch) {
            let catch_start = self.peek_span();
            self.eat_keyword(Keyword::Catch);

            let ty = self.parse_type_atom()?;
            self.expect(&TokenKind::LParen, "after the caught type");
            let binding = self.expect_identifier("naming the caught value")?;
            self.expect(&TokenKind::RParen, "after the catch binding");
            let catch_body = self.parse_block()?;
            let catch_end = catch_body.span;

            catches.push(CatchClause {
                ty,
                binding,
                body: catch_body,
                span: catch_start.to(catch_end),
            });
        }

        let finally = if self.eat_keyword(Keyword::Finally) {
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = finally
            .as_ref()
            .map(|b| b.span)
            .or_else(|| catches.last().map(|c| c.span))
            .unwrap_or(body.span);

        if catches.is_empty() && finally.is_none() {
            self.error(
                codes::EMPTY_TRY,
                end,
                "a `try` needs at least one `catch` or a `finally`",
                "with neither, it does nothing a plain block would not",
                Some("add a `catch Type(name) { }`, or a `finally { }`".into()),
            );
        }

        Some(Stmt::Try(TryStmt {
            body,
            catches,
            finally,
            span: start.to(end),
        }))
    }

    /// A statement starting with an expression: an assignment or a call.
    ///
    /// Compound assignment and increment are desugared here: `i += 1`, `i++`
    /// and `++i` all produce the tree of `i = i + 1`, so nothing downstream
    /// needs to know they exist.
    ///
    /// The two increment forms only differ in the value they produce, and a
    /// statement discards it. Desugaring both to the same assignment is what
    /// keeps that distinction out of every layer that does not need it.
    fn parse_expr_or_assign(&mut self) -> Option<Stmt> {
        let start = self.peek_span();

        let expr = self.parse_expr_inner()?;

        // Simultaneous assignment (roadmap Phase 4d): `left, right = right,
        // left;`. A comma here can only start a comma-separated list of
        // assignment targets — there is no other statement shape that puts a
        // bare top-level comma after a leading expression.
        if matches!(self.peek(), TokenKind::Comma) {
            return self.parse_multi_assign(start, expr);
        }

        // An increment that *is* the whole statement: its value goes nowhere.
        if let Expr::Increment(inc) = &expr {
            let end = self.peek_span();
            self.eat(&TokenKind::Semicolon);
            let one = one(inc.op_span);
            return Some(self.desugar_compound(
                inc.target.clone(),
                inc.op.as_binary(),
                one,
                inc.op_span,
                start.to(end),
            ));
        }

        // `a **= b` desugars to `a = a.pow(b)` (`exponentiation-operator`).
        // Handled apart from `compound_op` because `**` has no `BinaryOp`
        // variant — it is a method call, not a native binary operator.
        if matches!(self.peek(), TokenKind::StarStarEq) {
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
            let read = match &target {
                AssignTarget::Name(ident) => Expr::Path(ident.clone()),
                AssignTarget::Field(field) => Expr::Field(field.clone()),
                AssignTarget::Index(index) => Expr::Index(index.clone()),
            };
            return Some(Stmt::Assign(AssignStmt {
                target,
                value: pow_call(read, value, op_span),
                span: start.to(end),
            }));
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

            let Some(target) = as_assignable(&expr) else {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    start,
                    "the left-hand side of an assignment must be a place",
                    "only a name or a field names storage that can be written",
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

    /// `left, right = right, left;` — the comma-grouped form of assignment,
    /// entered once a comma is seen after the first parsed expression.
    ///
    /// Both lists' arities are preserved exactly as parsed, even when they
    /// differ (design D1/D5's "Assignment arity mismatch" scenario) — the
    /// checker reports the mismatch, not the parser.
    fn parse_multi_assign(&mut self, start: Span, first_expr: Expr) -> Option<Stmt> {
        let mut targets = Vec::new();

        let Some(first_target) = as_assignable(&first_expr) else {
            self.error(
                codes::UNEXPECTED_TOKEN,
                first_expr.span(),
                "the left-hand side of an assignment must be a place",
                "only a name or a field names storage that can be written",
                None,
            );
            return None;
        };
        targets.push(first_target);

        while self.eat(&TokenKind::Comma) {
            let expr = self.parse_expr_inner()?;
            let Some(target) = as_assignable(&expr) else {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    expr.span(),
                    "the left-hand side of an assignment must be a place",
                    "only a name or a field names storage that can be written",
                    None,
                );
                return None;
            };
            targets.push(target);
        }

        self.expect(
            &TokenKind::Assign,
            "after a comma-separated list of assignment targets",
        );

        let mut values = vec![self.parse_expr()?];
        while self.eat(&TokenKind::Comma) {
            values.push(self.parse_expr()?);
        }

        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(Stmt::MultiAssign(MultiAssignStmt {
            targets,
            values,
            span: start.to(end),
        }))
    }

    /// Builds the `target = target <op> value` that a compound form stands for.
    fn desugar_compound(
        &mut self,
        target: AssignTarget,
        op: BinaryOp,
        value: Expr,
        op_span: Span,
        span: Span,
    ) -> Stmt {
        // The place is read and written once each, in that order, which is
        // what the compound form means.
        let read = match &target {
            AssignTarget::Name(ident) => Expr::Path(ident.clone()),
            AssignTarget::Field(field) => Expr::Field(field.clone()),
            AssignTarget::Index(index) => Expr::Index(index.clone()),
        };
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
    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_expr_inner()
    }

    fn parse_expr_inner(&mut self) -> Option<Expr> {
        if !self.enter() {
            self.leave();
            return None;
        }
        let parsed = self.parse_expr_nested();
        self.leave();
        parsed
    }

    fn parse_expr_nested(&mut self) -> Option<Expr> {
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
        if self.check_keyword(Keyword::Unsafe) {
            return self.parse_unsafe_block().map(|b| Expr::Unsafe(Box::new(b)));
        }
        if self.check_keyword(Keyword::Commit) {
            return self.parse_commit_block().map(|b| Expr::Commit(Box::new(b)));
        }

        self.parse_ternary()
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

        let first_expr = self.parse_range()?;

        // `match scrutinee with binding { ... }` (roadmap Phase 4c) scopes a
        // `Resource<E>`: `binding` names whichever arm's pattern acquires
        // one, closed automatically on every exit from that arm.
        if !self.eat_keyword(Keyword::With) {
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

            return Some(Expr::Match(MatchExpr {
                scrutinee: Box::new(first_expr),
                with_binding: None,
                arms,
                acquisitions: Vec::new(),
                body: None,
                error: None,
                span: start.to(end),
            }));
        }

        let first_binding = self.expect_identifier("after `with`")?;

        // A comma after the first `with` starts a grouped acquisition list.
        // Otherwise the single `match ... with` form falls back to match arms.
        if !matches!(self.peek(), TokenKind::Comma) {
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

            return Some(Expr::Match(MatchExpr {
                scrutinee: Box::new(first_expr),
                with_binding: Some(first_binding),
                arms,
                acquisitions: Vec::new(),
                body: None,
                error: None,
                span: start.to(end),
            }));
        }

        let mut acquisitions = vec![MatchAcquisition {
            expr: Box::new(first_expr),
            binding: first_binding,
        }];

        while self.eat(&TokenKind::Comma) {
            let expr = self.parse_range()?;
            self.expect(
                &TokenKind::Keyword(Keyword::With),
                "between the expression and its binding in a grouped `match with`",
            );
            let binding = self.expect_identifier("after `with`")?;
            acquisitions.push(MatchAcquisition {
                expr: Box::new(expr),
                binding,
            });
        }

        let body_block = self.parse_block()?;
        let body = ArmBody::Block(body_block);
        let mut end = body.span();

        let error = if let TokenKind::Identifier(name) = self.peek().clone() {
            if name == "error" {
                self.pos += 1;
                self.expect(&TokenKind::LBrace, "before the error branch");
                let arm = self.parse_match_arm()?;
                self.expect(&TokenKind::RBrace, "to close the error branch");
                end = arm.span;
                Some(Box::new(arm))
            } else {
                None
            }
        } else {
            None
        };

        Some(Expr::Match(MatchExpr {
            scrutinee: acquisitions[0].expr.clone(),
            with_binding: Some(acquisitions[0].binding.clone()),
            arms: Vec::new(),
            acquisitions,
            body: Some(Box::new(body)),
            error,
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
            // `re'pattern' name?` — the optional trailing identifier binds
            // the `Regex.Match` inside the arm.
            TokenKind::Regex(pattern) => {
                self.pos += 1;
                let mut end = span;
                let binding = if let TokenKind::Identifier(name) = self.peek().clone() {
                    let binding_span = self.peek_span();
                    self.pos += 1;
                    end = binding_span;
                    Some(Ident::new(name, binding_span))
                } else {
                    None
                };
                Some(Pattern::Regex(RegexPattern {
                    pattern,
                    binding,
                    span: span.to(end),
                }))
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
            TokenKind::LParen => {
                self.pos += 1;
                let mut elements = Vec::new();
                if !matches!(self.peek(), TokenKind::RParen) {
                    loop {
                        elements.push(self.parse_pattern()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end = self.peek_span();
                self.expect(&TokenKind::RParen, "to close the tuple pattern");
                Some(Pattern::Tuple(TuplePattern {
                    elements,
                    span: span.to(end),
                }))
            }
            TokenKind::Identifier(name) => {
                self.pos += 1;

                // `Direction.North` names a variant; a bare name binds.
                if matches!(self.peek(), TokenKind::Dot) {
                    self.pos += 1;
                    let variant = self.expect_identifier("after the enum name")?;

                    let mut end = variant.span;
                    let mut bindings = Vec::new();
                    if self.eat(&TokenKind::LParen) {
                        if !matches!(self.peek(), TokenKind::RParen) {
                            loop {
                                bindings.push(self.parse_pattern()?);
                                if !self.eat(&TokenKind::Comma) {
                                    break;
                                }
                            }
                        }
                        end = self.peek_span();
                        self.expect(&TokenKind::RParen, "to close the variant's bindings");
                    }

                    return Some(Pattern::Variant(VariantPattern {
                        span: span.to(end),
                        enum_name: Ident::new(name, span),
                        variant,
                        bindings,
                    }));
                }

                // `Ok(file)` and `Error(error)` are the unqualified forms of
                // the built-in `Result<T,E>` variants; the parser resolves them
                // to `Result` synthetically so the rest of the pipeline treats
                // them like any other qualified variant.
                if (name == "Ok" || name == "Error") && matches!(self.peek(), TokenKind::LParen) {
                    let mut bindings = Vec::new();
                    self.pos += 1;
                    if !matches!(self.peek(), TokenKind::RParen) {
                        loop {
                            bindings.push(self.parse_pattern()?);
                            if !self.eat(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let end = self.peek_span();
                    self.expect(&TokenKind::RParen, "to close the variant's bindings");
                    let variant = Ident::new(name, span);
                    return Some(Pattern::Variant(VariantPattern {
                        span: span.to(end),
                        enum_name: Ident::new("Result", span),
                        variant,
                        bindings,
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

    /// `cond ? a : b`, which binds looser than everything else here.
    ///
    /// Right-associative, per level 16 of the operator table: `a ? b : c ? d :
    /// e` groups as `a ? b : (c ? d : e)`, which is the only reading in which
    /// the trailing branches mean anything.
    ///
    /// A `?` after an expression is unambiguously this operator: `??` and `?.`
    /// are their own tokens, and the `?` of `T?` only appears in type position.
    fn parse_ternary(&mut self) -> Option<Expr> {
        let condition = self.parse_range()?;

        if !matches!(self.peek(), TokenKind::Question) {
            return Some(condition);
        }
        let op_span = self.peek_span();
        self.pos += 1;

        let when_true = self.parse_ternary()?;
        self.expect(&TokenKind::Colon, "to separate the branches of the ternary");
        let when_false = self.parse_ternary()?;

        Some(Expr::Ternary(TernaryExpr {
            span: condition.span().to(when_false.span()),
            condition: Box::new(condition),
            when_true: Box::new(when_true),
            when_false: Box::new(when_false),
            op_span,
        }))
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

        // `start..end..step` (roadmap Phase 7): a second `..` — never `..=`,
        // the step does not bind a bound — introduces the step.
        let step = if self.eat(&TokenKind::DotDot) {
            Some(Box::new(self.parse_binary(0)?))
        } else {
            None
        };

        let span = step
            .as_ref()
            .map(|s| start.span().to(s.span()))
            .unwrap_or_else(|| start.span().to(end.span()));

        Some(Expr::Range(RangeExpr {
            span,
            start: Box::new(start),
            end: Box::new(end),
            step,
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

        // `++i` and `--i` in value position: the operand is updated first and
        // the expression is the new value (`LANGUAGE_SPEC` section 4).
        if let Some(op) = increment_op(self.peek()) {
            let op_span = self.peek_span();
            self.pos += 1;
            let name = self.expect_identifier("after the increment operator")?;
            let span = start.to(name.span);
            let target = AssignTarget::Name(name);
            return Some(Expr::Increment(IncrementExpr {
                target,
                op,
                fix: IncrementFix::Prefix,
                op_span,
                span,
            }));
        }

        let op = match self.peek() {
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Not => Some(UnaryOp::Not),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        };

        if let Some(op) = op {
            self.pos += 1;

            // `-<integer>` is one literal, not a negation applied to one.
            // Without folding it here, `-2147483648` would be rejected: its
            // magnitude does not fit in `Int32` even though the value does.
            if op == UnaryOp::Neg
                && let TokenKind::Integer(value) = self.peek().clone()
                // `-2 ** 2` is `-(2 ** 2)`: when the literal is the base of a
                // power, do not fold the sign into it — let the unary wrap the
                // whole power expression, consistent with `-x ** 2`.
                && !matches!(self.peek_at(1), TokenKind::StarStar)
            {
                let end = self.peek_span();
                self.pos += 1;
                return Some(Expr::Int(IntLit {
                    value: -value,
                    span: start.to(end),
                }));
            }

            let operand = self.parse_unary()?;
            let span = start.to(operand.span());
            return Some(Expr::Unary(UnaryExpr {
                op,
                operand: Box::new(operand),
                span,
            }));
        }

        // Member access binds tighter than everything else (level 1 of the
        // operator table), so it is applied to the primary before anything.
        let primary = self.parse_primary()?;
        let expr = self.parse_member_chain(primary)?;

        // `**` binds tighter than unary and multiplicative and is
        // right-associative (`exponentiation-operator`). Applied here, to the
        // postfix-complete operand, so a leading unary `-`/`!` wraps the whole
        // power expression.
        if matches!(self.peek(), TokenKind::StarStar) {
            return self.parse_power_tail(expr);
        }

        // `i++` and `i--` in value position: the expression is the previous
        // value and the operand is updated afterwards.
        if let Some(op) = increment_op(self.peek()) {
            let op_span = self.peek_span();

            let Some(target) = as_assignable(&expr) else {
                self.error(
                    codes::UNEXPECTED_TOKEN,
                    expr.span(),
                    "only a variable can be incremented",
                    format!("the operand of `{}` must be a name", op.as_str()),
                    None,
                );
                return None;
            };
            self.pos += 1;

            return Some(Expr::Increment(IncrementExpr {
                span: expr.span().to(op_span),
                target,
                op,
                fix: IncrementFix::Postfix,
                op_span,
            }));
        }

        Some(expr)
    }

    /// `base ** exponent` (`exponentiation-operator`): consumes the `**` and
    /// its right operand and returns the desugared tree `base.pow(exponent)`.
    ///
    /// The exponent is parsed with [`Self::parse_unary`], which recurses
    /// through this same tail — that is what makes `**` right-associative
    /// (`2 ** 3 ** 2` is `2 ** (3 ** 2)`) and lets the exponent be a unary
    /// expression (`2 ** -1`).
    fn parse_power_tail(&mut self, base: Expr) -> Option<Expr> {
        let op_span = self.peek_span();
        self.pos += 1; // the `**`
        let exponent = self.parse_unary()?;
        Some(pow_call(base, exponent, op_span))
    }

    /// Parses `"text {expr} text"` into its literal and expression parts
    /// (roadmap Phase 3b).
    ///
    /// Each `{expr}` was kept as raw source text by the lexer rather than
    /// tokenized inline (see `StrPart::Expr`'s own doc comment): it is
    /// tokenized here, on its own, and every resulting span is shifted by
    /// the offset the interpolation started at — so a diagnostic inside
    /// `{expr}` points at its real place in the file, not at offset 0 of a
    /// text nobody wrote as its own file.
    fn parse_interpolated(&mut self, parts: Vec<StrPart>, span: Span) -> Option<Expr> {
        let mut result = Vec::with_capacity(parts.len());

        for part in parts {
            match part {
                StrPart::Literal(text) => result.push(InterpolatedPart::Literal(text)),
                StrPart::Expr {
                    text,
                    span: inner_span,
                } => {
                    let sub_source = SourceFile::new("<interpolation>", text);
                    let sub_tokens: Vec<Token> = tokenize(&sub_source, self.sink)
                        .into_iter()
                        .map(|t| Token {
                            kind: t.kind,
                            span: Span::in_file(
                                inner_span.file,
                                inner_span.start + t.span.start,
                                inner_span.start + t.span.end,
                            ),
                        })
                        .collect();

                    let mut sub = Parser::new(self.source, &sub_tokens, self.sink);
                    let expr = sub.parse_expr()?;
                    if !sub.at_eof() {
                        let extra = sub.peek_span();
                        self.error(
                            codes::UNEXPECTED_TOKEN,
                            extra,
                            "unexpected token after the interpolated expression",
                            "an interpolation holds exactly one expression",
                            None,
                        );
                        return None;
                    }
                    result.push(InterpolatedPart::Expr(expr));
                }
            }
        }

        Some(Expr::Interpolated(InterpolatedStrExpr {
            parts: result,
            span,
        }))
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
            TokenKind::Float(lit) => {
                self.pos += 1;
                Some(Expr::Float(FloatLit {
                    text: lit.text,
                    width: lit.width,
                    span,
                }))
            }
            TokenKind::Char(value) => {
                self.pos += 1;
                Some(Expr::Char(CharLit { value, span }))
            }
            // Literals the norm defines and this phase does not implement.
            // They are recognized so the diagnostic can name them and their
            // phase; without the lexeme there would be nothing to name.
            TokenKind::InterpolatedStr(parts) => {
                self.pos += 1;
                self.parse_interpolated(parts, span)
            }
            TokenKind::Duration(lit, unit) => {
                self.pos += 1;
                Some(Expr::Duration(DurationLit {
                    nanos: duration_nanos(&lit.text, unit),
                    span,
                }))
            }
            TokenKind::Regex(pattern) => {
                self.pos += 1;
                Some(Expr::Regex(RegexLit { pattern, span }))
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
            TokenKind::Keyword(Keyword::This) => {
                self.pos += 1;
                Some(Expr::This(ThisExpr { span }))
            }
            TokenKind::Keyword(Keyword::Super) => {
                self.pos += 1;
                Some(Expr::Super(SuperExpr {
                    trait_name: None,
                    span,
                }))
            }
            // `Pin<T>` and `Pin(obj)` are built-in generic surface syntax; the
            // name is a keyword, but in expression position it acts like the
            // identifier `Pin` so `Pin(c)` parses as a call.
            TokenKind::Keyword(Keyword::Pin) => {
                self.pos += 1;
                self.parse_after_ident(Ident::new("Pin", span))
            }
            // Kept out of this arm's body on purpose: `parse_primary` runs
            // once per level of a deeply nested expression, and every arm's
            // locals count against that one shared frame.
            TokenKind::Keyword(Keyword::Transfer) => self.parse_transfer(span),
            // `<Type>expr`, the prefix spelling of a cast. Unambiguous: `<`
            // never starts a primary expression otherwise — comparison needs
            // a left operand, which nothing precedes here.
            //
            // Kept out of this match arm's own body on purpose: `parse_primary`
            // runs once per level of a deeply nested expression (parentheses
            // recurse through it), and in a debug build every arm's locals
            // count against that one shared frame — this one's locals ran the
            // 128-deep nesting guard test out of a 2 MB stack before it could
            // even report the diagnostic.
            TokenKind::Lt => self.parse_prefix_cast(span),
            TokenKind::LParen => {
                self.pos += 1;
                // A parenthesized expression has one value; a comma makes it
                // a tuple literal `(a, b, ...)` (roadmap Phase 3b).
                let first = self.parse_expr()?;
                if !self.eat(&TokenKind::Comma) {
                    self.expect(&TokenKind::RParen, "to close the parenthesis");
                    return Some(first);
                }
                let mut elements = vec![first];
                if !matches!(self.peek(), TokenKind::RParen) {
                    loop {
                        elements.push(self.parse_expr()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let end = self.peek_span();
                self.expect(&TokenKind::RParen, "to close the tuple");
                Some(Expr::Tuple(TupleExpr {
                    elements,
                    span: span.to(end),
                }))
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

    /// `<Type>expr`, the prefix spelling of a cast, once `parse_primary` has
    /// already consumed nothing but seen the leading `<`.
    fn parse_prefix_cast(&mut self, span: Span) -> Option<Expr> {
        self.pos += 1; // `<`
        let target = self.parse_type_atom()?;
        if !self.eat_gt() {
            let found = self.peek().description();
            self.error(
                codes::UNEXPECTED_TOKEN,
                self.peek_span(),
                "expected `>` to close the cast's type",
                format!("found {found}"),
                None,
            );
            return None;
        }
        // Only the primary that follows, not its own member chain: `<T>(x).field`
        // casts `(x)` and then reads `.field` off the cast — the outer
        // `parse_member_chain` applies that once this returns, the same way
        // it would to any other primary.
        let operand = self.parse_primary()?;
        Some(Expr::Cast(CastExpr {
            span: span.to(operand.span()),
            expr: Box::new(operand),
            target,
            optional: false,
        }))
    }

    /// `transfer(<expr>)`, factored out of `parse_primary` to keep its
    /// stack frame small for deeply nested expressions.
    fn parse_transfer(&mut self, span: Span) -> Option<Expr> {
        self.pos += 1; // `transfer`
        self.expect(&TokenKind::LParen, "after `transfer`");
        let expr = self.parse_expr()?;
        let end = self.peek_span();
        self.expect(&TokenKind::RParen, "to close `transfer`");
        Some(Expr::Transfer(TransferExpr {
            expr: Box::new(expr),
            span: span.to(end),
        }))
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

    /// The postfix chain hanging off an already-parsed expression: `.name`,
    /// `?.name` and `(args)`, in any order and any number of times.
    ///
    /// `Direction.North` and `user.name` are the same shape, and telling them
    /// apart means knowing whether `Direction` is a type or a value — which is
    /// resolution, not parsing. Both produce a field access and the checker
    /// decides which one it is. The same goes for `u.greeting()`: it is a call
    /// whose callee happens to be an access.
    fn parse_member_chain(&mut self, mut object: Expr) -> Option<Expr> {
        loop {
            match self.peek() {
                TokenKind::Dot | TokenKind::QuestionDot => {
                    let safe = matches!(self.peek(), TokenKind::QuestionDot);
                    self.pos += 1;

                    // `TraitName.super.method()` — the `super` itself is a
                    // keyword, and the leading trait name is the already-parsed
                    // primary it follows.
                    if let TokenKind::Keyword(Keyword::Super) = self.peek() {
                        let super_span = self.peek_span();
                        if let Expr::Path(ref ident) = object {
                            let trait_name = ident.clone();
                            self.pos += 1;
                            object = Expr::Super(SuperExpr {
                                trait_name: Some(trait_name),
                                span: object.span().to(super_span),
                            });
                            continue;
                        }
                    }

                    // `Pointer.from(place)` (roadmap Phase 4e, `ADR-015`):
                    // `from` is `Keyword::From` everywhere else (`implements
                    // X from Y`), but unambiguously a field name right after
                    // `.`/`?.` — nothing else can follow the access operator.
                    // The same applies to the universal `.type` member.
                    let name = if let TokenKind::Keyword(Keyword::From | Keyword::Type) = self.peek() {
                        let span = self.peek_span();
                        let text = match self.peek() {
                            TokenKind::Keyword(k) => k.as_str().to_string(),
                            _ => unreachable!("matched above"),
                        };
                        self.pos += 1;
                        Ident::new(text, span)
                    } else {
                        self.expect_identifier("after the access operator")?
                    };
                    object = Expr::Field(FieldExpr {
                        span: object.span().to(name.span),
                        object: Box::new(object),
                        name,
                        safe,
                    });
                }
                TokenKind::LParen => {
                    let args = self.parse_args()?;
                    let end = self.peek_span();
                    self.expect(&TokenKind::RParen, "to close the argument list");

                    object = Expr::Call(CallExpr {
                        span: object.span().to(end),
                        callee: Box::new(object),
                        args,
                    });
                }
                TokenKind::Keyword(Keyword::As) => {
                    self.pos += 1;
                    let optional = matches!(self.peek(), TokenKind::Question);
                    if optional {
                        self.pos += 1;
                    }
                    let target = self.parse_type_atom()?;
                    object = Expr::Cast(CastExpr {
                        span: object.span().to(target.span),
                        expr: Box::new(object),
                        target,
                        optional,
                    });
                }
                // `receiver[index]` (roadmap Phase 4e, `fase-4e-native-slice`,
                // design D5): a new postfix expression, same precedence tier
                // as `.field`/`(args)`, left-associative and chainable
                // (`a[i][j]`, `a.field[i]`). The grammar accepts any
                // receiver — only the checker restricts which receiver types
                // actually support it.
                TokenKind::LBracket => {
                    self.pos += 1;
                    // `receiver[start:end:step]` (roadmap Phase 7, `String`
                    // slicing): a `:` (or the `::` the lexer fuses two of
                    // into, as in `s[::2]`) before or after the first
                    // expression makes this a slice rather than an index.
                    // Every part is optional — `s[:]`, `s[::2]`, `s[1:]`.
                    if self.eat(&TokenKind::ColonColon) {
                        // `s[::step]` — both bounds omitted.
                        let step = if matches!(self.peek(), TokenKind::RBracket) {
                            None
                        } else {
                            Some(self.parse_expr()?)
                        };
                        let end = self.peek_span();
                        self.expect(&TokenKind::RBracket, "to close the slice expression");
                        object = Expr::Slice(SliceExpr {
                            span: object.span().to(end),
                            receiver: Box::new(object),
                            start: None,
                            end: None,
                            step: step.map(Box::new),
                        });
                        continue;
                    }
                    if self.eat(&TokenKind::Colon) {
                        let (end_expr, step) = self.parse_slice_tail()?;
                        let end = self.peek_span();
                        self.expect(&TokenKind::RBracket, "to close the slice expression");
                        object = Expr::Slice(SliceExpr {
                            span: object.span().to(end),
                            receiver: Box::new(object),
                            start: None,
                            end: end_expr.map(Box::new),
                            step: step.map(Box::new),
                        });
                        continue;
                    }
                    let first = self.parse_expr()?;
                    if self.eat(&TokenKind::ColonColon) {
                        // `s[start::step]` — the end is omitted.
                        let step = if matches!(self.peek(), TokenKind::RBracket) {
                            None
                        } else {
                            Some(self.parse_expr()?)
                        };
                        let end_span = self.peek_span();
                        self.expect(&TokenKind::RBracket, "to close the slice expression");
                        object = Expr::Slice(SliceExpr {
                            span: object.span().to(end_span),
                            receiver: Box::new(object),
                            start: Some(Box::new(first)),
                            end: None,
                            step: step.map(Box::new),
                        });
                        continue;
                    }
                    if self.eat(&TokenKind::Colon) {
                        let (end, step) = self.parse_slice_tail()?;
                        let end_span = self.peek_span();
                        self.expect(&TokenKind::RBracket, "to close the slice expression");
                        object = Expr::Slice(SliceExpr {
                            span: object.span().to(end_span),
                            receiver: Box::new(object),
                            start: Some(Box::new(first)),
                            end: end.map(Box::new),
                            step: step.map(Box::new),
                        });
                        continue;
                    }
                    let end = self.peek_span();
                    self.expect(&TokenKind::RBracket, "to close the index expression");

                    object = Expr::Index(IndexExpr {
                        span: object.span().to(end),
                        receiver: Box::new(object),
                        index: Box::new(first),
                    });
                }
                _ => return Some(object),
            }
        }
    }

    /// The `end` and `step` of a slice after the first `:` was consumed:
    /// `:end:step` with either part optional (`:end`, `:end:`, `::step`, `:`).
    fn parse_slice_tail(&mut self) -> Option<(Option<Expr>, Option<Expr>)> {
        let end = if matches!(self.peek(), TokenKind::Colon | TokenKind::RBracket) {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let step = if self.eat(&TokenKind::Colon) && !matches!(self.peek(), TokenKind::RBracket) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        Some((end, step))
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

/// The place an expression names, if it names one.
///
/// A safe access is not a place: `a?.b = 1` would have to mean something when
/// `a` is absent, and there is no answer that is not a silent no-op.
fn as_assignable(expr: &Expr) -> Option<AssignTarget> {
    match expr {
        Expr::Path(ident) => Some(AssignTarget::Name(ident.clone())),
        Expr::Field(field) if !field.safe => Some(AssignTarget::Field(field.clone())),
        Expr::Index(index) => Some(AssignTarget::Index(index.clone())),
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
        TokenKind::AmpEq => BinaryOp::BitAnd,
        TokenKind::PipeEq => BinaryOp::BitOr,
        TokenKind::CaretEq => BinaryOp::BitXor,
        TokenKind::ShlEq => BinaryOp::Shl,
        TokenKind::ShrEq => BinaryOp::Shr,
        _ => return None,
    })
}

/// The operator an increment or decrement stands for.
fn increment_op(kind: &TokenKind) -> Option<IncrementOp> {
    Some(match kind {
        TokenKind::PlusPlus => IncrementOp::Increment,
        TokenKind::MinusMinus => IncrementOp::Decrement,
        _ => return None,
    })
}

/// The literal `1` that `++` and `--` add or subtract.
fn one(span: Span) -> Expr {
    Expr::Int(IntLit { value: 1, span })
}

/// Desugars `base ** exponent` (and `base **= exponent`) into the method call
/// `base.pow(exponent)` (`exponentiation-operator`). Downstream stages then
/// see an ordinary `pow` call, so `Float` and `BinaryFloat` need no new code
/// and the integer surface only gains a `pow` method. `op_span` (the `**`) is
/// carried onto the synthetic member name so a diagnostic still points at the
/// operator.
fn pow_call(base: Expr, exponent: Expr, op_span: Span) -> Expr {
    let span = base.span().to(exponent.span());
    let arg_span = exponent.span();
    Expr::Call(CallExpr {
        callee: Box::new(Expr::Field(FieldExpr {
            object: Box::new(base),
            name: Ident {
                name: "pow".to_string(),
                span: op_span,
            },
            safe: false,
            span,
        })),
        args: vec![Arg {
            name: None,
            value: exponent,
            span: arg_span,
        }],
        span,
    })
}

/// Converts a duration literal's magnitude and unit into nanoseconds.
fn duration_nanos(text: &str, unit: DurationUnit) -> i64 {
    use DurationUnit::*;
    let multiplier: i64 = match unit {
        Nanoseconds => 1,
        Microseconds => 1_000,
        Milliseconds => 1_000_000,
        Seconds => 1_000_000_000,
        Minutes => 60 * 1_000_000_000,
        Hours => 60 * 60 * 1_000_000_000,
        Days => 24 * 60 * 60 * 1_000_000_000,
        Weeks => 7 * 24 * 60 * 60 * 1_000_000_000,
    };

    // Try exact integer parsing first; fall back to `f64` for fractional
    // values. A literal beyond `i64` nanoseconds saturates — the same ceiling
    // `Duration`'s own arithmetic keeps.
    if !text.contains(['.', 'e', 'E'].as_slice())
        && let Ok(value) = text.parse::<i128>()
    {
        return value
            .saturating_mul(multiplier as i128)
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    }
    let value = text.parse::<f64>().unwrap_or(0.0);
    (value * multiplier as f64) as i64
}

/// One parsed member of a class body, before it is filed by kind.
///
/// It exists only inside the parser: the tree keeps the three kinds in
/// separate lists, because everything downstream looks up fields,
/// constructors and methods separately and never in declaration order.
enum ClassMember {
    Field(FieldDecl),
    Construct(ConstructDecl),
    Method(MethodDecl),
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
        // Same level as `==`: they answer neighbouring questions about the
        // same two values.
        T::Keyword(Keyword::Is) => (Is, 3),
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
        // Bitwise between comparison and additive, shift tighter than the
        // other three — the same relative order C, Rust and Swift share, so
        // `a & b == c` reads as `a & (b == c)` the way it does everywhere
        // else, not as `(a & b) == c`.
        T::Pipe => (BitOr, 6),
        T::Caret => (BitXor, 7),
        T::Amp => (BitAnd, 8),
        T::Shl => (Shl, 9),
        T::Shr => (Shr, 9),
        T::Plus => (Add, 10),
        T::Minus => (Sub, 10),
        T::Star => (Mul, 11),
        T::Slash => (Div, 11),
        T::Percent => (Rem, 11),
        _ => return None,
    })
}
