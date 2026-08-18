# Lexer and Parser

The lexer answers “what tokens are present?” and the parser answers “how are
those tokens structured?”. Neither decides whether an operation is type-correct
or permitted. Keeping that boundary makes syntax reusable by the formatter and
LSP and prevents grammar rules from becoming accidental semantics.

> **Implementation status:** batch Unicode lexing and recursive-descent parsing
> are implemented for the current subset, with broad recognition of later
> tokens, recovery, source spans, precedence, and extensive tests. Incremental
> snapshots and the lossless syntax tree required by formatter/LSP are target
> architecture.

## Lexical contract

The lexer produces a token stream ending in `Eof`. Each token retains its byte
span in its source file. It recognizes the complete language vocabulary where
possible even when a later stage is unfinished, allowing a targeted pending-
feature diagnostic instead of “unexpected character”.

Token families include:

- case-sensitive identifiers and reserved keywords;
- integer and floating spellings, widths, bases and separators;
- canonical strings, characters, interpolation parts and escapes;
- `re'...'` regex literals with their pattern escapes preserved;
- native duration literals such as `5s`;
- longest-match operators such as `**=`, `..=`, `...`, `??`, `?.`, `|>`,
  shifts and compound assignment;
- delimiters and punctuation.

Lexical recognition is not semantic validation. For example, the lexer keeps
every code point between character quotes; the type/literal validator decides
whether the result is exactly one grapheme. Similarly, a regex token does not
mean the regex program is valid.

## Unicode and source positions

Canonical internal spans are byte offsets. Human diagnostics convert them to
one-based line and Unicode-scalar columns. LSP conversion additionally needs
UTF-16 positions. These conversions belong in the shared source snapshot rather
than being independently recomputed by every tool.

String and character literal contents are normalized according to the language
contract before runtime construction. Identifiers remain case-sensitive. A
capitalized keyword spelling is an identifier, not the lowercase keyword.

## Lexical failures

The lexer reports and advances after invalid input where safe. Existing stable
families cover unrecognized characters, unterminated strings/comments/chars/
regex/interpolation, unknown escapes, invalid numeric separators/suffixes, and
integer spellings beyond the lexer representation.

```text
error[E0202]: unterminated string literal
  --> src/main.zrk:4:17
   |
 4 | mut name = "Ada
   |            ^^^^ string starts here
   = cause: the closing quote was not found before the source ended
   = help: add `"` after the string content
```

Recovery must always consume input or reach EOF. Invalid bytes/tokens remain
represented in the future lossless tree so editor tools can operate on broken
documents without pretending the program is valid.

## Parser contract

The parser builds structured declarations, statements, expressions, types, and
patterns with spans. The current semantic AST represents imports, uses, enums,
classes/contracts under active integration, functions, blocks, loops, variables,
assignments, conditionals, returns, calls, member access, ranges, ternaries,
match, variants, lambdas, literals, unary and binary operators.

The parser owns:

- delimiter and declaration grammar;
- optional semicolon placement;
- operator precedence and associativity;
- whether braces are required or the single-statement `if` form applies;
- positional, named, optional/default, and variadic parameter syntax;
- parse-time desugaring whose semantic identity is specified, such as compound
  assignment and increment representation;
- grammar-specific diagnostics and synchronization.

The parser does not decide whether `Int + String` is legal, whether a match is
exhaustive, whether a name is visible, or whether a return type agrees.

## Precedence and ambiguity

Longest-match lexing resolves token boundaries; parser precedence resolves tree
shape. Parentheses override precedence. Associativity is explicit per operator,
including right-associative exponentiation/ternary forms where specified.

Context resolves syntactic ambiguity without type guessing. For example, a
parenthesized expression becomes a lambda only when the following lambda syntax
is present. `if` has one parsed structure; semantic context later determines
whether both branches produce an expression value.

## Optional semicolons and single-statement `if`

Semicolons may be omitted only where the grammar remains unambiguous. The parser
does not apply JavaScript-style automatic semicolon insertion.

The brace-free `if` governs exactly one following statement, on the same or next
line. It does not admit `else` or nested brace-free `if`. The following statement
is outside that conditional regardless of indentation. Braced forms are used for
multiple statements, nesting, and alternatives.

## Lossless syntax and semantic AST

The target frontend keeps two representations:

```text
lossless syntax: tokens + comments + whitespace + invalid/recovered nodes
semantic AST:    normalized declarations/expressions used by sema and IR
```

The lossless tree enables formatter stability, editor recovery, refactors, and
incremental subtree reuse. The semantic AST remains compact and free of trivia.
It should not be replaced merely to introduce the tooling tree.

## Recovery and limits

The parser synchronizes at safe declaration, member, statement, delimiter, and
arm boundaries. It continues after a malformed construct and guarantees forward
progress. Nesting is bounded at 128 to prevent hostile or accidental source from
overflowing the parser stack.

A recovered node carries error state. Later stages may inspect surrounding valid
declarations to find more errors, but recovered syntax cannot be lowered to a
binary. See [Diagnostic Recovery](04b-diagnostic-recovery.md).

## Later-feature diagnostics

Recognized-but-unavailable syntax uses the normal diagnostic structure and names
its private-development roadmap phase in `cause` or `help`. The compiler should
centralize stage status so accepting a feature in the parser does not leave tests
or semantic diagnostics claiming it is entirely unavailable.

---

**Previous:** [← Compiler Pipeline](01-compiler-pipeline.md) · **Next:** [Name Resolution →](03-name-resolution.md)
