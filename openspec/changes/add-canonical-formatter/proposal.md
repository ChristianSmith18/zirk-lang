## Why

`zirk format` today is a deliberately narrow slice: it only canonicalizes
triple-quoted-string layout and leaves every other style choice untouched.
The roadmap promises a canonical, idempotent formatter; the LSP already
exposes formatting; and the new browser package makes it possible to offer
Format inside the playground. This change grows the slice into the real
formatter.

## What Changes

- A new platform-neutral `zirk-format` crate formats a whole document:
  indentation, intra-line spacing, line breaking, blank-line collapsing,
  and comment placement — TypeScript/Prettier-style semantics where a
  construct that exceeds the line width breaks *all* of its elements onto
  their own lines.
- Comments are preserved and re-anchored: leading comments keep the
  indentation level of the construct they belong to, and trailing comments
  stay on their line separated by a single space. The lexer currently
  discards trivia, so the formatter gains a trivia-preserving token stream.
- `// zirk-compiler: fmt off` / `// zirk-compiler: fmt on` directive
  comments mark regions the formatter emits verbatim.
- The CLI accepts one or many files and directories (recursive `.zrk`
  discovery), keeps `--check`, and adds `--diff` to preview changes.
- The browser package gains a versioned `format` operation so the
  playground can format source inside the worker; the manifest's operation
  list and the TypeScript declarations reflect it.
- The LSP's `textDocument/formatting` answer upgrades from the
  triple-quote slice to full canonical formatting with no protocol change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `canonical-formatter`: the capability grows from the `'''`-only slice to
  full canonical document formatting — whitespace, breaking, comments,
  directives, multi-target CLI.
- `zirk-cli-commands`: the `zirk format` subcommand contract changes from a
  single file to files, directories, `--check`, and `--diff`.
- `compiler-workspace`: the workspace gains the `zirk-format` crate and its
  dependency rules.
- `zirk-browser-compiler`: the versioned package contract gains a `format`
  operation with bounded request/response types.
- `zirk-lsp`: the formatting requirement's observable behavior becomes full
  document formatting.

## Impact

- New crate `zirk-format`; `zirk-lexer` gains a trivia-preserving mode (or
  equivalent comment access) without changing its default contract.
- `zirk-cli`: `zirk format` re-plumbs onto the engine with multi-target
  input; `zirk-check` unaffected.
- `zirk-browser`: new `format` operation in the versioned contract,
  manifest, and generated TypeScript declarations — `apiVersion` stays 1
  because the addition is backward-compatible for existing consumers.
- `zirk-lsp` / VS Code extension: formatting returns full-document edits.
- `../zirk-lang-site`: companion change needed to expose Format in the
  playground UI and to re-acquire the rebuilt package; plus the usual
  handbook/status sync for documentation updates.
