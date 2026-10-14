# zirk-lsp-mvp

## Why

Editor support today is a CLI bridge: the extension spawns `zirk-check` per
file on disk and caches its `--types` output. That makes hover intermittent —
absent until a check pass finishes, absent entirely while the file has syntax
errors, and blind to unsaved text. Diagnostics lag behind the keystroke because
the compiler only sees what was written to a temp file, after the fact.

The compiler frontend already runs fully in process — lexer, parser, module
loader and checker are libraries with span-carrying output. A minimal language
server that reuses them gives Zirk the "real editor" baseline (live
diagnostics, hover, symbols, formatting on unsaved text) without waiting for
the full incremental LSP scoped in Phase 9.

## What Changes

- New crate/binary `zirk-lsp`: a `tower-lsp` server speaking LSP over stdio.
- In-memory analysis: the frontend gains an entry point that accepts document
  text directly instead of reading only paths from disk, so the server answers
  from what the editor holds, not from what was last saved.
- Server capabilities, deliberately scoped to the MVP: `textDocument/didOpen`,
  `didChange` (full sync), `didClose`; `publishDiagnostics` from the checker's
  `DiagnosticSink`; `textDocument/hover` from the existing type map;
  `textDocument/documentSymbol` from the AST; `textDocument/formatting` via
  `zirk_cli::format`.
- The VS Code extension spawns `zirk-lsp` through `vscode-languageclient` when
  the binary is available, and keeps the `editor-cli-bridge` (CLI check +
  `--types` hover) as the fallback when it is not.
- Not in scope: completion, go-to-definition, references, rename, signature
  help, semantic tokens, incrementality and cancellation. Those need the
  queryable symbol API and error-tolerant parsing that Phase 9 owns.

## Capabilities

### New Capabilities

- `zirk-lsp`: the language server itself — protocol lifecycle, document
  synchronization over in-memory text, published diagnostics, hover, document
  symbols, and formatting, all sourced from the existing frontend crates.

### Modified Capabilities

- `editor-tooling`: the "Scope without a language server" requirement changes —
  autocomplete/navigation/rename stay out of scope, but diagnostics, hover,
  symbols and formatting are now provided by `zirk-lsp` when the server binary
  is present, with the CLI bridge remaining as the no-server fallback.

## Impact

- **New crate** `crates/zirk-lsp` (workspace member) plus a `zirk-lsp` binary.
- **`zirk-cli` / module loading**: gains a text-based load path alongside the
  path-based one; `run_frontend_lenient` semantics are reused.
- **`editors/vscode`**: `extension.js` gains the language-client wiring and a
  `zirk.lspPath`/auto-discovery setting; `package.json` gains the
  `vscode-languageclient` dependency. Installed copies under
  `~/.devin/extensions/` follow the AGENTS.md sync procedure.
- **New dependency**: `tower-lsp` (Rust), `vscode-languageclient` (npm).
- **Public docs/status**: the feature-status sources and handbook LSP chapter
  describe the LSP as Phase 9 work; this change delivers a documented MVP
  subset, so `docs/init/ZIRK_FEATURE_STATUS.md` and the `11-lsp.md` status note
  must be updated, and `../zirk-lang-site` re-synced with
  `./scripts/sync-website-content.sh` (plus `--audit-date` if the status
  evidence moved).
