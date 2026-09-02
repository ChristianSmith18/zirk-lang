## 1. VS Code Extension

- [x] 1.1 Create the structure in `editors/vscode/` with its manifest
- [x] 1.2 Associate the `.zrk` extension with the Zirk language
- [x] 1.3 Define the highlighting grammar in `syntaxes/zirk.tmLanguage.json`
- [x] 1.4 Define comments, closing pairs and folding in `language-configuration.json`

## 2. Lexicon Coverage

- [x] 2.1 Cover the keywords of the implemented subset
- [x] 2.2 Cover the keywords of later phases
- [x] 2.3 Verify by cross-comparison against `zirk-lexer` that coverage is complete
- [x] 2.4 Highlight `this` as a reference to the current value and not as control flow

## 3. Provisional Formatter

- [x] 3.1 Implement the indentation-based format provider
- [x] 3.2 Document where it diverges from what `COMPILER_SPEC` section 10 requires
- [x] 3.3 Record that it must delegate to `zirk format` when it exists

## 4. Wrap-up

- [x] 4.1 Verify that the configuration files are valid JSON
- [x] 4.2 Confirm that no compiler crate is touched
