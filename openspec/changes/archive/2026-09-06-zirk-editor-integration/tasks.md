## 1. Phase 1 — Diagnostics bridge

- [x] 1.1 Add `activationEvents` and `contributes.configuration`/`contributes.commands` to `editors/vscode/package.json`
- [x] 1.2 Implement executable discovery in `editors/vscode/extension.js` (`zirk.executablePath` > workspace `target/debug/zirk-check` > `target/release/zirk-check` > PATH `zirk`)
- [x] 1.3 Implement `runCheck(document)` that invokes the CLI with `--json`, parses the JSON from stderr, and clears/sets diagnostics
- [x] 1.4 Map JSON diagnostics to `vscode.Diagnostic` ranges, handling 1-based columns and missing locations
- [x] 1.5 Wire `onDidOpenTextDocument`, `onDidSaveTextDocument`, and `onDidCloseTextDocument` to update diagnostics
- [x] 1.6 Add a `Zirk` output channel and log check invocations and raw compiler output

## 2. Phase 2 — Commands and status bar

- [x] 2.1 Register `Zirk: Check File`, `Zirk: Build File`, and `Zirk: Run File` commands
- [x] 2.2 Implement build/run command handlers using the full `zirk` CLI and stream output to the `Zirk` channel
- [x] 2.3 Add a status bar item that shows the latest check result and a click action to re-check
- [x] 2.4 Add a task provider so `tasks.json` can invoke `zirk build`/`zirk run`

## 3. Phase 3 — Quality of life

- [x] 3.1 Add code actions that convert deprecated type names (`BinaryFloat*`, `Decimal*`) to the new names
- [x] 3.2 Add basic snippets for `fn`, `if/else`, `for`, `class`, and `import`
- [x] 3.3 Support live debounced checks, either by writing a temporary `.zrk` next to the original or by adding `--stdin` support to `zirk-check`
- [x] 3.4 Prepare the formatter provider to delegate to `zirk format` when the binary is available

## 4. Verification and documentation

- [x] 4.1 Test the extension locally by symlinking `editors/vscode` to the VS Code extensions folder
- [x] 4.2 Verify diagnostics appear for `crates/zirk-cli/tests/corpus/invalid/type_mismatch.zrk`
- [x] 4.3 Verify build/run commands work on a valid `.zrk` file with the `backend` feature enabled
- [x] 4.4 Update `editors/vscode/README.md` with setup and configuration instructions
