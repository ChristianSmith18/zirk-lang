## Context

The VS Code extension in `editors/vscode/` is a pure-JavaScript extension. It currently uses `vscode.languages.createDiagnosticCollection` to publish a single hard-coded diagnostic for deprecated type names and `vscode.languages.registerDocumentFormattingEditProvider` for indentation. The compiler already exposes a frontend-only validator as `zirk-check` and the full `zirk check` subcommand; both emit JSON diagnostics with `severity`, `code`, `message`, `location`, `cause`, and `help`.

## Goals / Non-Goals

**Goals:**
- Make compiler diagnostics visible in the editor on open and save.
- Let the user run `Check`, `Build`, and `Run` from the command palette.
- Keep the editor integration working even when the LLVM backend is unavailable by preferring `zirk-check`.
- Allow the user to override the compiler binary path.
- Avoid a full LSP server; this is a CLI bridge, not a language server.

**Non-Goals:**
- Autocomplete, hover, go-to-definition, rename, or semantic tokens. Those require the incremental LSP in Phase 9.
- Real-time diagnostics while the user types. This is deferred to Phase 3 and may use a future `zirk-check --stdin` option or a temporary file.
- A canonical formatter. The existing provisional formatter stays; `zirk format` will replace it later.

## Decisions

### D1 — Use `zirk-check`/`zirk check --json` as the source of truth
The compiler already produces stable JSON diagnostics. Re-implementing checks in the extension would duplicate logic and drift from the language. We call the CLI and parse its JSON output from stderr.

### D2 — Executable discovery prefers workspace binaries, then PATH
Editors are often opened in the `zirk-lang` workspace, where `target/debug/zirk-check` is available. Search order:
1. `zirk.executablePath` configuration if set.
2. `target/debug/zirk-check` relative to the workspace root.
3. `target/release/zirk-check`.
4. `zirk` on the system PATH (which supports `zirk check --json <file>`).

### D3 — `zirk-check` arguments follow `<file> --json`
`zirk check` accepts `--json` anywhere, but the standalone `zirk-check` binary requires the file path first. We standardize on `<file> --json` for `zirk-check` and `check --json <file>` for `zirk`.

### D4 — Diagnostic ranges map from 1-based Unicode columns to VS Code's UTF-16 positions
The compiler reports `line` and `column` (1-based, Unicode character count). VS Code positions are 0-based UTF-16 code units. For the ASCII-heavy Zirk source this is effectively the same; non-ASCII code points may produce a small offset. We mark at least one character starting at `column - 1` and, when possible, extend to the end of the word at that position.

### D5 — Check on save, not on every keystroke
Reading from disk keeps imports relative to the entry file working and avoids a temporary file. Live checks without save require either writing a temp file or adding `--stdin` support to `zirk-check` and are deferred to Phase 3.

### D6 — Keep the provisional formatter until `zirk format` exists
The extension's current indentation formatter remains. When `zirk format` is available the formatter provider will detect the binary and delegate; otherwise it falls back to the current implementation.

## Risks / Trade-offs

- **CLI startup latency may feel slow for large files** → Mitigation: `zirk-check` is frontend-only and fast for the current subset; clear diagnostics before invoking so stale results are not shown while waiting.
- **Diagnosing unsaved files is not supported in Phase 1** → Mitigation: document that the user must save; Phase 3 may add `--stdin` or temp-file checks.
- **Imported files that are open but unsaved are read from disk** → Mitigation: document as a known limitation; full LSP will solve it.
- **Column mapping for non-ASCII source may be slightly off** → Mitigation: mark a single character at the reported column; the user can still identify the line.

## Migration Plan

- Add the new commands and diagnostics provider in `editors/vscode/extension.js`.
- Update `editors/vscode/package.json` with `activationEvents`, commands, configuration, and keybindings placeholders.
- Update `editors/vscode/README.md`.
- No migration is needed for existing users; the extension continues to activate when a `.zrk` file is opened.

## Open Questions

- Should the extension watch `target/debug/zirk-check` and auto-detect when the user rebuilds the compiler?
- Should `zirk-check` gain a `--stdin` mode so live checks can avoid disk writes?
