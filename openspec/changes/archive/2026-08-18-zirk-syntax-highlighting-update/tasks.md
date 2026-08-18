## 1. Setup and Project Configuration

- [x] 1.1 Ensure working directory is within `editors-worktree` branch to isolate changes.
- [x] 1.2 Associate `init.zrk` file mapping in `editors/vscode/package.json` so project manifests are highlighted correctly.

## 2. Syntax Highlighting Implementation

- [x] 2.1 Implement format placeholders (`:variable|modifier` and `\:` escapes) inside string patterns in `editors/vscode/syntaxes/zirk.tmLanguage.json`.
- [x] 2.2 Add standard namespaces (`std.*`) and arrow `->` alias highlights to module imports block in `editors/vscode/syntaxes/zirk.tmLanguage.json`.
- [x] 2.3 Implement named arguments and shorthand named arguments (`variable:`) rules in `editors/vscode/syntaxes/zirk.tmLanguage.json`.
- [x] 2.4 Update keywords control list and storage modifiers in `editors/vscode/syntaxes/zirk.tmLanguage.json` with Zirk 1.x keywords (such as `throw`, `select`, `commit`, etc.).
- [x] 2.5 Add regex literals (`re'...'`) and char literals (`'...'`) to `editors/vscode/syntaxes/zirk.tmLanguage.json`.
- [x] 2.6 Refine operator matching rules (remove `as`, add `is`, `|>`, `**`, `=>`, `->`, `~`).
- [x] 2.7 Update primitive types list (remove `Integer`/`Decimal*`, add `Float*`, temporal types, collection types, synchronization primitives).

## 3. Packaging and Verification

- [x] 3.1 Package the updated VSCode extension using `npx vsce package` to produce a `.vsix` bundle in `editors-worktree`.
- [x] 3.2 Install the `.vsix` bundle in the local editor (agy-ide).
- [x] 3.3 Verify syntax highlighting on existing Zirk source files (`.zrk` and `init.zrk`).
