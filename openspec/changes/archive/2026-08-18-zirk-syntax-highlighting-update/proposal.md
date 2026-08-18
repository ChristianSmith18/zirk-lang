## Why

The current syntax highlighting definition for Zirk-lang in editor extensions is outdated. It lacks support for Zirk 1.x features such as structured concurrency keywords, float widths, temporal types, synchronization primitives, named parameters shorthand, and distinct coloring for native vs. user module imports. Updating it ensures that developers have a modern, correct, and premium editor experience.

## What Changes

- **Keywords**: Add missing control-flow and structured concurrency keywords (`do`, `yield`, `throw`, `throws`, `select`, `scope`, `shield`, `cancelled`, `timeout`, `cancellation`).
- **Modifiers**: Add missing declarations, class modifiers, and manifest keywords (`strict`, `override`, `extends`, `super`, `commit`, `repeatable`, `project`, `build_targets`, `globals`, `permissions`, `requires`, `during`, `build`, `runtime`, `both`).
- **Imports**: Highlight standard library imports (`std.*`) with a special namespace token (`support.other.namespace.zirk`) and local/package paths as normal strings.
- **Arguments**: Highlight named arguments (`host:`) and shorthand named arguments (`timeout:`) within parameter/argument list contexts.
- **Formatting**: Support string format specifiers and placeholders (e.g. `:name`, `:id|08`, escaped `\:`).
- **Literals**: Highlight regex literals (`re'pattern'`) and char literals (`'grapheme'`).
- **Operators**: Add operators `is`, `|>`, `**`, `=>`, `->`, `~` and remove the obsolete `as` operator.
- **Built-in Types**: Remove obsolete `Integer` and `Decimal*` families; add `Float*` and `UInt`, standard collections, concurrency, and sync primitive types, as well as `Result`, `Option`, and `Throwable`.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `editor-tooling`: Update editor/VSCode integration to support Zirk 1.x syntax specification.

## Impact

- `editors/vscode/syntaxes/zirk.tmLanguage.json`: Highlighting rules will be updated.
- Editor experience for developers writing Zirk code (correct syntax styling and identification of errors).
