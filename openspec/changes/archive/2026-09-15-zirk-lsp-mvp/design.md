# zirk-lsp-mvp — design

## Context

The frontend is already a library: `zirk-cli/src/modules.rs::Loader` loads and
parses a crate (entry file plus imports) into `SourceMap` + `Program`,
`zirk_sema::check` produces a `CheckedProgram` whose `type_map` carries spans,
kinds, names and `inferred` flags, and `zirk-cli/src/format.rs::format_text`
canonicalizes a text given its tokens. The editor extension consumes the same
data today, but only through spawned processes on disk files
(`editor-cli-bridge`), which is what makes hover and diagnostics lag behind
unsaved text.

## Goals / Non-Goals

**Goals:**

- One in-process analysis path for open `.zrk` documents: in-memory text in,
  `DiagnosticSink` + `CheckedProgram` + `Program` out.
- LSP surface: document sync, `publishDiagnostics`, `hover`,
  `documentSymbol`, `formatting`.
- The extension keeps working — unchanged behavior — when no server binary
  exists.

**Non-Goals:**

- Incremental snapshots, cancellation, per-symbol invalidation (Phase 9).
- Completion, definition, references, rename, signature help, semantic tokens
  — they need a queryable symbol API in `zirk-sema` that does not exist yet.
- Error-tolerant parsing. When parsing fails the server still publishes
  diagnostics but answers `null` for hover/symbols on that text — same data
  ceiling the CLI bridge has today, minus the latency.

## Decisions

### 1. New crate `crates/zirk-lsp`, binary `zirk-lsp`, built on `tower-lsp`

`tower-lsp` gives the stdio transport, JSON-RPC codec, and lifecycle plumbing
for free; `lsp-server` (rust-analyzer's) offers finer control over
cancellation we explicitly do not need in the MVP. `zirk-lsp` depends on the
`zirk-cli` library target (`frontend`, `modules`, `format`) plus the stage
crates — no logic is duplicated.

*Alternative considered:* keep spawning `zirk-check` per request. Rejected —
it is the current bridge's own bottleneck and cannot see unsaved text.

### 2. In-memory sources via an overlay map in `Loader`

`Loader::load` currently does `std::fs::read_to_string(path)`. Add
`Loader::load_with_overlay(entry, overlay: &HashMap<PathBuf, String>, sink)`
(or an `overlays` field consulted before `fs::read_to_string`). The open
document is keyed by its filesystem path; its text always comes from the
overlay while imports keep resolving from disk. This is the single
compiler-side change the server needs — imports, spans and `SourceMap`
behavior are untouched.

*Alternative considered:* write the document to a temp file (what the live
check does today). Rejected — it reintroduces the staleness this change
removes, and litters the tree with temp files.

### 3. Full-text document sync

Advertise `TextDocumentSyncKind::Full`: every `didChange` carries the whole
document text. Incremental sync would save bytes but adds line/offset
bookkeeping for files that are a few KB. Revisit in Phase 9.

### 4. Re-run the whole frontend per analysis, serialized

Each analysis is `Loader` → `parse` → `zirk_sema::check` on the latest text —
no caching between runs, no LLVM. Frontend cost on example-sized files is
milliseconds. Analyses run inside `tokio::task::spawn_blocking` (sema is
synchronous) and are serialized per document; each run is tagged with the
document version it analyzed, and results are only published/applied if the
version is still current — a stale run never overwrites newer text.

*Alternative considered:* per-request analysis (analyze on every hover).
Rejected — diagnostics need a push pass on change anyway, so one analysis per
change that stores `CheckedProgram` + `SourceMap` + text serves both.

### 5. Hover, symbols, formatting reuse existing outputs

- **Hover**: LSP position → byte offset via the stored text; pick the
  smallest `type_map` span containing it, declaration kinds beating
  expressions on ties — the same rule `extension.js` implements today, moved
  server-side. Render `MarkdownString`-equivalent markup with a `zirk` code
  block and `*(inferred)*` marker.
- **Document symbols**: walk the merged `Program` — functions, classes
  (methods and fields nested), enums, contracts, type aliases, externs —
  mapping AST spans to LSP ranges.
- **Formatting**: expose `format_text` (or a `format_source(&str)` wrapper
  that tokenizes/parses internally) from `zirk-cli`; return a single
  full-range `TextEdit` when the result differs, none otherwise.

### 6. Position mapping helper in `zirk-diagnostics`

Spans are byte offsets into a `SourceMap` file; LSP wants line/character.
Add a `span → (line, utf16 column)` conversion next to `SourceMap` so the
server and any future tool share one implementation (diagnostics JSON already
computes line/column — factor that out rather than duplicating it).

### 7. Extension: `vscode-languageclient` with bridge fallback

`extension.js` resolves `zirk-lsp` using the existing `findCompiler` search
roots (`target/{debug,release}/zirk-lsp`, `zirk.lspPath` setting, PATH). When
found, it starts a `LanguageClient` for the `zirk` language id and skips the
CLI check/`--types` hover for documents the server covers; when absent,
everything behaves exactly as today. `package.json` gains
`vscode-languageclient` as a dependency.

## Risks / Trade-offs

- [Syntax errors yield no analysis at all] → inherited from the frontend;
  diagnostics still publish, and interactive requests return `null` per spec.
  Parser recovery is Phase 9 scope; the server degrades no worse than the
  bridge does today.
- [A slow analysis could publish stale diagnostics after a newer edit] →
  every run carries the document version; publish only when it matches the
  latest.
- [`tower-lsp` maintenance cadence] → pin a released version ≥7 days old per
  repo policy; the API surface used is small and stable.
- [Two copies of the extension installed] (`~/.devin/extensions/` keeps old
  versioned folders) → document the AGENTS.md copy step; an outdated sibling
  folder can shadow providers.

## Migration Plan

1. Add `crates/zirk-lsp` and the `Loader` overlay + `format` text entry point;
   `cargo build -p zirk-lsp`.
2. Update `editors/vscode` (extension + `package.json`), copy to
   `~/.devin/extensions/christiansmith.zirk-lang-<version>/` per AGENTS.md.
3. Update `docs/init/ZIRK_FEATURE_STATUS.md` and `11-lsp.md` status note;
   run `./scripts/sync-website-content.sh` for `../zirk-lang-site`.

Rollback: delete the `zirk-lsp` binary or unset `zirk.lspPath` — the
extension falls back to the bridge with no code change needed.

## Open Questions

- Whether `zirk-lsp` should also serve `.zkinit` documents (they share the
  grammar but have no checker semantics) — deferrable; MVP can serve `.zrk`
  only.
