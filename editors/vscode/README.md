# Zirk Language Support

VS Code extension offering syntax highlighting, diagnostics, and language configuration for the Zirk programming language.

When the `zirk-lsp` language server is available, live diagnostics, hover, document symbols, and formatting are served in process by the server and the CLI bridge stays silent. Without the server, all commands, diagnostics, hover, and formatting continue to work through the CLI bridge. The CLI bridge commands remain available regardless.

## Features

- **Language Server (LSP):** When `zirk-lsp` is running, the extension gets live in-memory diagnostics (including unsaved text), type-aware hover, document symbols, and canonical formatting from the server instead of the CLI bridge. Autocomplete, go-to-definition, find references, and rename remain Phase 9 work.

- **Full Syntax Highlighting:** Covers the entire Zirk language specification including keywords, operators, range operators (`..`, `..=`), braced dynamic range operands (`{start}..{end}:{step}`) with normal embedded-expression highlighting, collection/index brackets, fixed array types (`T[n]`), decorators (`@test`, `@e2e`, `@bench`), variables, types, and strings (with escape sequence support and string interpolation).
- **Compiler Diagnostics:** `zirk-check` runs on open, on save, and live while typing (debounced), and errors/warnings are shown as inline diagnostics with the compiler code, cause, and help text. With `zirk-lsp` running, diagnostics come from the server instead and cover unsaved edits.
- **Hover Types:** Hovering a function, method, variable, parameter, field, or expression shows the type the compiler resolved for it. Inferred types (a `fn` with no return annotation, a `mut x = ...` binding) are marked *(inferred)*. Served by `zirk-lsp` when available, otherwise read from `zirk check --types`.
- **Document Symbols:** With `zirk-lsp`, functions, methods, classes, structs, traits, enums, and constants appear in the outline and breadcrumb views.
- **Formatting:** With `zirk-lsp`, `Format Document` applies the compiler's canonical formatting. Without it, the extension delegates to `zirk fmt` when available and falls back to a provisional whitespace-formatting pass.
- **Quick Fixes:** Code actions rename deprecated type spellings (`BinaryFloat*` → `Float*`, `Decimal16`/`32`/`64`/`128` → `Decimal`) in place.
- **Snippets:** Built-in snippets for `fn`, `if`/`else`, `for`, `class`, and `import`.
- **Check Command:** Run `Zirk: Check File` from the command palette to re-check the active file.
- **Run Button:** A ▶ play button in the editor title bar (like the Python extension) saves the file and runs `zirk run <file>` in an integrated `Zirk` terminal, so interactive programs work normally.
- **Build & Run Commands:** Run `Zirk: Build File` and `Zirk: Run File` from the command palette. Build output streams to the `Zirk` output channel; run uses the integrated terminal.
- **Status Bar:** Shows the latest check result for the active `.zrk` file. Click it to re-check.
- **Task Provider:** Provides `zirk: build` and `zirk: run` tasks that can be invoked from the `Tasks: Run Task` command or configured in `.vscode/tasks.json`.
- **Auto-Closing Pairs:** Automatic completion of curly braces `{}`, square brackets `[]`, parentheses `()`, and quotation marks `""`.
- **Comment Toggling:** Standard toggle for line comments (`//`) and block comments (`/* ... */`).
- **Init File Icon:** Files named `.zkinit` (or ending in `.zkinit`) use the dedicated `zirk-init` icon (Zirk mark with a configuration gear) while keeping full Zirk syntax highlighting. They are a separate `zirk-init` language surface, so compile/check commands do not run on them.

## Installation for Local Testing

To test the extension locally in VS Code, Cursor, Windsurf, or any other VS Code fork:

### Option A: Install via symlink (recommended for development)

1. Create a symlink to the `editors/vscode` directory inside your VS Code extension folder:
   ```bash
   ln -s "/Users/cristian/Projects/PROPIOS/zirk-lang/editors/vscode" ~/.vscode/extensions/zirk-lang
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

## Configuration

The extension reads two VS Code settings:

- `zirk.executablePath` — full path to the `zirk` or `zirk-check` executable. Leave empty to let the extension search:
  1. `target/debug/zirk-check`
  2. `target/release/zirk-check`
  3. `target/debug/zirk`
  4. `target/release/zirk`
  5. `zirk-check` or `zirk` on `PATH`
- `zirk.checkOnSave` — run `zirk-check` automatically when a `.zrk` file is saved (default: `true`).
- `zirk.checkOnType` — run `zirk-check` live while typing (default: `true`). The extension writes the current buffer to a hidden temporary `.zrk` next to the file so relative imports keep working, and removes it after each check.
- `zirk.checkDelay` — debounce delay in milliseconds for live checking (default: `500`).
- `zirk.lspPath` — full path to the `zirk-lsp` language server executable. Leave empty to let the extension search `target/debug/zirk-lsp`, `target/release/zirk-lsp`, and `PATH` from the same roots as `zirk.executablePath`. Build it with `cargo build -p zirk-lsp`.

`zirk-check` is the frontend-only validator and does not require LLVM. If you need build/run commands later, build the full `zirk` binary with the `backend` feature enabled.

## Tasks

The extension contributes `zirk` tasks. You can run them through `Tasks: Run Task` or add them to `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "type": "zirk",
      "task": "build",
      "label": "Zirk: Build current file",
      "problemMatcher": []
    },
    {
      "type": "zirk",
      "task": "run",
      "label": "Zirk: Run current file",
      "problemMatcher": []
    }
  ]
}
```

## About Formatting

The extension tries to delegate formatting to `zirk format` first. That subcommand is specified but scheduled for a later phase, so today it exits with an error and the extension falls back to a basic, built-in formatter that only aligns indentation by following curly braces and square brackets. Once `zirk format` lands, delegation kicks in automatically.

It is important to know how it differs from the upcoming official formatter:

- `ZIRK_COMPILER_SPEC.md` section 10 defines the official formatter as **canonical, idempotent, and configuration-free to prevent style fragmentation**. The formatter in this extension respects the editor's `tabSize` and `insertSpaces`, which is exactly the type of configuration the spec discards.
- It is not aware of strings or comments: a line ending in `{` inside a comment will shift the indentation of subsequent lines.

When `zirk format` is implemented, this extension must delegate formatting to it, and this temporary formatter will be retired. Until then, it is a provisional convenience, not the official style definition of Zirk.

## Keywords

The highlighter covers the **entire language** defined by the specifications, not just the subset currently implemented by the compiler. This is deliberate: the editor displays the language as specified, and it is the compiler's responsibility to report which constructs are not yet available and in which phase they will arrive.
