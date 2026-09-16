## Context

`zirk-cli::format` currently rewrites only `'''` literal layout, working
directly on the raw text plus token spans — it never reprints code, which
is why it can afford to ignore trivia entirely. The `zirk-lexer` skips
whitespace and comments (`skip_trivia`), so a full formatter that
re-prints the document must recover comments from somewhere else. The LSP
already calls `format_source(name, text)` for in-memory documents, and
`zirk-browser` provides the versioned `check`/`run` contract this change
extends with `format`.

## Goals / Non-Goals

**Goals:**

- One engine, `zirk-format`, that turns parsed source into canonical text:
  4-space indentation, normalized spacing, width-driven all-or-nothing
  breaking, collapsed blank lines, preserved comments, and verbatim
  `fmt off`/`fmt on` regions.
- A trivia-preserving token stream so comments survive re-printing.
- `zirk format` over files and directories, `--check`, `--diff`.
- A `format` operation in the browser package, reused by the LSP.

**Non-Goals:**

- Formatting `.zkinit` manifests or non-`.zrk` files.
- Configurable style: the formatter is canonical; the only knobs are the
  off/on directives. (A `zirk.toml`-style config is a separate change.)
- Comment rewrapping, sorting `import` lists, or semantic transforms.
- Sorting/normalizing member order — the formatter never reorders code.
- stdin input and LSP range-formatting (`textDocument/rangeFormatting`),
  left for later slices if wanted.

## Decisions

### 1. AST for structure, trivia stream for comments

The formatter walks the AST to know *what* each construct is (block,
member, list, expression) and drives a comment channel fed by a
trivia-preserving lex: `zirk-lexer` gains a `tokenize_with_trivia` (name
TBD) that additionally yields comment tokens with their spans. Whitespace
never needs to be a token — column/line info from spans is enough to
classify each comment as leading (own line) or trailing (shares a line).

Attachment rules, mirroring the TypeScript/Prettier behavior the user
asked for:

- A comment on its own line attaches to the *next* construct and is
  emitted at that construct's indentation level.
- A comment sharing a line with code attaches to that line and is emitted
  after one space.
- Comments between two elements of a broken list keep their own line at
  the list's indentation level.
- Comments between the last element and a closing delimiter are emitted
  before the delimiter at the element indentation.

*Alternatives considered:* (a) keep rewriting raw text — rejected, it
cannot reflow anything; (b) full CST/trivia tree — rejected for now as too
big an upfront refactor of parser and AST consumers; attaching comments
positionally to AST-guided output gives the same observable contract with
far less machinery.

### 2. A document-IR pretty printer (group / indent / break)

The printer builds a `Doc` tree (Prettier-style: `text`, `line`,
`softline`, `group`, `indent`, `join`) and renders it against the line
width. A `group` either fits flat or breaks *all* of its `line`s — which
is exactly the all-or-nothing list behavior the user described. This keeps
breaking decisions declarative and testable per construct.

**Canonical width: 100 columns, indent: 4 spaces.** Both are stated in the
spec as documented constants; 4 matches every handbook example and the
existing corpus, 100 matches the de-facto line lengths already in the
docs. If either proves wrong while dogfooding the corpus it is a
one-constant change — not a spec change, since the spec only requires a
documented constant.

### 3. `fmt off`/`fmt on` handled pre-print, emitted verbatim

Before printing, the trivia stream is scanned for
`// zirk-compiler: fmt off` / `// zirk-compiler: fmt on` directive
comments. Disabled regions are emitted byte-for-byte from the original
source at the surrounding indentation's position; everything else flows
through the printer. An `on` without `off` warns; `off` at EOF is legal;
directives never nest (a second `off` inside is an ordinary comment).

### 4. Engine is pure text-in/text-out; IO lives in the CLI

`zirk-format` exposes `format(name, source) -> Result<String, Diagnostics>`
and nothing else — no filesystem, no clocks, no process. That keeps it
`wasm32-unknown-unknown`-compatible by construction, reusable by
`zirk-cli` (files/dirs/`--check`/`--diff`), `zirk-lsp`
(`textDocument/formatting`), and `zirk-browser` (the new `format`
operation). The existing `'''` rule and the "never rewrite unparseable
input" contract move into the engine unchanged.

### 5. `format` joins the browser contract as an additive v1 operation

`zirk-browser` adds `format` to its versioned operations: request is just
the source plus the existing limit fields; response is either formatted
source or the same ordered diagnostics `check` returns. The manifest's
`operations` list and generated `.d.ts` pick it up. `apiVersion` stays 1 —
consumers that only know `check`/`run` are unaffected; the version bump is
reserved for breaking contract changes.

### 6. Parse-stability is a first-class gate

Formatting must be a fixed point *and* a parse fixed point: the corpus,
every handbook example, and the site examples are formatted, re-formatted
(identical bytes), and re-parsed (identical program structure). This runs
as a dedicated test harness so reflow bugs surface as data, not as a
subjective review.

## Risks / Trade-offs

- **Comment attachment ambiguity.** Odd placements (comment after `{` on
  its own line, comment inside an empty block, file-trailing comments) can
  attach "wrong". → Attachment rules above are explicit and each oddity
  gets a golden test; worst case is a misplaced comment, never lost code —
  parse-stability guarantees that.
- **Trivia mode touches the lexer's hot path.** → The trivia mode is
  additive; `tokenize` keeps its exact current signature and cost.
- **Dogfooding churn.** First run over the repo's own `.zrk` corpus and
  doc examples will produce a large diff. → Land the engine, then apply it
  in one dedicated commit, and only then enable `--check` in CI.
- **Spec↔code drift on width/indent constants.** → Constants live in one
  place and are named in the docs.

## Migration Plan

1. Land `zirk-format` + CLI/LSP/browser plumbing with the new engine.
2. Reformat the repository corpus and doc examples in a single commit.
3. Add `zirk format --check` to `scripts/check-local.sh`/CI.
4. Companion site change: acquire the rebuilt package and wire Format
   into the playground UI.

## Open Questions

- Exact directive spelling (`// zirk-compiler: fmt off` is what the user
  asked for; final name decided when the lexer directive scan lands).
- Whether `--diff` output is unified-diff or a simpler before/after —
  decided at implementation; spec only requires printing the changes.
