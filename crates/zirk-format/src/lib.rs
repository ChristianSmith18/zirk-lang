//! # zirk-format
//!
//! **Responsibility:** canonical, deterministic Zirk document formatting.
//! **Boundary:** this crate is pure text-in/text-out: it performs no file,
//! network, clock, process, or other host I/O.

#![forbid(unsafe_code)]

use std::ops::Range;
use zirk_ast::{
    Block, ClassDecl, ContractDecl, EnumDecl, Expr, FieldDecl, MethodDecl, Program, Stmt,
};
use zirk_diagnostics::{DiagnosticSink, SourceFile};
use zirk_lexer::TokenKind;

/// The canonical maximum line width.
pub const LINE_WIDTH: usize = 100;
/// The canonical indentation width.
pub const INDENT_WIDTH: usize = 4;

/// Formatting output together with non-fatal directive diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub source: String,
    pub warnings: Vec<String>,
}

/// A small, public pretty-printing document algebra. A group is rendered
/// flat only when all its soft lines fit; otherwise every line breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc {
    Text(String),
    Line,
    SoftLine,
    Group(Vec<Doc>),
    Indent(Vec<Doc>),
}
impl Doc {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }
    pub fn line() -> Self {
        Self::Line
    }
    pub fn softline() -> Self {
        Self::SoftLine
    }
    pub fn group(parts: Vec<Self>) -> Self {
        Self::Group(parts)
    }
    pub fn indent(parts: Vec<Self>) -> Self {
        Self::Indent(parts)
    }
    pub fn join(parts: impl IntoIterator<Item = Self>, separator: Self) -> Vec<Self> {
        let mut out = Vec::new();
        for item in parts {
            if !out.is_empty() {
                out.push(separator.clone());
            }
            out.push(item);
        }
        out
    }
    pub fn render(&self) -> String {
        render(self, 0, false)
    }
}
fn flat_len(doc: &Doc) -> usize {
    match doc {
        Doc::Text(s) => s.len(),
        Doc::Line => LINE_WIDTH + 1,
        Doc::SoftLine => 1,
        Doc::Group(xs) | Doc::Indent(xs) => xs.iter().map(flat_len).sum(),
    }
}
fn render(doc: &Doc, indent: usize, broken: bool) -> String {
    match doc {
        Doc::Text(s) => s.clone(),
        Doc::Line => format!("\n{}", " ".repeat(indent)),
        Doc::SoftLine => {
            if broken {
                format!("\n{}", " ".repeat(indent))
            } else {
                " ".into()
            }
        }
        Doc::Group(xs) => {
            let b = broken || flat_len(doc) > LINE_WIDTH;
            xs.iter().map(|x| render(x, indent, b)).collect()
        }
        Doc::Indent(xs) => xs
            .iter()
            .map(|x| render(x, indent + INDENT_WIDTH, broken))
            .collect(),
    }
}

/// Validates and formats source. Invalid source returns rendered diagnostics;
/// no caller receives a partial output that could be written accidentally.
pub fn format(name: &str, text: &str) -> Result<String, String> {
    format_with_warnings(name, text).map(|result| result.source)
}

/// Formats source and reports malformed formatter directives without turning
/// them into a syntax failure. Disabled sections are conservatively retained
/// byte-for-byte; this prevents a style tool from changing opt-out content.
pub fn format_with_warnings(name: &str, text: &str) -> Result<FormatResult, String> {
    let source = SourceFile::new(name.to_owned(), text.to_owned());
    let mut sink = DiagnosticSink::new();
    let tokens = zirk_lexer::tokenize(&source, &mut sink);
    let program = zirk_parser::parse(&source, &tokens, &mut sink);
    if sink.has_errors() {
        return Err(render_diagnostics(&sink));
    }
    let (_, comments) = zirk_lexer::tokenize_with_trivia(&source, &mut sink);
    let mut disabled_start = None;
    let mut disabled_ranges = Vec::new();
    let mut warnings = Vec::new();
    for comment in comments {
        match comment.text.trim() {
            "// zirk-compiler: fmt off" if disabled_start.is_none() => {
                disabled_start = Some(comment.span.start as usize);
            }
            "// zirk-compiler: fmt on" => match disabled_start.take() {
                Some(start) => disabled_ranges.push(start..comment.span.end as usize),
                None => {
                    warnings.push("formatter directive `fmt on` has no preceding `fmt off`".into())
                }
            },
            "// zirk-compiler: fmt off" => {}
            _ => {}
        }
    }
    if let Some(start) = disabled_start {
        disabled_ranges.push(start..text.len());
    }
    let declarations = format_declaration_headers(text, &program);
    let source = if disabled_ranges.is_empty() {
        let declarations = normalize_comment_attachment(&declarations);
        let staged = SourceFile::new(name.to_owned(), declarations.clone());
        let mut staged_sink = DiagnosticSink::new();
        let staged_tokens = zirk_lexer::tokenize(&staged, &mut staged_sink);
        let canonical = format_text(&declarations, &staged_tokens);
        let canonical_source = SourceFile::new(name.to_owned(), canonical.clone());
        let mut canonical_sink = DiagnosticSink::new();
        let canonical_tokens = zirk_lexer::tokenize(&canonical_source, &mut canonical_sink);
        let spaced = normalize_token_spacing(&canonical, &canonical_tokens);
        let broken = break_long_delimited_groups(&spaced);
        normalize_indentation(&normalize_comment_attachment(&broken))
    } else {
        normalize_outside_disabled(&declarations, &disabled_ranges)
    };
    Ok(FormatResult { source, warnings })
}

