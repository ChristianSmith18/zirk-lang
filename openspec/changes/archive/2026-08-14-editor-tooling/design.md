## Context

Editor support was developed on the `feature/syntax-highlighting` branch and brought into `develop` as code. This change registers it retroactively.

It does not touch any compiler crate: it is VS Code configuration files plus a formatting script. The technical risk is zero; what does exist are two decisions worth writing down.

## Goals / Non-Goals

**Goals:**

- Write Zirk with highlighting that reflects the language as the specs define it.
- Make lexicon coverage a verifiable requirement rather than something checked once.

**Non-Goals:**

- LSP, editor diagnostics, autocomplete, navigation, renaming. They require the incremental frontend from `ZIRK_COMPILER_SPEC.md` section 10 and are Phase 9.
- Define Zirk's canonical style. That is `zirk format`.

## Decisions

### D1 — The highlighter covers the whole language, not the implemented subset

An editor that only highlighted `fn`, `mut` and `if` would imply that `class` or `task` are not part of Zirk, when the spec defines them and the compiler already recognizes them to say in which phase they arrive.

The division of labor is: **the editor shows the language; the compiler says what is available.**

**Consequence:** every keyword added to the lexer in Phases 2 through 5 must also be added to the highlighter. It is debt that silently degrades, so correspondence with the lexer was made a verifiable requirement rather than a comment.

### D2 — The extension formatter is kept, marked as provisional

`ZIRK_COMPILER_SPEC.md` section 10 requires the official formatter to be **canonical, idempotent and without configuration that fragments style**. The extension formatter respects the editor's `tabSize` and `insertSpaces`, which is exactly what that requirement rules out. It also does not understand strings or comments: a line ending in `{` inside a comment shifts the next indentation.

**Discarded alternative:** remove it until `zirk format` exists. Discarded because it would leave anyone writing Zirk today without any indentation help for eight phases, in exchange for a purity nobody observes: an editor formatter is not the language formatter, and no one will confuse them if it is stated.

**What was done** is to write it where it is read: the extension README explains where it diverges from the spec and that it must be removed when `zirk format` exists.

## Risks / Trade-offs

- **Highlighting falls behind as the language grows** → Mitigation: correspondence with the lexer is a spec requirement, not a note. It should be automated as a test once more than one editor is supported.

- **The provisional formatter stays forever** → Mitigation: the requirement explicitly says it must delegate to `zirk format` when it exists, so the debt has a written exit condition.

- **Only VS Code is supported** → Accepted. The TextMate grammar is reusable by other editors; the configuration and formatter are not. It is resolved when someone needs it.

## Migration Plan

Does not apply: it is purely additive and does not touch the compiler.

## Open Questions

None.
