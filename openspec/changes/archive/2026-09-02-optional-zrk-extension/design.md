## Context

The `zirk` and `zirk-check` CLIs accept one source argument per invocation. Today this argument is treated as an exact path, so `zirk run main` fails unless a file literally named `main` exists; the canonical form is `zirk run main.zrk`. The compiler pipeline itself does not care about the extension once the file is loaded, so the restriction lives entirely at the CLI boundary.

The language spec and existing docs already show the `.zrk` extension in examples. This change normalizes the CLI so that the extension is an optional convenience rather than a requirement.

## Goals / Non-Goals

**Goals:**
- Allow `zirk build <name>`, `zirk run <name>`, and `zirk check <name>` to work as well as the `<name>.zrk` variants.
- Apply the same normalization to the standalone `zirk-check` binary.
- Preserve all existing behavior when the `.zrk` extension is already present.
- Update normative specs and tests to cover the extension-agnostic input.

**Non-Goals:**
- Removing the `.zrk` extension from `import` statements (they already omit it).
- Changing the lexer, parser, or diagnostics to recognize non-`.zrk` files.
- Supporting arbitrary extensions (e.g. `.txt` or `.zirk`).
- Modifying multi-file project handling (Phase 6).

## Decisions

### Where to resolve the path

The resolution happens at the CLI argument boundary, before the path reaches `frontend::run_frontend` or `driver::compile`. This keeps the shared driver and `modules::load` unchanged and ensures `zirk`, `zirk build`, `zirk run`, and `zirk-check` all behave identically.

### Resolution rule

If the user-provided argument ends with `.zrk`, use it verbatim. Otherwise, append `.zrk` and use the resulting path. This is deterministic, easy to document, and avoids the ambiguity of guessing whether a bare file name should win over a `.zrk` sibling.

### Shared helper placement

A small `resolve_source_path` function is added to `crates/zirk-cli/src/frontend.rs` because that module is already shared by `main.rs` and `bin/zirk-check.rs`. It returns a `PathBuf` so both callers can pass it through unchanged.

## Risks / Trade-offs

- **Diagnostic file paths will show the resolved `.zrk` name.** If the user typed `zirk run main`, the "could not read `main.zrk`" diagnostic is still accurate and actionable.
- **Existing tests that build the `zirk` argument manually will keep passing** because the `.zrk` form continues to be accepted.
- **No breaking changes** to valid commands; only previously invalid bare-name invocations become valid.
