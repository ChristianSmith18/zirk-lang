# Zirk Language Support

VS Code extension offering syntax highlighting and basic language configuration for the Zirk programming language.

## Features

- **Full Syntax Highlighting:** Covers the entire Zirk language specification including keywords, operators, decorators (`@test`, `@e2e`, `@bench`), variables, types, and strings (with escape sequence support and string interpolation).
- **Auto-Closing Pairs:** Automatic completion of curly braces `{}`, square brackets `[]`, parentheses `()`, and quotation marks `""`.
- **Comment Toggling:** Standard toggle for line comments (`//`) and block comments (`/* ... */`).

## Installation for Local Testing

To test the extension locally in VS Code, Cursor, Windsurf, or any other VS Code fork:

### Option A: Install via symlink (recommended for development)

1. Create a symlink to the `editors/vscode` directory inside your VS Code extension folder:
   ```bash
   ln -s "/Users/cristian/Projects/PROPIOS/zirk-lang-syntax/editors/vscode" ~/.vscode/extensions/zirk-lang
   ```
2. Restart your editor (VS Code, Cursor, Windsurf, etc.).
3. Open any file with `.zrk` extension (like the corpus examples).

### Option B: Build a portable `.vsix` package

1. Install the VS Code extension manager globally:
   ```bash
   npm install -g @vscode/vsce
   ```
2. Run the packaging command from the extension folder (`editors/vscode`):
   ```bash
   cd editors/vscode
   vsce package
   ```
3. Install the generated `.vsix` file in your editor via the command palette (`Developer: Install Extension from VSIX...`).

## About Formatting

The extension includes a basic, built-in formatter that only aligns indentation by following curly braces and square brackets. It exists because **there is no `zirk format` yet**: that command is scheduled for Phase 9 of the roadmap.

It is important to know how it differs from the upcoming official formatter:

- `ZIRK_COMPILER_SPEC.md` section 10 defines the official formatter as **canonical, idempotent, and configuration-free to prevent style fragmentation**. The formatter in this extension respects the editor's `tabSize` and `insertSpaces`, which is exactly the type of configuration the spec discards.
- It is not aware of strings or comments: a line ending in `{` inside a comment will shift the indentation of subsequent lines.

When `zirk format` is implemented, this extension must delegate formatting to it, and this temporary formatter will be retired. Until then, it is a provisional convenience, not the official style definition of Zirk.

## Keywords

The highlighter covers the **entire language** defined by the specifications, not just the subset currently implemented by the compiler. This is deliberate: the editor displays the language as specified, and it is the compiler's responsibility to report which constructs are not yet available and in which phase they will arrive.