/// Canonical block indentation is determined from braces in source text, not
/// from expression spans. This deliberately leaves expression tokens alone
/// while giving every statement, control-flow body, and nested block a stable
/// four-space base indentation. Lines inside an unclosed `(` or `[` are
/// continuations and sit one level deeper than their statement; `}` depth
/// alone does not create continuations, so enum variants and match arms
/// separated by commas keep their own indentation.
fn normalize_indentation(source: &str) -> String {
    let had_newline = source.ends_with('\n');
    let mut depth = 0usize;
    // Stack of unclosed delimiters. A `(`/`[` on top makes the next line a
    // continuation (one level deeper); a `{` on top does not, so enum
    // variants, record fields, and match arms keep their own level even
    // though their neighbours end in commas.
    let mut stack: Vec<u8> = Vec::new();
    let mut triple_quoted = false;
    let mut output = Vec::new();
    for line in source.lines() {
        if triple_quoted {
            output.push(line.to_owned());
            if line.contains("'''") {
                triple_quoted = false;
            }
            continue;
        }
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.is_empty() {
            output.push(String::new());
            continue;
        }
        let closing_brace = !triple_quoted && trimmed.starts_with('}');
        let closing_delimiter =
            !triple_quoted && matches!(trimmed.as_bytes().first(), Some(b'}' | b')' | b']'));
        let triple_continuation = trimmed.starts_with("'''")
            && output
                .last()
                .is_some_and(|previous| previous.trim_end().ends_with('='));
        let continuation = matches!(stack.last(), Some(b'(' | b'['));
        let indentation = if (continuation || triple_continuation) && !closing_delimiter {
            " ".repeat((depth + 1) * INDENT_WIDTH)
        } else {
            " ".repeat(depth.saturating_sub(usize::from(closing_brace)) * INDENT_WIDTH)
        };
        output.push(format!("{indentation}{trimmed}"));
        let deltas = delimiter_deltas(trimmed, triple_quoted, &mut stack);
        if deltas.toggles_triple {
            triple_quoted = !triple_quoted;
        }
        if !triple_quoted {
            depth = depth
                .saturating_add(deltas.brace_opens)
                .saturating_sub(deltas.brace_closes);
        }
    }
    let mut result = output.join("\n");
    if had_newline {
        result.push('\n');
    }
    result
}

struct Deltas {
    brace_opens: usize,
    brace_closes: usize,
    toggles_triple: bool,
}

fn delimiter_deltas(line: &str, initially_triple_quoted: bool, stack: &mut Vec<u8>) -> Deltas {
    let mut deltas = Deltas {
        brace_opens: 0,
        brace_closes: 0,
        toggles_triple: false,
    };
    if initially_triple_quoted {
        deltas.toggles_triple = line.contains("'''");
        return deltas;
    }
    let bytes = line.as_bytes();
    let mut quoted = None;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"'''") {
            deltas.toggles_triple = true;
            return deltas;
        }
        let byte = bytes[index];
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
        } else if matches!(byte, b'\'' | b'\"') {
            quoted = Some(byte);
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'/') {
            break;
        } else {
            match byte {
                b'{' => {
                    deltas.brace_opens += 1;
                    stack.push(b'{');
                }
                b'}' => {
                    deltas.brace_closes += 1;
                    if stack.last() == Some(&b'{') {
                        stack.pop();
                    }
                }
                b'(' | b'[' => stack.push(byte),
                b')' if stack.last() == Some(&b'(') => {
                    stack.pop();
                }
                b']' if stack.last() == Some(&b'[') => {
                    stack.pop();
                }
                b')' | b']' => {}
                _ => {}
            }
        }
        index += 1;
    }
    deltas
}

/// Re-anchor comments by line position without touching their content. A
/// standalone comment follows the indentation of the next construct; a
/// trailing comment is separated from its code by exactly one space.
fn normalize_comment_attachment(source: &str) -> String {
    let had_newline = source.ends_with('\n');
    let lines = source.lines().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        let Some(comment_start) = line_comment_start(line) else {
            output.push((*line).to_owned());
            continue;
        };
        let (code, comment) = line.split_at(comment_start);
        if code.trim().is_empty() {
            let next = lines[index + 1..].iter().find(|candidate| {
                !candidate.trim().is_empty() && line_comment_start(candidate).is_none()
            });
            let indent = next.map_or_else(
                || leading_indent(line),
                |next| {
                    if next.trim_start().starts_with('}') {
                        output
                            .last()
                            .filter(|previous: &&String| previous.trim_end().ends_with('{'))
                            .map(|previous| {
                                format!("{}{}", leading_indent(previous), " ".repeat(INDENT_WIDTH))
                            })
                            .unwrap_or_else(|| leading_indent(next))
                    } else {
                        leading_indent(next)
                    }
                },
            );
            output.push(format!("{indent}{}", comment.trim_start()));
        } else {
            output.push(format!("{} {}", code.trim_end(), comment.trim_start()));
        }
    }
    let mut result = output.join("\n");
    if had_newline {
        result.push('\n');
    }
    result
}

/// Normalize spaces between adjacent lexical tokens while retaining the
/// author's line structure.  This is deliberately token based rather than a
/// string replacement: operators in strings, regexes and interpolations are
/// opaque lexer tokens and are therefore never confused with source syntax.
fn normalize_token_spacing(source: &str, tokens: &[zirk_lexer::Token]) -> String {
    let tokens = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Eof))
        .collect::<Vec<_>>();
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    // Tracks a `?` whose matching `:` is still ahead — that colon is the
    // ternary separator and takes spaces on both sides, unlike a `:` type
    // annotation or the optional marker `?:`. Boundaries and a consumed
    // colon reset it.
    let mut pending_ternary = false;
    for (index, pair) in tokens.windows(2).enumerate() {
        let previous = pair[0];
        let next = pair[1];
        let before_previous = index.checked_sub(1).map(|index| &tokens[index].kind);
        let previous_end = previous.span.end as usize;
        let next_start = next.span.start as usize;
        output.push_str(&source[cursor..previous_end]);
        let gap = &source[previous_end..next_start];
        if gap.contains('\n') {
            output.push_str(gap);
        } else if gap.contains("//") || gap.contains("/*") {
            output.push_str(gap);
        } else {
            // A `?` is ternary when an operand follows it; otherwise it is a
            // suffix (`name?:`, `T?`) that binds to the preceding token.
            let after_next = tokens.get(index + 2).map(|token| &token.kind);
            let replacement = match (&previous.kind, &next.kind) {
                (_, TokenKind::Question) if after_next.is_some_and(is_operand_start) => " ",
                (_, TokenKind::Question) => "",
                (TokenKind::Question, TokenKind::Colon) => "",
                (_, TokenKind::Colon) if pending_ternary => " ",
                (
                    TokenKind::Identifier(_) | TokenKind::RParen | TokenKind::RBracket,
                    TokenKind::LBracket,
                ) => "",
                _ => token_gap(&previous.kind, &next.kind, before_previous, gap),
            };
            output.push_str(replacement);
        }
        match previous.kind {
            TokenKind::Question if is_operand_start(&next.kind) => {
                pending_ternary = true
            }
            TokenKind::Colon
            | TokenKind::Comma
            | TokenKind::Semicolon
            | TokenKind::LParen
            | TokenKind::RParen
            | TokenKind::LBrace
            | TokenKind::RBrace
            | TokenKind::LBracket
            | TokenKind::RBracket
            | TokenKind::Assign
            | TokenKind::Arrow
            | TokenKind::FatArrow => pending_ternary = false,
            _ => {}
        }
        cursor = next_start;
    }
    if let Some(last) = tokens.last() {
        if cursor == 0 {
            return source.to_owned();
        }
        output.push_str(&source[cursor..last.span.end as usize]);
        output.push_str(&source[last.span.end as usize..]);
    } else {
        output.push_str(source);
    }
    output
}

