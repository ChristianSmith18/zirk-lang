# Zirk Language Support

VS Code extension offering syntax highlighting, diagnostics, and language configuration for the Zirk programming language.

When the `zirk-lsp` language server is available, it provides live diagnostics, hover, document and workspace symbols, formatting, definition and type definition, references, document highlights, safe rename, contextual completion, signature help, inferred-type inlay hints, and semantic tokens. The CLI bridge stays silent for overlapping providers. Without the server, commands, diagnostics, hover, and formatting continue to use the CLI bridge; semantic navigation and authoring features require the language server.

## Features

- **Language Server (LSP):** When `zirk-lsp` is running, the extension gets live in-memory diagnostics (including unsaved text), type-aware hover, document and workspace symbols, canonical formatting, go-to-definition and type definition, find references, document highlights, alias-aware safe rename, contextual completion, completion detail resolution, signature help with generic substitutions, inferred-type and parameter-name hints, diagnostic-backed versioned quick fixes, and compiler-backed semantic tokens. Import completion covers compiler-authorized local, standard-library, and declared dependency paths, and can offer top-level `share` declarations from an unopened module; unresolved imports keep a current partial snapshot so completion works during editing. Focused protocol tests verify these providers against the multi-module fixture.
- **Debugging (DAP):** The interpreter target uses `zirk-dap` to launch Zirk source, verify breakpoints, step, and inspect bounded stack and local snapshots. The experimental native target compiles with `zirk build --debug-info` and uses `lldb-dap` with LLVM DWARF source mappings; on macOS the compiler also creates a `.dSYM` bundle. Native editor acceptance for breakpoints, stepping, stack, and locals is pending. Values unavailable from the interpreter are labeled, and SecretString payloads are redacted. The interpreter accepts only its browser-safe capability subset, so programs that require native host features such as environment access fail closed there. Browser debugging is not provided.

- **Full Syntax Highlighting:** Covers the entire Zirk language specification including keywords, operators, range operators (`..`, `..=`), braced dynamic range operands (`{start}..{end}:{step}`) with normal embedded-expression highlighting, collection/index brackets, fixed array types (`T[n]`), decorators (`@test`, `@e2e`, `@bench`), variables, types, and strings (with escape sequence support and string interpolation).
- **Compiler Diagnostics:** `zirk-check` runs on open, on save, and live while typing (debounced), and errors/warnings are shown as inline diagnostics with the compiler code, cause, and help text. With `zirk-lsp` running, diagnostics come from the server instead and cover unsaved edits.
- **Hover Types:** Hovering a function, method, variable, parameter, field, or expression shows the type the compiler resolved for it. Inferred types (a `fn` with no return annotation, a `mut x = ...` binding) are marked *(inferred)*. Served by `zirk-lsp` when available, otherwise read from `zirk check --types`.
- **Document Symbols:** With `zirk-lsp`, functions, methods, classes, structs, traits, enums, and constants appear in the outline and breadcrumb views.
- **Navigation:** Go to Definition and Find All References follow compiler-resolved identities across files, including aliased imports. Document Highlights distinguish reads from writes.
- **Semantic Highlighting:** The language server classifies resolved functions, methods, types, contracts, parameters, fields, variables, imports, declarations, and standard-library symbols. VS Code overlays these classifications on the TextMate grammar using the Zirk palette as a theme fallback.
- **Formatting:** With `zirk-lsp`, `Format Document` applies the compiler's canonical formatting. Without it, the extension delegates to `zirk fmt` when available and falls back to a provisional whitespace-formatting pass.
- **Quick Fixes:** Code actions rename deprecated type spellings (`BinaryFloat*` → `Float*`, `Decimal16`/`32`/`64`/`128` → `Decimal`) in place.
- **Snippets:** Built-in snippets for `fn`, `if`/`else`, `for`, `class`, and `import`.
- **Check Command:** Run `Zirk: Check File` from the command palette to re-check the active file.
- **Run Button:** A ▶ play button in the editor title bar (like the Python extension) saves the file and runs `zirk run <file>` in an integrated `Zirk` terminal, so interactive programs work normally.
- **Build & Run Commands:** Run `Zirk: Build File` and `Zirk: Run File` from the command palette. Build output streams to the `Zirk` output channel; run uses the integrated terminal.
- **Status Bar:** Shows the latest diagnostic state for the active `.zrk` file; in CLI fallback mode, click it to re-check.
- **Language Service Status:** Shows `Starting`, `Indexing`, `Ready`, `CLI fallback`, or `LSP failed` so it is clear whether semantic editor features are available.
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

### Devin local development install

Devin loads development extensions from its local extensions directory. To test a build across workspaces without relying on this repository being open:

1. Build both native editor services from the repository root:
   ```sh
   cargo build --release -p zirk-lsp -p zirk-dap
   ```
2. Copy the extension files to the versioned Devin extension directory and place the matching services in its `bin` directory:
   ```sh
   extension_dir="$HOME/.devin/extensions/christiansmith.zirk-lang-0.6.0"
   mkdir -p "$extension_dir/bin"
   rsync -a --exclude='*.vsix' --exclude='.DS_Store' editors/vscode/ "$extension_dir/"
   cp target/release/zirk-lsp "$extension_dir/bin/zirk-lsp"
   cp target/release/zirk-dap "$extension_dir/bin/zirk-dap"
   ```
3. Open a clean Devin window. The extension discovers both bundled executables from its own directory. No repository-specific setting is needed; `zirk.lspPath` and `zirk.dapPath` remain available for explicit overrides.

This local install procedure is documented but still needs confirmation in Devin after the app is available.

## Configuration

The extension reads these VS Code settings:

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
- `zirk.dapPath` — full path to the `zirk-dap` interpreter adapter. Leave empty to let the extension search its bundled `bin` directory, the workspace's `target` directories, and `PATH`.
- `zirk.nativeDebuggerPath` — full path to `lldb-dap` or `lldb-vscode`. Leave empty to let the extension use `xcrun --find lldb-dap` on macOS and then search `PATH`.

`zirk-check` is the frontend-only validator and does not require LLVM. If you need build/run commands later, build the full `zirk` binary with the `backend` feature enabled.

For interpreter debugging, build `zirk-dap` with `cargo build -p zirk-dap` or configure `zirk.dapPath`. For native debugging, install Xcode Command Line Tools (macOS) or provide `lldb-dap` on `PATH`, and make sure the `zirk` compiler is discoverable. Use the contributed **Debug Zirk program (interpreter)** or **Debug Zirk program (native)** configuration. The native launch builds the selected source with `--debug-info` before starting LLDB.

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
