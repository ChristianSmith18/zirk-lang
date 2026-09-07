# Tasks: `.zkinit` file icon

## 1. Assets

- [x] 1.1 Copy `docs/assets/zirk-init.svg` to `editors/vscode/images/zirk-init-gear.svg`.

## 2. Extension contributions

- [x] 2.1 Add a `zirk-init` language contribution in `editors/vscode/package.json` (`aliases`, `filenames: [".zkinit"]`, optional `extensions: [".zkinit"]`, same `language-configuration.json`, `icon` → `images/zirk-init-gear.svg` for light and dark).
- [x] 2.2 Add a second `contributes.grammars` entry mapping `zirk-init` to `./syntaxes/zirk.tmLanguage.json` (scope `source.zirk`).
- [x] 2.3 Add the existing `snippets/zirk.json` to the `zirk-init` language if appropriate.

## 3. Packaging and install

- [x] 3.1 Validate `editors/vscode/package.json` (JSON parse).
- [x] 3.2 Copy updated `package.json`, grammar reference, and `images/zirk-init-gear.svg` to `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/`.
- [x] 3.3 Rebuild the `.vsix` if that is part of the release flow.

## 4. Verification

- [x] 4.1 Reload VS Code and confirm a `.zkinit` file shows the `zirk-init` gear icon and Zirk highlighting.
- [x] 4.2 Confirm `.zrk` files keep `logo.svg` and are unchanged.
- [x] 4.3 Confirm `Zirk: Check/Build/Run` do not run on `.zkinit` documents.

## 5. Documentation

- [x] 5.1 Mention the `.zkinit` icon/recognition in `editors/vscode/README.md` or `docs/SYNTAX_HIGHLIGHTING.md` if appropriate.
- [x] 5.2 Sync `openspec/specs/editor-tooling` (MODIFIED: Source file recognition) and create `openspec/specs/zkinit-editor-recognition` at archive time.