fn token_gap(
    previous: &TokenKind,
    next: &TokenKind,
    before_previous: Option<&TokenKind>,
    original_gap: &str,
) -> &'static str {
    use TokenKind::*;
    if matches!(
        next,
        Comma | Semicolon | RParen | RBracket | Dot | QuestionDot | ColonColon
    ) || matches!(
        previous,
        LParen | LBracket | Dot | QuestionDot | ColonColon | Hash
    ) {
        return "";
    }
    if matches!(next, LParen) {
        return match previous {
            Keyword(k)
                if matches!(
                    k,
                    zirk_lexer::Keyword::This
                        | zirk_lexer::Keyword::Super
                        | zirk_lexer::Keyword::Construct
                        | zirk_lexer::Keyword::Fn
                ) =>
            {
                ""
            }
            Keyword(_) => " ",
            _ => "",
        };
    }
    // Generic type argument delimiters are already adjacent in canonical
    // source. Preserve that distinction from comparison operators, which are
    // separated by whitespace and are handled below.
    if original_gap.is_empty() && matches!(previous, Lt | Gt | Shr)
        || original_gap.is_empty() && matches!(next, Lt | Gt)
    {
        return "";
    }
    if matches!(previous, Comma) {
        return " ";
    }
    if matches!(previous, Colon) {
        return " ";
    }
    if matches!(next, Colon) {
        return "";
    }
    if matches!(previous, LBrace) || matches!(next, RBrace) {
        return " ";
    }
    if is_binary_operator(previous) || is_binary_operator(next) {
        if matches!(previous, Minus | Plus | Not | Tilde)
            && before_previous.is_none_or(|before| {
                matches!(
                    before,
                    Assign
                        | LParen
                        | LBracket
                        | Comma
                        | Colon
                        | FatArrow
                        | Arrow
                        | Keyword(zirk_lexer::Keyword::Return)
                )
            })
            && matches!(
                next,
                Identifier(_) | Keyword(_) | Integer(_) | Float(_) | Duration(_, _)
            )
        {
            return "";
        }
        return " ";
    }
    if matches!(previous, Question) {
        return " ";
    }
    if matches!(previous, FatArrow | Arrow) || matches!(next, FatArrow | Arrow) {
        return " ";
    }
    // Adjacent words/literals would otherwise merge into a different token.
    " "
}

/// Whether a token can begin an operand — used to tell the ternary `?`
/// (`cond ? a : b`) from the suffix `?` (`name?:`, `T?`).
fn is_operand_start(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier(_)
            | TokenKind::Keyword(_)
            | TokenKind::Integer(_)
            | TokenKind::Float(_)
            | TokenKind::Duration(..)
            | TokenKind::Str(_)
            | TokenKind::InterpolatedStr(_)
            | TokenKind::Char(_)
            | TokenKind::Regex(_)
            | TokenKind::LParen
            | TokenKind::LBracket
            | TokenKind::Minus
            | TokenKind::Plus
            | TokenKind::Not
            | TokenKind::Tilde
            | TokenKind::DotDotDot
    )
}

fn is_binary_operator(token: &TokenKind) -> bool {
    matches!(
        token,
        TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Assign
            | TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::Gt
            | TokenKind::GtEq
            | TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::QuestionQuestion
            | TokenKind::Amp
            | TokenKind::Pipe
            | TokenKind::Caret
            | TokenKind::Shl
            | TokenKind::Shr
            | TokenKind::PipeGt
            | TokenKind::StarStar
    )
}

/// Apply the canonical all-or-nothing list rule to parenthesized and bracketed
/// expressions that still fit on one physical source line.  Header and call
/// formatting handles the common declaration/call forms; this pass covers
/// collection and nested expression lists without reconstructing their AST.
fn break_long_delimited_groups(source: &str) -> String {
    let file = SourceFile::new("<formatter>".to_owned(), source.to_owned());
    let mut sink = DiagnosticSink::new();
    let tokens = zirk_lexer::tokenize(&file, &mut sink);
    if sink.has_errors() {
        return source.to_owned();
    }
    let tokens = tokens
        .iter()
        .filter(|token| !matches!(token.kind, TokenKind::Eof))
        .collect::<Vec<_>>();
    let mut stack = Vec::new();
    let mut replacements = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LParen | TokenKind::LBracket => stack.push(index),
            TokenKind::RParen | TokenKind::RBracket => {
                let Some(open_index) = stack.pop() else {
                    continue;
                };
                let open = tokens[open_index];
                let compatible = matches!(
                    (&open.kind, &token.kind),
                    (TokenKind::LParen, TokenKind::RParen)
                        | (TokenKind::LBracket, TokenKind::RBracket)
                );
                if !compatible {
                    continue;
                }
                let start = open.span.start as usize;
                let end = token.span.end as usize;
                let line_start = source[..start].rfind('\n').map_or(0, |offset| offset + 1);
                if source[start..end].contains('\n')
                    || source[line_start..end].chars().count() <= LINE_WIDTH
                {
                    continue;
                }
                let mut depth = 0usize;
                let mut element_start = open.span.end as usize;
                let mut elements = Vec::new();
                for inner in &tokens[open_index + 1..index] {
                    match inner.kind {
                        TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
                        TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                            depth = depth.saturating_sub(1)
                        }
                        TokenKind::Comma if depth == 0 => {
                            elements.push(source[element_start..inner.span.start as usize].trim());
                            element_start = inner.span.end as usize;
                        }
                        _ => {}
                    }
                }
                if elements.is_empty() {
                    continue;
                }
                elements.push(source[element_start..token.span.start as usize].trim());
                if elements.iter().any(|element| element.contains("//")) {
                    continue;
                }
                let indent = leading_indent(&source[line_start..start]);
                let inner = format!("{indent}{}", " ".repeat(INDENT_WIDTH));
                replacements.push(Replacement {
                    start,
                    end,
                    text: format!(
                        "{}\n{}\n{indent}{}",
                        &source[start..open.span.end as usize],
                        elements
                            .iter()
                            .map(|element| format!("{inner}{element},"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        &source[token.span.start as usize..end]
                    ),
                });
            }
            _ => {}
        }
    }
    replacements.sort_by_key(|replacement| replacement.start);
    let mut output = source.to_owned();
    // An outer group subsumes an inner one, so retain only non-overlapping
    // replacements and apply them from the end of the file.
    let mut covered_start = usize::MAX;
    for replacement in replacements.into_iter().rev() {
        if replacement.end <= covered_start {
            covered_start = replacement.start;
            output.replace_range(replacement.start..replacement.end, &replacement.text);
        }
    }
    output
}

