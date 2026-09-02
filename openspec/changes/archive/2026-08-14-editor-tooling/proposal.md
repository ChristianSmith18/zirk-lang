## Why

Editor support was developed in a separate branch and brought into `develop` as code, without going through OpenSpec. This change registers it retroactively so the capability is recorded in `openspec/specs/` with verifiable requirements, instead of existing only as files in `editors/`.

What motivates registering it is not ceremony: **highlighting has a property that silently degrades**. Every keyword added to the lexer in Phases 2 through 5 —and there are many— can end up unhighlighted with no warning. Turning "the highlighter covers all keywords" into a requirement makes that debt visible.

## What Changes

- **VS Code extension** in `editors/vscode/`, with syntax highlighting, language configuration —comments, closing pairs, folding— and a provisional formatter.
- **Complete lexicon coverage**: the 46 keywords recognized by `zirk-lexer`, verified by cross-comparison.
- The highlighter covers the **whole language** of the specs, not only the implemented subset.

Explicitly **out of scope**: LSP, diagnostics inside the editor, autocomplete and navigation. All of that is Phase 9 and requires the compiler to expose an incremental frontend.

## Capabilities

### New Capabilities

- `editor-tooling`: editor support for writing Zirk — syntax highlighting, language configuration and its relationship with the canonical formatter that arrives in Phase 9.

### Modified Capabilities

None.

## Impact

**New:** `editors/vscode/` and an entry in `.gitignore` for `*.vsix`.

**No effect on the compiler:** no crate is touched. The test suite and CI do not change.

**Known tension:** the extension formatter respects editor configuration, and `ZIRK_COMPILER_SPEC.md` section 10 requires the official formatter to be canonical and without configuration that fragments style. It is kept as a provisional convenience while `zirk format` does not exist, documented in the extension README.
