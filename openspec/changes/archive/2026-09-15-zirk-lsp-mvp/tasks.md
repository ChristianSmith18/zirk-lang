# zirk-lsp-mvp — tasks

## 1. Frontend support for in-memory sources

- [x] 1.1 Add an overlay entry point to `zirk-cli/src/modules.rs` (e.g. `load_with_overlay(entry, overlay, sink)` or an `overlays` field consulted before `fs::read_to_string`), keeping import resolution from disk unchanged; verify with a unit test where the entry text lives only in memory and an `import` still resolves from disk
- [x] 1.2 Expose a text-level formatting entry point from `zirk-cli/src/format.rs` (e.g. `pub fn format_source(&str) -> Result<String, String>` wrapping tokenize + parse + `format_text`); verify it returns the canonical layout for a `'''` literal and an error for unparseable text
- [x] 1.3 Add a `span → (line, utf16 column)` conversion in `zirk-diagnostics` shared by the server and the existing JSON diagnostic render; verify the JSON diagnostics output is byte-identical before and after the refactor

## 2. `zirk-lsp` crate and server skeleton

- [x] 2.1 Create `crates/zirk-lsp` as a workspace member with a `zirk-lsp` binary on `tower-lsp` (released ≥7 days, per dependency policy), declaring capabilities per spec; verify `cargo build -p zirk-lsp` succeeds and the binary answers `initialize`/`shutdown`/`exit` over stdio
- [x] 2.2 Implement the document store: `didOpen`/`didChange` (full sync)/`didClose` keep latest text + version per URI; verify a unit test shows edits replace text wholesale and `didClose` drops state
- [x] 2.3 Implement the serialized analysis pass: `load_with_overlay` → `zirk_sema::check` inside `spawn_blocking`, tagged with the document version, storing `CheckedProgram` + `SourceMap` + text only when the version is still current; verify a test where a superseded run does not overwrite newer state

## 3. LSP features

- [x] 3.1 Publish diagnostics after open and after each change, mapping `DiagnosticSink` spans to LSP ranges and severities, clearing on `didClose`; verify by driving the server over stdio with a scripted client (error text produces the diagnostic, fixing it publishes an empty set)
- [x] 3.2 Implement `textDocument/hover` from the stored `type_map` using the smallest-containing-span rule with declaration kinds beating expressions, rendering a `zirk` code block plus `*(inferred)*` when marked; verify against `examples/inferred_returns_examples.zrk` that `square` reports `fn square(n: Int32): Int32` marked inferred and that hover on unanalyzable text returns `null`
- [x] 3.3 Implement `textDocument/documentSymbol` from the merged `Program` (functions, classes with nested members, enums, contracts, type aliases, externs); verify the symbol tree for an example file matches its declarations with correct ranges
- [x] 3.4 Implement `textDocument/formatting` via the new `format_source` entry, returning one full-range `TextEdit` only when the output differs; verify an idempotent format request on canonical text returns no edits

## 4. Editor extension wiring

- [x] 4.1 Add `vscode-languageclient` to `editors/vscode/package.json` and a `zirk.lspPath` configuration; verify `npm install` (or the repo's package flow) succeeds
- [x] 4.2 In `extension.js`, resolve `zirk-lsp` through the existing `findCompiler` roots (`zirk.lspPath`, `target/{debug,release}/zirk-lsp`, PATH) and start a `LanguageClient` for the `zirk` language id when found; verify the client starts when `target/debug/zirk-lsp` exists
- [x] 4.3 Gate the CLI-bridge check and `--types` hover so they do not run for documents the server covers, keeping them as the fallback when no server binary exists; verify both paths: with the binary, hover/diagnostics come from the server; without it, behavior matches the current extension
- [x] 4.4 Bump the extension version, copy `editors/vscode` to `~/.devin/extensions/christiansmith.zirk-lang-<new-version>/` per AGENTS.md, and validate the JSON/JS loads (e.g. `node -e "require('./extension.js')"` shape check / Extension Host log)

## 5. Documentation and site sync

- [x] 5.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` and the status note in `docs/handbook/07-toolchain/11-lsp.md` to describe the delivered MVP subset (diagnostics, hover, symbols, formatting) versus the Phase 9 remainder; verify wording claims only what this change ships
- [x] 5.2 Update the extension README's feature list and the "features that require an LSP" note accordingly; verify no doc still claims hover does not exist
- [ ] 5.3 After committing the repo changes, run `./scripts/sync-website-content.sh` (with `--audit-date` if status evidence changed) so `../zirk-lang-site` reflects the new state; verify the script completes cleanly