fn leading_indent(line: &str) -> String {
    line.chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .map(|character| if character == '\t' { "    " } else { " " })
        .collect()
}

fn line_comment_start(line: &str) -> Option<usize> {
    let mut quoted = None;
    let mut escaped = false;
    let bytes = line.as_bytes();
    let mut index = 0;
    while index + 1 < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                quoted = None;
            }
        } else if matches!(byte, b'\'' | b'\"') {
            quoted = Some(byte);
        } else if byte == b'/' && bytes[index + 1] == b'/' {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[derive(Debug)]
struct Replacement {
    start: usize,
    end: usize,
    text: String,
}

/// Formats syntactic headers without serializing the whole tree. Every range
/// is measured against `source`; applying the complete set back-to-front
/// makes independent AST replacements safe even when earlier replacements
/// change byte lengths.
fn format_declaration_headers(source: &str, program: &Program) -> String {
    let mut replacements = Vec::new();
    for function in &program.functions {
        replacements.push(Replacement {
            start: function.span.start as usize,
            end: function.body.span.start as usize,
            text: callable_header(
                source,
                function.span.start as usize,
                function.name.span.end as usize,
                function.body.span.start as usize,
                &function.params,
            ),
        });
        collect_long_call_replacements(source, &function.body, &mut replacements);
    }
    for class in &program.classes {
        collect_class_header_replacements(source, class, &mut replacements);
    }
    for contract in &program.contracts {
        collect_contract_header_replacements(source, contract, &mut replacements);
    }
    for enumeration in &program.enums {
        collect_enum_header_replacements(source, enumeration, &mut replacements);
    }
    replacements.sort_by_key(|replacement| replacement.start);
    let mut output = source.to_owned();
    for replacement in replacements.into_iter().rev() {
        output.replace_range(replacement.start..replacement.end, &replacement.text);
    }
    output
}

fn collect_class_header_replacements(
    source: &str,
    class: &ClassDecl,
    replacements: &mut Vec<Replacement>,
) {
    collect_braced_header_replacement(source, class.span.start as usize, replacements);
    for field in &class.fields {
        replacements.push(format_field_replacement(source, field));
    }
    for construct in &class.constructors {
        replacements.push(Replacement {
            start: construct.span.start as usize,
            end: construct.body.span.start as usize,
            text: callable_header(
                source,
                construct.span.start as usize,
                construct.span.start as usize + "construct".len(),
                construct.body.span.start as usize,
                &construct.params,
            ),
        });
        collect_long_call_replacements(source, &construct.body, replacements);
    }
    for method in &class.methods {
        collect_method_header_replacement(source, method, replacements);
    }
    for nested in &class.nested {
        collect_class_header_replacements(source, nested, replacements);
    }
}

fn collect_contract_header_replacements(
    source: &str,
    contract: &ContractDecl,
    replacements: &mut Vec<Replacement>,
) {
    collect_braced_header_replacement(source, contract.span.start as usize, replacements);
    for method in &contract.methods {
        collect_method_header_replacement(source, method, replacements);
    }
}

fn collect_enum_header_replacements(
    source: &str,
    enumeration: &EnumDecl,
    replacements: &mut Vec<Replacement>,
) {
    collect_braced_header_replacement(source, enumeration.span.start as usize, replacements);
}

fn collect_braced_header_replacement(
    source: &str,
    start: usize,
    replacements: &mut Vec<Replacement>,
) {
    let brace = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .expect("parsed braced declaration has an opening brace");
    replacements.push(Replacement {
        start,
        end: brace,
        text: format!("{} ", normalize_header(&source[start..brace])),
    });
}

/// Calls are replaced only when the call node itself exceeds the width. This
/// avoids treating a parenthesized subexpression as though its AST span still
/// included the parentheses, while retaining all-or-nothing argument breaks.
fn collect_long_call_replacements(
    source: &str,
    block: &Block,
    replacements: &mut Vec<Replacement>,
) {
    for statement in &block.statements {
        let candidate = match statement {
            Stmt::Let(statement) => statement.init.as_ref(),
            Stmt::Return(statement) => statement.value.as_ref(),
            Stmt::Throw(statement) => statement.value.as_ref(),
            Stmt::Expr(statement) => Some(&statement.expr),
            Stmt::Block(block) => {
                collect_long_call_replacements(source, block, replacements);
                None
            }
            _ => None,
        };
        if let Some(Expr::Call(call)) = candidate {
            let start = call.span.start as usize;
            let end = call.span.end as usize;
            let flat = source[start..end].trim();
            let line_start = source[..start].rfind('\n').map_or(0, |offset| offset + 1);
            if source[line_start..start].chars().count() + flat.chars().count() <= LINE_WIDTH {
                continue;
            }
            let line_indent = leading_indent(&source[line_start..start]);
            let callee =
                source[call.callee.span().start as usize..call.callee.span().end as usize].trim();
            let arguments = call
                .args
                .iter()
                .map(|argument| {
                    source[argument.span.start as usize..argument.span.end as usize].trim()
                })
                .collect::<Vec<_>>();
            let inner_indent = format!("{line_indent}{}", " ".repeat(INDENT_WIDTH));
            replacements.push(Replacement {
                start,
                end,
                text: format!(
                    "{callee}(\n{}\n{line_indent})",
                    arguments
                        .iter()
                        .map(|argument| format!("{inner_indent}{argument},"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            });
        }
    }
}

fn format_field_replacement(source: &str, field: &FieldDecl) -> Replacement {
    let start = field.span.start as usize;
    let end = field.span.end as usize;
    let prefix = collapse_inline_whitespace(&source[start..field.name.span.start as usize]);
    let prefix = if prefix.is_empty() {
        prefix
    } else {
        format!("{prefix} ")
    };
    // `TypeRef::span` covers only the type's leading name, so the type text is
    // sliced instead: everything between the `:` after the field name and the
    // `=` introducing the default (or the terminating `;`). Slicing also keeps
    // spellings `TypeRef` reserializes differently, like `T[n]` fixed arrays.
    let colon = source[field.name.span.end as usize..end]
        .find(':')
        .map(|offset| field.name.span.end as usize + offset + 1)
        .unwrap_or(end);
    let type_end = match &field.default {
        Some(default) => {
            let region = &source[colon..default.span().start as usize];
            region
                .rmatch_indices('=')
                .map(|(offset, _)| offset)
                .find(|offset| region.as_bytes().get(offset + 1) != Some(&b'>'))
                .map(|offset| colon + offset)
                .unwrap_or(colon)
        }
        None => colon
            + source[colon..end]
                .find(';')
                .unwrap_or(end - colon),
    };
    let ty = collapse_inline_whitespace(&source[colon..type_end]);
    let mut text = format!("{prefix}{}: {ty}", field.name.name);
    if let Some(default) = &field.default {
        text.push_str(" = ");
        text.push_str(source[default.span().start as usize..default.span().end as usize].trim());
    }
    if source[start..end].trim_end().ends_with(';') {
        text.push(';');
    }
    Replacement { start, end, text }
}

fn collect_method_header_replacement(
    source: &str,
    method: &MethodDecl,
    replacements: &mut Vec<Replacement>,
) {
    let Some(body) = &method.body else {
        let start = method.span.start as usize;
        let semicolon = source[start..]
            .find(';')
            .map(|offset| start + offset)
            .expect("parsed abstract method ends with a semicolon");
        replacements.push(Replacement {
            start,
            end: semicolon + 1,
            text: format!(
                "{};",
                callable_header(
                    source,
                    start,
                    method.name.span.end as usize,
                    semicolon,
                    &method.params
                )
                .trim_end()
            ),
        });
        return;
    };
    replacements.push(Replacement {
        start: method.span.start as usize,
        end: body.span.start as usize,
        text: callable_header(
            source,
            method.span.start as usize,
            method.name.span.end as usize,
            body.span.start as usize,
            &method.params,
        ),
    });
    collect_long_call_replacements(source, body, replacements);
}

fn callable_header(
    source: &str,
    start: usize,
    name_end: usize,
    body_start: usize,
    _parameters: &[zirk_ast::Param],
) -> String {
    let open = source[name_end..body_start]
        .find('(')
        .map(|offset| name_end + offset)
        .expect("parsed callable has an opening parenthesis");
    let close = matching_paren(source, open, body_start)
        .expect("parsed callable has a closing parenthesis");
    let prefix = collapse_inline_whitespace(&source[start..open]);
    let suffix = collapse_inline_whitespace(&source[close + 1..body_start]);
    let parameters = split_top_level_commas(&source[open + 1..close])
        .into_iter()
        .map(format_parameter_text)
        .collect::<Vec<_>>();
    let flat = format!("{prefix}({}){suffix} ", parameters.join(", "));
    if flat.len() <= LINE_WIDTH {
        flat
    } else {
        format!(
            "{prefix}(\n{}\n){suffix} ",
            parameters
                .iter()
                .map(|parameter| format!("{}{},", " ".repeat(INDENT_WIDTH), parameter))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn matching_paren(source: &str, open: usize, limit: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, character) in source[open..limit].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn collapse_inline_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_header(value: &str) -> String {
    collapse_inline_whitespace(value)
        .replace(',', ", ")
        .replace(",  ", ", ")
}
fn format_parameter_text(value: &str) -> String {
    let value = collapse_inline_whitespace(value);
    let Some((name, rest)) = value.split_once(':') else {
        return value;
    };
    format!("{}: {}", name.trim(), rest.trim())
}

fn split_top_level_commas(value: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut nesting = 0usize;
    for (offset, character) in value.char_indices() {
        match character {
            '<' | '(' | '[' | '{' => nesting += 1,
            '>' | ')' | ']' | '}' => nesting = nesting.saturating_sub(1),
            ',' if nesting == 0 => {
                pieces.push(value[start..offset].trim());
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    let last = value[start..].trim();
    if !last.is_empty() {
        pieces.push(last);
    }
    pieces
}
fn normalize_outside_disabled(text: &str, disabled: &[Range<usize>]) -> String {
    let mut output = String::new();
    let mut offset = 0;
    let mut previous_blank = false;
    for segment in text.split_inclusive('\n') {
        let end = offset + segment.len();
        let protected = disabled
            .iter()
            .any(|range| range.start < end && offset < range.end);
        if protected {
            output.push_str(segment);
            previous_blank = false;
        } else {
            let newline = segment.strip_suffix('\n').is_some();
            let line = segment.trim_end_matches('\n').replace('\t', "    ");
            let line = line.trim_end();
            if line.is_empty() {
                if !previous_blank {
                    if newline {
                        output.push('\n');
                    }
                    previous_blank = true;
                }
            } else {
                output.push_str(line);
                if newline {
                    output.push('\n');
                }
                previous_blank = false;
            }
        }
        offset = end;
    }
    output
}
fn render_diagnostics(sink: &DiagnosticSink) -> String {
    sink.diagnostics()
        .iter()
        .map(|d| format!("error[{}]: {}\n", d.code.as_str(), d.message))
        .collect()
}

/// Existing triple-string canonicalization, retained in the shared engine.
pub fn format_text(text: &str, tokens: &[zirk_lexer::Token]) -> String {
    let mut literals: Vec<_> = tokens
        .iter()
        .filter_map(|t| match t.kind {
            TokenKind::Str(_) | TokenKind::InterpolatedStr(_)
                if text[t.span.start as usize..].starts_with("'''") =>
            {
                Some((t.span.start as usize, t.span.end as usize))
            }
            _ => None,
        })
        .collect();
    literals.sort_unstable_by_key(|(start, _)| *start);
    let mut output = text.replace('\t', "    ");
    for (start, end) in literals.into_iter().rev() {
        let replacement = canonical_literal(&output, start, end);
        let line_start = output[..start].rfind('\n').map_or(0, |n| n + 1);
        let replace_start = if output[line_start..start].trim().is_empty() {
            line_start
        } else {
            output[line_start..start].trim_end().len() + line_start
        };
        output.replace_range(replace_start..end, &replacement);
    }
    let mut blank = false;
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim_end();
            if line.is_empty() {
                if blank {
                    None
                } else {
                    blank = true;
                    Some("")
                }
            } else {
                blank = false;
                Some(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if output.ends_with('\n') { "\n" } else { "" }
}
fn canonical_literal(text: &str, start: usize, end: usize) -> String {
    let line_start = text[..start].rfind('\n').map_or(0, |n| n + 1);
    let prefix = &text[line_start..start];
    let indent: String = text[line_start..]
        .chars()
        .take_while(|c| *c == ' ')
        .collect();
    let opening_is_own_line = prefix.trim().is_empty();
    let edge = if opening_is_own_line {
        indent
    } else {
        format!("{}{}", indent, " ".repeat(INDENT_WIDTH))
    };
    // The opening delimiter's column is the literal's left edge: every
    // content line keeps exactly the characters beyond it, since extra
    // indentation is string content, not layout.
    let old_edge = prefix.chars().count();
    let body = &text[start + 3..end - 3];
    let body = body
        .rsplit_once('\n')
        .map_or(body, |(v, _)| v)
        .trim_matches('\n');
    let content = body
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", edge, line.chars().skip(old_edge).collect::<String>())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let opening = if opening_is_own_line {
        edge.clone()
    } else {
        format!("\n{edge}")
    };
    if content.is_empty() {
        format!("{opening}'''\n{edge}'''")
    } else {
        format!("{opening}'''\n{content}\n{edge}'''")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn group_is_flat_when_it_fits() {
        assert_eq!(
            Doc::group(vec![Doc::text("a"), Doc::softline(), Doc::text("b")]).render(),
            "a b"
        );
    }
    #[test]
    fn group_breaks_all_soft_lines() {
        let part = "x".repeat(LINE_WIDTH);
        assert!(
            Doc::group(vec![
                Doc::text(part),
                Doc::softline(),
                Doc::text("b"),
                Doc::softline(),
                Doc::text("c")
            ])
            .render()
            .contains("\n")
        );
    }

    #[test]
    fn removes_trailing_whitespace_collapses_blank_lines_and_normalizes_tabs() {
        let source = "fn main(): Void {\t}\t  \n\n\n";
        let formatted = format("test.zrk", source).unwrap();
        assert_eq!(formatted, "fn main(): Void { }\n\n");
        assert!(!formatted.lines().any(|line| line.ends_with(' ')));
    }

    #[test]
    fn preserves_constructor_sugar_without_synthesizing_members() {
        let source = "class User { construct(public name: String) { } }\nclass Marker { }\nfn main(): Void { }\n";
        let formatted = format("oop.zrk", source).unwrap();
        assert_eq!(formatted, source);
        assert!(formatted.contains("construct(public name: String)"));
        assert!(formatted.contains("class Marker { }"));
    }

    #[test]
    fn directives_preserve_disabled_bytes_and_warn_for_orphan_on() {
        let source = "// zirk-compiler: fmt off\nfn main(): Void {\t}\n// zirk-compiler: fmt on\n";
        let result = format_with_warnings("directives.zrk", source).unwrap();
        assert_eq!(result.source, source);
        assert!(result.warnings.is_empty());

        let orphan = format_with_warnings(
            "directives.zrk",
            "// zirk-compiler: fmt on\nfn main(): Void { }\n",
        )
        .unwrap();
        assert_eq!(orphan.warnings.len(), 1);
    }

    #[test]
    fn directives_resume_formatting_outside_the_disabled_region() {
        let source = "fn main(): Void {\n// zirk-compiler: fmt off\n\tmut   x=1;\n// zirk-compiler: fmt on\n}\t\n\n\n";
        let result = format_with_warnings("directives.zrk", source).unwrap();
        assert!(result.source.starts_with("fn main(): Void {\n"));
        assert!(result.source.contains("\tmut   x=1;\n"));
        assert!(!result.source.ends_with("\n\n\n"));
    }

    #[test]
    fn unclosed_and_nested_off_directives_are_verbatim_without_nesting() {
        let unclosed = "fn main(): Void {\n// zirk-compiler: fmt off\n\tmut x=1;\n}\n";
        assert_eq!(
            format_with_warnings("directives.zrk", unclosed)
                .unwrap()
                .source,
            unclosed
        );
        let nested = "fn main(): Void {\n// zirk-compiler: fmt off\n// zirk-compiler: fmt off\n\tmut x=1;\n// zirk-compiler: fmt on\n}\n";
        assert_eq!(
            format_with_warnings("directives.zrk", nested)
                .unwrap()
                .source,
            nested
        );
    }

    #[test]
    fn valid_cli_corpus_is_idempotent_and_reparseable() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../zirk-cli/tests/corpus/valid");
        let mut entries = std::fs::read_dir(root)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "zrk"))
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let input = std::fs::read_to_string(&path).unwrap();
            let once = format(&path.display().to_string(), &input).unwrap();
            let twice = format(&path.display().to_string(), &once)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            assert_eq!(twice, once, "{}", path.display());
            assert_program_shape_is_stable(&path.display().to_string(), &input, &once);
        }
    }

    /// Every `.zrk` document the repository ships — the valid and invalid CLI
    /// corpus, the top-level examples, and the ````zirk` fences embedded in
    /// `docs/` — plus the curated playground sources in the companion site
    /// checkout. Parseable documents are formatted, re-formatted, and
    /// re-parsed; handbook fragments that only parse inside a synthetic
    /// `fn main` are verified under the same wrap the playground applies.
    /// Sources that cannot parse under either form are documentation
    /// fragments or pseudocode: they are reported, not formatted.
    #[test]
    fn corpus_docs_and_site_examples_are_idempotent_and_reparseable() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let mut documents = Vec::new();
        collect_zrk_files(&root.join("crates/zirk-cli/tests/corpus"), &mut documents);
        collect_zrk_files(&root.join("examples"), &mut documents);
        collect_markdown_fences(&root.join("docs"), &mut documents);
        collect_site_examples(&root, &mut documents);

        let mut verified = 0usize;
        let mut wrapped = 0usize;
        let mut fragments = Vec::new();
        let mut violations = Vec::new();
        for document in &documents {
            let name = &document.name;
            match check_document(name, &document.source) {
                DocumentCheck::Verified => verified += 1,
                DocumentCheck::Unparseable(error) => {
                    match wrap_top_level_statements(&document.source)
                        .map(|wrap| check_document(name, &wrap))
                    {
                        Some(DocumentCheck::Verified) => wrapped += 1,
                        Some(DocumentCheck::Violation(error)) => {
                            violations.push(format!("{name}: {error}"))
                        }
                        Some(DocumentCheck::Unparseable(_)) | None => {
                            // The first diagnostic line tells doc readers why a
                            // fence was skipped — pseudocode looks different
                            // from a snippet that simply stopped parsing.
                            let reason = error.lines().next().unwrap_or("does not parse");
                            fragments.push(format!("{name} — {reason}"));
                        }
                    }
                }
                DocumentCheck::Violation(error) => violations.push(format!("{name}: {error}")),
            }
        }
        eprintln!(
            "formatter harness: {verified} documents verified ({wrapped} via playground wrap),              {} fragments skipped",
            fragments.len()
        );
        for fragment in &fragments {
            eprintln!("  fragment: {fragment}");
        }
        assert!(verified > 0, "no documents were verified");
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }

    enum DocumentCheck {
        /// Formatted, re-formatted to identical bytes, and re-parsed to the
        /// same program shape.
        Verified,
        /// `format` reported diagnostics; the source may be a fragment.
        Unparseable(String),
        /// The source parses but the formatter broke idempotence or changed
        /// the program shape — a formatter bug, never skippable.
        Violation(String),
    }

    /// One document must format to a fixed point that re-parses into the
    /// same program shape.
    fn check_document(name: &str, input: &str) -> DocumentCheck {
        let once = match format(name, input) {
            Ok(once) => once,
            Err(error) => return DocumentCheck::Unparseable(error),
        };
        let twice = match format(name, &once) {
            Ok(twice) => twice,
            Err(error) => {
                return DocumentCheck::Violation(format!("reformat failed: {error}"))
            }
        };
        if twice != once {
            return DocumentCheck::Violation("not idempotent".to_owned());
        }
        match (program_shape(name, input), program_shape(name, &once)) {
            (Ok(before), Ok(after)) if before == after => DocumentCheck::Verified,
            (Ok(_), Ok(_)) => {
                DocumentCheck::Violation("formatted output changed the program shape".to_owned())
            }
            (_, Err(error)) => {
                DocumentCheck::Violation(format!("formatted output does not re-parse: {error}"))
            }
            (Err(_), _) => DocumentCheck::Unparseable("input does not parse".to_owned()),
        }
    }

    fn program_shape(name: &str, text: &str) -> Result<String, String> {
        let source = SourceFile::new(name.to_owned(), text.to_owned());
        let mut sink = DiagnosticSink::new();
        let tokens = zirk_lexer::tokenize(&source, &mut sink);
        let program = zirk_parser::parse(&source, &tokens, &mut sink);
        if sink.has_errors() {
            return Err(render_diagnostics(&sink));
        }
        let rendered = format!("{program:?}");
        let mut output = String::with_capacity(rendered.len());
        let mut remaining = rendered.as_str();
        while let Some(start) = remaining.find("Span {") {
            output.push_str(&remaining[..start]);
            let span = &remaining[start..];
            let end = span.find('}').expect("debug span closes");
            remaining = &span[end + 1..];
        }
        output.push_str(remaining);
        Ok(output)
    }

    struct Document {
        name: String,
        source: String,
    }

    fn collect_zrk_files(dir: &std::path::Path, documents: &mut Vec<Document>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries = entries
            .flatten()
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect_zrk_files(&path, documents);
            } else if path.extension().is_some_and(|extension| extension == "zrk") {
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                documents.push(Document {
                    name: path.display().to_string(),
                    source,
                });
            }
        }
    }

    fn collect_markdown_fences(dir: &std::path::Path, documents: &mut Vec<Document>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries = entries
            .flatten()
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect_markdown_fences(&path, documents);
                continue;
            }
            if !path.extension().is_some_and(|extension| extension == "md") {
                continue;
            }
            let Ok(markdown) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (line, fence) in zirk_fences(&markdown) {
                documents.push(Document {
                    name: format!("{}:{line}", path.display()),
                    source: fence,
                });
            }
        }
    }

    /// The companion site checkout contributes its curated playground
    /// sources. When the checkout is absent the harness still covers the
    /// handbook fences those examples cite as their `sourcePath`.
    fn collect_site_examples(root: &std::path::Path, documents: &mut Vec<Document>) {
        let site = std::env::var("ZIRK_LANG_SITE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| root.join("../zirk-lang-site"));
        let Ok(generator) = std::fs::read_to_string(site.join("tools/playground/generate.ts"))
        else {
            return;
        };
        let mut rest = generator.as_str();
        let mut index = 0usize;
        while let Some(marker) = rest.find("source:") {
            let after_marker = rest[marker + "source:".len()..].trim_start();
            let Some((literal, consumed)) = read_ts_string_literal(after_marker) else {
                rest = &rest[marker + "source:".len()..];
                continue;
            };
            index += 1;
            documents.push(Document {
                name: format!("zirk-lang-site playground example #{index}"),
                source: literal,
            });
            rest = &after_marker[consumed..];
        }
    }

    /// Reads one `'...'` or `` `...` `` TypeScript literal at the start of
    /// `text`, returning the unescaped contents and the number of bytes the
    /// literal occupied. `String.raw` templates and concatenations are not
    /// `source:` shapes, so they are left out.
    fn read_ts_string_literal(text: &str) -> Option<(String, usize)> {
        let quote = *text.as_bytes().first()?;
        if quote != b'\'' && quote != b'`' {
            return None;
        }
        let mut output = String::new();
        let mut escaped = false;
        for (offset, character) in text[1..].char_indices() {
            if escaped {
                output.push(match character {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    other => other,
                });
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote as char {
                return Some((output, offset + 2));
            } else {
                output.push(character);
            }
        }
        None
    }

    /// ` ```zirk ` fenced blocks, returned as (line of the opening fence,
    /// code). Four-backtick outer blocks are not used in this repository's
    /// docs, so a bare ` ``` ` always closes.
    fn zirk_fences(markdown: &str) -> Vec<(usize, String)> {
        let mut fences = Vec::new();
        let mut open = None;
        let mut buffer = String::new();
        for (index, line) in markdown.lines().enumerate() {
            let trimmed = line.trim();
            match &mut open {
                None if trimmed.starts_with("```zirk") => {
                    open = Some(index + 1);
                    buffer.clear();
                }
                Some(start) if trimmed == "```" => {
                    fences.push((*start, std::mem::take(&mut buffer)));
                    open = None;
                }
                Some(_) => {
                    buffer.push_str(line);
                    buffer.push('\n');
                }
                None => {}
            }
        }
        fences
    }

    /// Mirror of the playground's `wrapTopLevelStatements`: declarations and
    /// imports stay at the top level; bare statements move into a synthetic
    /// `fn main` appended after them. Returns `None` when the source already
    /// declares `fn main` or holds no statements to wrap.
    fn wrap_top_level_statements(source: &str) -> Option<String> {
        if source.contains("fn main") || source.trim().is_empty() {
            return None;
        }
        fn starts_declaration(line: &str) -> bool {
            let head = line.split_whitespace().next().unwrap_or("");
            matches!(
                head,
                "import"
                    | "use"
                    | "class"
                    | "enum"
                    | "interface"
                    | "trait"
                    | "record"
                    | "contract"
                    | "type"
                    | "extern"
                    | "share"
                    | "fn"
            ) || line.starts_with('#')
                || line.starts_with('@')
        }
        let mut declarations = Vec::new();
        let mut statements = Vec::new();
        let mut in_declaration = false;
        for line in source.lines() {
            if starts_declaration(line) {
                in_declaration = true;
                declarations.push(line.to_owned());
                continue;
            }
            if in_declaration
                && (line.trim().is_empty()
                    || line.starts_with([' ', '\t'])
                    || line.starts_with('}'))
            {
                declarations.push(line.to_owned());
                continue;
            }
            in_declaration = false;
            statements.push(if line.trim().is_empty() {
                String::new()
            } else {
                format!("{}{line}", " ".repeat(INDENT_WIDTH))
            });
        }
        if statements.iter().all(|line| line.trim().is_empty()) {
            return None;
        }
        let mut wrapped = declarations.join("\n");
        if !wrapped.is_empty() {
            wrapped.push('\n');
        }
        wrapped.push_str("fn main(): Void {\n");
        wrapped.push_str(&statements.join("\n"));
        wrapped.push_str("\n}\n");
        Some(wrapped)
    }

    fn assert_program_shape_is_stable(name: &str, input: &str, formatted: &str) {
        assert_eq!(
            program_shape(name, input).unwrap_or_else(|error| panic!("{name}: {error}")),
            program_shape(name, formatted).unwrap_or_else(|error| panic!("{name}: {error}")),
            "{name}"
        );
    }

    #[test]
    fn function_signatures_stay_flat_or_break_all_parameters() {
        assert_eq!(
            format("f.zrk", "fn f(a: Int32, b: String): Void { }\n").unwrap(),
            "fn f(a: Int32, b: String): Void { }\n"
        );
        let params = (0..12)
            .map(|index| format!("parameter{index}: VeryLongTypeName"))
            .collect::<Vec<_>>()
            .join(", ");
        let formatted = format("f.zrk", &format!("fn f({params}): Void {{ }}\n")).unwrap();
        assert!(formatted.contains("\n    parameter0: VeryLongTypeName,"));
        assert!(formatted.contains("\n) : Void") || formatted.contains("\n): Void"));
    }

    #[test]
    fn declaration_header_replacements_preserve_function_syntax() {
        let source = "share unsafe fn identity<T>(value: T): T throws Error { return value; }\n";
        assert_eq!(format("f.zrk", source).unwrap(), source);
    }

    #[test]
    fn class_members_and_declaration_headers_are_canonicalized() {
        let source = "class User implements One,Two {\nprivate   inmut age : Int32 =  42;\nconstruct( public name :String) { }\ngreet( value :Int32): String { return \"ok\"; }\n}\n";
        let formatted = format("class.zrk", source).unwrap();
        assert!(formatted.contains("class User implements One, Two {"));
        assert!(formatted.contains("private inmut age: Int32 = 42;"));
        assert!(formatted.contains("construct(public name: String) { }"));
        assert!(formatted.contains("greet(value: Int32): String { return \"ok\"; }"));
        assert_eq!(format("class.zrk", &formatted).unwrap(), formatted);
    }

    #[test]
    fn enum_and_contract_members_keep_their_declared_shapes() {
        let source = "share enum Outcome { Ok( value : String),Fail }\ninterface Display { show( prefix :String): String; }\n";
        let formatted = format("members.zrk", source).unwrap();
        assert!(
            formatted.contains("share enum Outcome { Ok(value: String), Fail }"),
            "{formatted}"
        );
        assert!(
            formatted.contains("interface Display { show(prefix: String): String; }"),
            "{formatted}"
        );
        assert_eq!(format("members.zrk", &formatted).unwrap(), formatted);
    }

    #[test]
    fn comments_follow_constructs_and_keep_trailing_spacing() {
        let source = "fn main(): Void {\n  // leading\n    mut x: Int32 = 1;    // trailing\n}\nfn empty(): Void {\n// empty block comment\n}\n";
        let formatted = format("comments.zrk", source).unwrap();
        assert!(formatted.contains("    // leading\n    mut x: Int32 = 1; // trailing"));
        assert!(formatted.contains("fn empty(): Void {\n    // empty block comment\n}"));
    }

    #[test]
    fn comments_in_lists_and_at_eof_are_preserved() {
        let source = "fn main(): Void {\n    stdout.println(\n// list interior\n        1\n    );\n}\n  // file trailing\n";
        let formatted = format("comments.zrk", source).unwrap();
        assert!(
            formatted.contains("stdout.println(\n        // list interior\n        1"),
            "{formatted}"
        );
        assert!(formatted.ends_with("// file trailing\n"));
        assert_eq!(format("comments.zrk", &formatted).unwrap(), formatted);
    }

    #[test]
    fn nested_control_flow_blocks_use_four_space_indentation() {
        let source =
            "fn main(): Void {\nif true {\nmut x: Int32 = 0;\nwhile x < 2 {\nx = x + 1;\n}\n}\n}\n";
        let formatted = format("blocks.zrk", source).unwrap();
        assert_eq!(
            formatted,
            "fn main(): Void {\n    if true {\n        mut x: Int32 = 0;\n        while x < 2 {\n            x = x + 1;\n        }\n    }\n}\n"
        );
        assert_eq!(format("blocks.zrk", &formatted).unwrap(), formatted);
    }

    #[test]
    fn long_calls_break_every_argument_while_short_calls_stay_inline() {
        let arguments = (0..8)
            .map(|index| format!("namedArgument{index}: \"a sufficiently long value\""))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "fn main(): Void {{\n    very_long_function_name({arguments});\n    short_call(1, 2);\n}}\n"
        );
        let formatted = format("calls.zrk", &source).unwrap();
        assert!(formatted.contains("very_long_function_name(\n        namedArgument0:"));
        assert!(
            formatted.contains("        namedArgument7: \"a sufficiently long value\",\n    );")
        );
        assert!(formatted.contains("short_call(1, 2);"));
        assert_eq!(format("calls.zrk", &formatted).unwrap(), formatted);
    }

    #[test]
    fn expressions_and_long_collections_use_canonical_spacing_and_breaking() {
        let values = (0..12)
            .map(|index| format!("veryLongCollectionValue{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let source = format!(
            "fn main(): Void {{\nmut answer: Int32=-value+other*2;\nmut mapper=(a: Int32,b: Int32): Int32=>a+b;\nmut message=\"value {{answer}}\";\nmut values=[{values}];\n}}\n"
        );
        let formatted = format("expressions.zrk", &source).unwrap();
        assert!(
            formatted.contains("mut answer: Int32 = -value + other * 2;"),
            "{formatted}"
        );
        assert!(formatted.contains("(a: Int32, b: Int32): Int32 => a + b"));
        assert!(formatted.contains("mut message = \"value {answer}\";"));
        assert!(
            formatted.contains("mut values = [\n        veryLongCollectionValue0,"),
            "{formatted}"
        );
        assert!(formatted.contains("        veryLongCollectionValue11,\n    ];"));
        assert_eq!(format("expressions.zrk", &formatted).unwrap(), formatted);
    }
}
