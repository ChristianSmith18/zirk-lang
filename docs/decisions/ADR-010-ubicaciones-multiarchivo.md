# ADR-010 — Locations across multiple files

- **Status:** accepted
- **Date:** August 14, 2026
- **Phase:** 2

## Context

Through Phase 2 the compiler processed **a single file**, and that's why `Span` is just a pair of byte offsets: `{ start, end }`. The one who interprets it is the single `SourceFile` that exists, and `location(span)` and `snippet(span)` are its own methods.

Modules break that assumption. An `import` brings in declarations from another file, and a diagnostic about that other file, rendered with the input `SourceFile`, would point at the wrong fragment of the wrong text — without failing, because the offsets are valid in any string long enough.

`Span` isn't just some internal structure: it travels in every AST node, in every IR instruction ([ADR-007](./ADR-007-forma-de-la-ir.md)) and, by Phase 8, inside the `.zpkg`. `ZIRK_COMPILER_SPEC.md` section 11 requires the debugger to have a faithful mapping to the `.zrk`. Changing its shape later costs more than deciding it now.

## Options considered

### A separate source map artifact, JavaScript-style

A separate file that maps output to input, generated at the end. Discarded without much debate: it solves the opposite problem. It's useful for reconstructing locations **after** compiling, and what's needed is to query them **during**. As an internal structure it would be an indirection nobody wants to pay for on every diagnostic.

### Global offset space, rustc-style

All files occupy a single virtual address space: file 2 starts where file 1 ends. `Span` doesn't change, and to know which file it belongs to, a binary search over the starting offsets is done.

It's the **cheapest option today**: it touches neither the lexer, nor the parser, nor the AST, nor the IR. Zero changes across five crates.

It's discarded for two reasons, both about where this project is heading and not about what this phase needs:

1. **A file that changes size shifts the base of every following file**, and with it invalidates all their spans. An LSP re-lexes a file on every keystroke: editing the first file would invalidate the spans of everything else. The LSP is Phase 9 and incremental compilation comes after; adopting now a representation that hinders them is choosing the more expensive of the two costs.

2. **A calculation error in the base doesn't fail: it lies.** A span from file A interpreted against file B produces a plausible diagnostic that points at the wrong place. A diagnostic that points wrong is worse than one that doesn't show up at all.

rustc adopted this design before having LSP ambitions, and compensates for it by relativizing spans when serializing for the incremental cache. It's a patch to an early decision, not a model to copy.

## Decision

**A `Span` names its file. Offsets remain local to it.**

```rust
pub struct FileId(pub u32);

pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}
```

A `SourceMap` owns the crate's `SourceFile`s and answers `location(span)` and `snippet(span)` by dispatching on `span.file`. Stages receive `&SourceMap` where they used to receive `&SourceFile`; the shape of the calls doesn't change.

`Span::to()` combines two spans and **requires them to be from the same file**: combining locations from different files means nothing, and making it impossible by construction is half the value of this decision.

### On size

`Span` goes from 8 to 12 bytes. In a large crate — on the order of 100,000 AST nodes — that's about 400 KB extra, and a similar order of magnitude in the IR. For a compiler that isn't a quantity that justifies anything.

Packing the three fields into 64 bits (16-bit file, 24-bit offsets) was considered. It's discarded: alignment eats up a good part of the savings, accessors stop being plain fields, and debugging gets worse. If a profile ever proves it matters, packing is a local change to this type. Guessing it now is not.

## Consequences

- **Diagnostics are correct by construction across files.** There's no way to render a span against the wrong file without the type giving it away.
- **Editing one file doesn't invalidate the spans of the others**, which is the property the Phase 9 LSP and incremental compilation need.
- **The migration is mechanical but cross-cutting**: the lexer builds the spans and everything else propagates them, so there's a single creation point per file.
- `SourceFile` still exists with the same responsibility — text, lines, fragments — and stops being the entry point; `SourceMap` is now that.
- The serialized form of the Phase 8 `.zpkg` will have to decide whether `FileId` is stored as-is or reindexed per package. Noted, not resolved: it depends on the format, which doesn't exist yet.
