## Context

The Zirk VSCode extension's syntax highlighting is defined in [zirk.tmLanguage.json](file:///Users/cristian/Projects/PROPIOS/zirk-lang/editors/vscode/syntaxes/zirk.tmLanguage.json). The language has evolved with new structured concurrency, decorators, pattern matching, imports, and string formatting constructs. This design updates the TextMate grammar to achieve full coverage and correct classification.

## Goals / Non-Goals

**Goals:**
- Update keywords, modifiers, types, literals, and operators in `zirk.tmLanguage.json` to cover Zirk 1.x specs.
- Distinguish standard library namespaces (`std.*`) from user-defined local paths inside `import` statements.
- Correctly highlight named parameters and shorthand argument colons (`variable:`).
- Highlight string format placeholders and specifiers (e.g., `:variable|modifier`) and support escaped colons (`\:`).
- Package the updated extension into a `.vsix` file and install it in the current editor (agy-ide) to verify functionality.
- Perform all work in the isolated `editors-worktree` git worktree.

**Non-Goals:**
- Implement language server protocol (LSP) features, diagnostics, auto-complete, or refactor tooling.
- Modify the compiler or runtime implementation.

## Decisions

### Decision 1: TextMate Grammar Scopes
We will use VSCode/TextMate standard scopes to ensure compatible syntax coloring across different editor themes:
- Named arguments: `variable.parameter.argument.zirk`
- Standard library namespace: `support.other.namespace.zirk`
- Regex literal: `string.regexp.zirk`
- Char literal: `constant.character.zirk`
- Format placeholder: `variable.other.placeholder.zirk`
- Format specifier: `constant.other.format-specifier.zirk`

### Decision 2: Named Parameter & Shorthand Matching
- **Shorthand arguments** like `timeout:` are matched using positive lookahead for a comma or closing paren: `\b([a-zA-Z_][a-zA-Z0-9_]*)\s*:(?=\s*[,)])`.
- **Explicit arguments** like `host: "localhost"` are matched using negative lookahead for double colon or type declaration keywords: `\b([a-zA-Z_][a-zA-Z0-9_]*)\s*(:)(?!\s*:)`.

### Decision 3: Quoted vs. Unquoted Imports
Inside `import` statements, we distinguish standard library modules by matching `std.` namespaces unquoted, whereas local paths match standard string rules. Standard modules are matched inside the `from` block using `\bstd\.[a-zA-Z_][a-zA-Z0-9_]*(\.[a-zA-Z_][a-zA-Z0-9_]*)*\b`.

### Decision 4: String Formatting Specifiers
Within double-quoted strings, format placeholders are matched using:
`(?<!\\)(:)([a-zA-Z_][a-zA-Z0-9_]*|[0-9]+)(?:(\\|)([a-zA-Z0-9_\-\.\#\*\+\%]+))?`
Escaped colons `\:` are added to the double-quoted string escape patterns to prevent false matches.

## Risks / Trade-offs

- **[Risk]** Loose named-argument regexes matching type annotations.
  - *Mitigation*: Ensure keywords and type definitions are evaluated first in the patterns array.
- **[Risk]** Format specifiers matching URLs or file paths in normal strings (e.g. `http://...`).
  - *Mitigation*: The lookbehind prevents matching escaped colons, and the placeholder identifier pattern excludes slashes and normal punctuation.
- **[Risk]** Broken editor behavior during extension loading/testing.
  - *Mitigation*: Work entirely in `editors-worktree` and build a package (`.vsix`) for targeted installation rather than running live extensions in debug mode.
