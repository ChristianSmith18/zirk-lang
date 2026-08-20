//! Recursive-descent parser with precedence climbing.
//!
//! Precedence is implemented with a loop over levels rather than one function
//! per level: the `precedence()` table is the readable definition of what
//! `ZIRK_LANGUAGE_SPEC.md` section 4 fixes, and adding an operator means adding
//! a row rather than a function.

use crate::codes;
use zirk_ast::*;
use zirk_diagnostics::{Code, Diagnostic, DiagnosticSink, Phase, SourceFile, Span};
use zirk_lexer::{Keyword, StrPart, Token, TokenKind, tokenize};

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
const MAX_NESTING: u32 = 128;

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

    /// Reports a literal the language has and this phase does not implement.
    ///
    /// Literals cannot go through [`Self::report_if_from_another_phase`]: that
    /// one names the token by its symbol, and a literal's symbol is its
    /// content, which would put the user's own text where the construct's name
    /// belongs.
    fn pending_literal(&mut self, span: Span, what: &str, phase: Phase) -> Option<Expr> {
        self.error(
            codes::NOT_IMPLEMENTED,
            span,
            format!("{what} is not implemented yet"),
            format!("it exists in the language but arrives in Phase {phase}"),
            Some("see docs/init/ZIRK_ROADMAP.md for the scope of each phase".into()),
        );
        self.pos += 1;
        // The statement is abandoned whole. Without this, the `;` left behind
        // produces a second "expected an expression" that matches no mistake
        // the user made.
        self.synchronize();
        None
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

            // `value` is contextual: only a `class` right after it makes this
            // a value class instead of an identifier starting an expression,
            // which cannot appear at the top level anyway.
            if matches!(self.peek(), TokenKind::Identifier(name) if name == "value")
                && self.tokens.get(self.pos + 1).map(|t| &t.kind)
                    == Some(&TokenKind::Keyword(Keyword::Class))
            {
                if let Some(c) = self.parse_value_class(shared_at.is_some()) {
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
            ClassKind::ValueClass => unreachable!("a value class has its own compact grammar"),
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

    /// `value class Name(field: Type, field: Type, ...);`
    ///
    /// A record's semantics compressed into one declaration: the parenthesized
    /// list becomes `fields`, exactly as if each had been written
    /// `field: Type;` in a `record` body. `value` is contextual — recognized
    /// only in front of `class`, the same way `strict` is only meaningful
    /// after `inmut::`.
    fn parse_value_class(&mut self, shared: bool) -> Option<ClassDecl> {
        let start = self.peek_span();
        self.pos += 1; // `value`
        self.eat_keyword(Keyword::Class);

        let name = self.expect_identifier("after `value class`")?;
        self.expect(&TokenKind::LParen, "after the value class name");

        let mut fields = Vec::new();
        if !matches!(self.peek(), TokenKind::RParen) {
            loop {
                let field_start = self.peek_span();
                let field_name = self.expect_identifier("as a field name")?;
                self.expect(&TokenKind::Colon, "after the field name");
                let ty = self.parse_type()?;
                let field_end = ty.span;
                fields.push(FieldDecl {
                    name: field_name,
                    ty,
                    visibility: Visibility::Public,
                    mutability: Mutability::Immutable,
                    explicit_modifiers: false,
                    span: field_start.to(field_end),
                });
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RParen, "to close the value class's fields");
        let end = self.peek_span();
        self.expect(&TokenKind::Semicolon, "after a value class declaration");

        Some(ClassDecl {
            name,
            kind: ClassKind::ValueClass,
            type_params: Vec::new(),
            implements: Vec::new(),
            extends: None,
            fields,
            constructors: Vec::new(),
            methods: Vec::new(),
            shared,
            span: start.to(end),
        })
    }

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
                    start,
                )
                .map(ClassMember::Method);
        }

        self.parse_field(visibility, start).map(ClassMember::Field)
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

    /// A field: `[visibility] [mut|inmut] name: Type;`
    ///
    /// Writing neither modifier means `public mut`
    /// (`ZIRK_LANGUAGE_SPEC.md` section 7). Both spellings produce the same
    /// member; only the flag remembers which was written.
    fn parse_field(&mut self, visibility: Option<Visibility>, start: Span) -> Option<FieldDecl> {
        let mutability = match self.peek() {
            TokenKind::Keyword(Keyword::Mut) => {
                self.pos += 1;
                Some(Mutability::Mutable)
            }
            TokenKind::Keyword(Keyword::Inmut) => {
                self.pos += 1;
                Some(Mutability::Immutable)
            }
            _ => None,
        };

        let name = self.expect_identifier("as the name of a field")?;
        self.expect(&TokenKind::Colon, "after the field name");
        let ty = self.parse_type()?;
        let end = self.peek_span();
        self.eat(&TokenKind::Semicolon);

        Some(FieldDecl {
            name,
            ty,
            visibility: visibility.unwrap_or(Visibility::Public),
            mutability: mutability.unwrap_or(Mutability::Mutable),
            explicit_modifiers: visibility.is_some() || mutability.is_some(),
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

    fn parse_fn(&mut self, shared: bool) -> Option<FnDecl> {
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
        if let TokenKind::Identifier(name) = self.peek().clone() {
            self.pos += 1;

            // `Fn(P...) => R` / `Function(P...) => R`: recognized so the
            // diagnostic can name the construct instead of reporting whatever
            // token `(` happens to confuse next (decision D9 of the design —
            // this phase's parser and checker reject the annotation on
            // purpose, closure values elsewhere are unaffected).
            if (name == "Fn" || name == "Function") && matches!(self.peek(), TokenKind::LParen) {
                return self.reject_function_type(name, span);
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
            return Some(ty);
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

    /// `Fn(P...) => R` or `Function(P...) => R` in type position, rejected on
    /// purpose per decision D9 — consumes the whole shape for recovery, then
    /// reports it by name rather than leaving `(` to confuse whatever parses
    /// next.
    fn reject_function_type(&mut self, name: String, start: Span) -> Option<TypeRef> {
        self.pos += 1; // `(`
        let mut depth = 1u32;
        while depth > 0 && !self.at_eof() {
            match self.peek() {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => depth -= 1,
                _ => {}
            }
            self.pos += 1;
        }

        let mut end = self.peek_span();
        if self.eat(&TokenKind::FatArrow)
            && let Some(returns) = self.parse_type_atom()
        {
            end = returns.span;
        }

        self.error(
            codes::NOT_IMPLEMENTED,
            start.to(end),
            format!("`{name}(...)` is not implemented yet"),
            "function types exist in the language, but this phase's parser and checker reject the annotation on purpose (decision D9)",
            Some(
                "let a closure's type be inferred instead of annotating it: assign it to a local without a type annotation"
                    .into(),
            ),
        );
        None
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

            let end = constraints.last().map(|c| c.span).unwrap_or(name.span);
            params.push(TypeParam {
                name,
                variance,
                constraints,
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

        let scrutinee = self.parse_range()?;

        // `match scrutinee with binding { ... }` (roadmap Phase 4c) scopes a
        // `Resource<E>`: `binding` names whichever arm's pattern acquires
        // one, closed automatically on every exit from that arm.
        let with_binding = if self.eat_keyword(Keyword::With) {
            Some(self.expect_identifier("after `with`")?)
        } else {
            None
        };

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
            with_binding,
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
            TokenKind::Duration(_, _) => {
                self.pending_literal(span, "a duration literal", Phase::SEVEN)
            }
            TokenKind::Regex(_) => self.pending_literal(span, "a regex literal", Phase::SEVEN),
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
                Some(Expr::Super(SuperExpr { span }))
            }
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

                    let name = self.expect_identifier("after the access operator")?;
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
                    let target = self.parse_type_atom()?;
                    object = Expr::Cast(CastExpr {
                        span: object.span().to(target.span),
                        expr: Box::new(object),
                        target,
                    });
                }
                _ => return Some(object),
            }
        }
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
