## Why

The VS Code extension currently only provides syntax highlighting, auto-closing pairs, a provisional formatter, and a small regex-based diagnostic for deprecated type names. The compiler already emits structured JSON diagnostics via `zirk check --json <file>` and `zirk-check <file> --json`, but the editor does not consume them. Bridging the CLI and the editor now gives users immediate, accurate feedback without waiting for the incremental LSP in Phase 9.

## What Changes

- Extend `editors/vscode/extension.js` to run `zirk-check` (or `zirk check`) on the active `.zrk` file and publish real compiler diagnostics as `vscode.Diagnostic` entries.
- Add VS Code commands (`Zirk: Check File`, `Zirk: Build File`, `Zirk: Run File`), an output channel for compiler messages, and a status bar item that shows the latest check result.
- Add extension configuration (`zirk.executablePath`, `zirk.checkOnSave`) and automatic discovery of workspace binaries (`target/debug/zirk-check`, `target/release/zirk-check`, `zirk`, or PATH).
- Keep the provisional formatter, but prepare it to delegate to `zirk format` when that command lands in Phase 9.
- Update the `editor-tooling` capability spec and add an `editor-cli-bridge` spec to record the new diagnostics, command, and configuration requirements.

This is a phased rollout:
- **Phase 1** — diagnostics on save/open, executable discovery, output channel.
- **Phase 2** — `Build`/`Run` commands, status bar, workspace task provider.
- **Phase 3** — snippets, code actions for deprecated types, live debounced checks (possibly via a future `zirk-check --stdin` option).
- **Phase 9** — full LSP server replacing the CLI bridge.

## Capabilities

### New Capabilities
- `editor-cli-bridge`: Defines how the editor discovers the `zirk`/`zirk-check` executable, invokes it with `--json`, parses the structured diagnostics, maps them to editor positions, and exposes command-palette actions for check/build/run.

### Modified Capabilities
- `editor-tooling`: Removes the blanket statement that the editor SHALL NOT show diagnostics and adds requirements for compiler-driven diagnostics, command palette integration, and configuration.

## Impact

- `editors/vscode/package.json`: adds `activationEvents`, `contributes.commands`, `contributes.configuration`, and default keybinding placeholders.
- `editors/vscode/extension.js`: adds a diagnostics provider, command handlers, executable discovery, status bar, and output channel.
- `editors/vscode/README.md`: documents setup, configuration, and the Phase 9 LSP migration path.
- `openspec/specs/editor-tooling/spec.md`: updated with the new requirements.
- `openspec/specs/editor-cli-bridge/spec.md`: new capability spec.
- No compiler crate is changed in Phase 1. A later optimization may add `--stdin` support to `zirk-check` to avoid disk writes during live debounced checks.
- The `../zirk-lang-site` companion repository should be reviewed; if it surfaces `editor-tooling` requirements or the extension README, run `scripts/sync-website-content.sh` after the change is committed.
