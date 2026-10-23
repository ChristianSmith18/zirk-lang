# editor-tooling

## Purpose

Define editor support for writing Zirk: recognition of `.zrk` files, syntax highlighting, and language configuration.

The highlighter covers the full language defined by the specs, not just the subset the compiler implements. The split is deliberate: **the editor shows the language; the compiler says what is available.**

It does not include a language server —autocomplete, navigation, renaming—, which require the incremental frontend and belong to Phase 9. In the meantime, compiler diagnostics reach the editor through the `editor-cli-bridge`.

## Requirements

### Requirement: Source file recognition

Editor support SHALL be associated with the `.zrk` extension and with `.zkinit` project-initialization files. `.zkinit` files SHALL be a distinct editor language surface (`zirk-init`) so they carry their own file icon, while reusing the Zirk grammar and language configuration.

#### Scenario: Opening a Zirk file
- **WHEN** a file with the `.zrk` extension is opened
- **THEN** the editor recognizes it as the Zirk language

#### Scenario: Opening the project manifest
- **WHEN** the `.zkinit` file is opened
- **THEN** the editor recognizes it as the Zirk language

#### Scenario: Opening a `.zkinit` file
- **WHEN** a file named `.zkinit` is opened
- **THEN** the editor recognizes it as the Zirk-init surface with the dedicated `zirk-init` icon and Zirk syntax highlighting

### Requirement: Full lexicon coverage

Highlighting SHALL cover **all** keywords recognized by the Phase 3 lexer, not only those of the implemented subset. This includes keywords for structured concurrency, pattern matching with resources, decoration modifiers, and manifest configuration.

Covering the full language is deliberate: the editor shows the language as defined by the specs, and it is the compiler that indicates which construct is not yet available and in which phase it arrives.

#### Scenario: Keyword from the implemented subset
- **WHEN** the source contains `fn`, `mut`, `inmut`, `if`, `else`, or `return`
- **THEN** they are highlighted as keywords

#### Scenario: Concurrency keyword and match with resources
- **WHEN** the source contains `task`, `await`, `select`, `parallel`, `thread`, `match`, `with`, `do`, `yield`, `throw`, `throws`, or `commit`
- **THEN** they are highlighted as control or storage keywords

#### Scenario: .zkinit manifest keyword
- **WHEN** the source contains `project`, `build_targets`, `globals`, `permissions`, `requires`, or `during`
- **THEN** they are highlighted as keywords or structural blocks

#### Scenario: Correspondence with the lexer
- **WHEN** the highlighter's keywords are compared against those of `zirk-lexer`
- **THEN** every word the lexer recognizes is covered by the highlighter

### Requirement: Distinction by role

Highlighting SHALL distinguish the lexical categories from one another: control flow, modifiers, types (including floats and temporal types), literals (including Unicode characters and regular expressions), operators (including `is` and the pipe `|>`), format markers within strings, and comments.

#### Scenario: Differentiated categories
- **WHEN** a file with constructs from several categories is highlighted
- **THEN** control flow, modifiers, and types receive distinct scopes

#### Scenario: The current instance is not control flow
- **WHEN** the source contains `this`
- **THEN** it is highlighted as a reference to the current value, not as a control keyword

#### Scenario: Format marker within a string
- **WHEN** the source contains a string with `:variable|modifier`
- **THEN** the colon, the placeholder name, and the format modifier are distinguished from the normal text of the string

#### Scenario: Native and user library imports
- **WHEN** the source contains `import { stdin } from std.io;` and `import { User } from "./user";`
- **THEN** `std.io` is highlighted as a native namespace
- **AND** `from` is highlighted as control
- **AND** `"./user"` is highlighted as literal text

#### Scenario: Named arguments and shorthand
- **WHEN** the source contains `host: "localhost"` or the shorthand `timeout:`
- **THEN** the identifiers before the colon are highlighted as call arguments

### Requirement: Language configuration

The editor SHALL know the language's comment delimiters and opening/closing pairs, including `'''…'''` triple-quoted delimiters.

#### Scenario: Commenting a selection
- **WHEN** the comment command is used
- **THEN** the `//` or `/* */` delimiters from `ZIRK_LANGUAGE_SPEC.md` section 1 are inserted

#### Scenario: Auto-closing
- **WHEN** `{`, `(`, `[`, or a quote is typed
- **THEN** the editor offers the corresponding closing character

#### Scenario: Triple-quote highlighting
- **WHEN** a `'''…'''` literal is opened
- **THEN** its content is scoped as a string and `{expr}` retains expression scopes

### Requirement: The extension's formatter is provisional

The extension formatter MUST remain provisional while `zirk format` does not cover the full style surface and MUST NOT be presented as the definition of Zirk style.

`ZIRK_COMPILER_SPEC.md` section 10 defines the official formatter as canonical, idempotent, and configuration-free. For triple-quoted layout, the extension SHALL delegate to `zirk format` when it is available.

#### Scenario: Provisional nature documented
- **WHEN** the extension's documentation is consulted
- **THEN** it states that its formatter is provisional and that the canonical one is `zirk format`

#### Scenario: Delegation for delivered rules
- **WHEN** the extension formats a file containing `'''` literals and `zirk format` is available
- **THEN** `zirk format` produces the triple-quoted layout

### Requirement: Scope without a language server

The editor extension SHALL use `zirk-lsp` for compiler-backed semantic features when the server is available, including diagnostics, hover, symbols, formatting, navigation, references, rename, completion, signature help, semantic tokens, inlay hints, workspace symbols, and code actions as advertised by the server. When the server is unavailable, the `editor-cli-bridge` SHALL provide only the fallback features it can implement correctly: compiler diagnostics, type hover, formatting, and explicit check/build/run commands. The extension SHALL show whether the LSP is active or fallback mode is active and SHALL NOT imply that unsupported semantic features are available.

#### Scenario: Diagnostics while editing
- **WHEN** code with a type error is written in a `.zrk` file
- **THEN** the editor flags it through the language server when available, or through the CLI bridge otherwise
- **AND** the diagnostic is shown in the editor

#### Scenario: Hover shows the resolved type
- **WHEN** the user hovers over a declaration name or an expression
- **THEN** the editor shows the type the compiler resolved for it, including inferred types marked as such

#### Scenario: Features that still require an LSP
- **WHEN** the user requests autocomplete, go-to-definition, or rename
- **THEN** the editor provides them when the corresponding LSP capability is active
- **AND** it does not claim they are available through the CLI-only fallback

#### Scenario: Language server available
- **WHEN** `zirk-lsp` starts successfully for a `.zrk` workspace
- **THEN** the extension routes semantic requests through the server
- **AND** shows the active server state

#### Scenario: Language server unavailable
- **WHEN** `zirk-lsp` cannot be located or fails to start
- **THEN** the extension uses the CLI bridge for its supported fallback features
- **AND** identifies unavailable semantic navigation and completion features in the language status or output

#### Scenario: No duplicate feature providers
- **WHEN** the language server is active for a document
- **THEN** the CLI bridge does not publish competing diagnostics, hover, or formatting results for that document

#### Scenario: Semantic highlighting falls back
- **WHEN** semantic tokens are missing or temporarily unavailable
- **THEN** the Zirk syntax grammar continues to provide lexical highlighting

### Requirement: Triple-quote edge guide

The extension SHALL draw a column-level guide connecting paired `'''` delimiters at their shared column, independently of optional editor settings.

#### Scenario: Guide tracks edits
- **WHEN** delimiters move or lines are inserted inside a triple-quoted literal
- **THEN** the guide repaints at the current edge

### Requirement: Command palette integration
The editor SHALL provide commands to check, build, and run the current `.zrk` file through the `editor-cli-bridge`.

#### Scenario: Running Zirk commands from the editor
- **WHEN** the user invokes `Zirk: Check File`, `Zirk: Build File`, or `Zirk: Run File`
- **THEN** the editor invokes the appropriate CLI command
- **AND** it displays the result in an output channel or status bar

### Requirement: Debugger integration

The editor SHALL expose Zirk debugging through a DAP adapter when a supported execution target is configured. The debug UI SHALL report target capability limits and SHALL distinguish runtime values from compile-time type information.

#### Scenario: Debug a supported interpreter program
- **WHEN** a supported interpreter launch configuration starts a debug session
- **THEN** the editor exposes verified breakpoints, stepping, stack frames, scopes, and runtime variables supported by that target

#### Scenario: Unsupported debug target
- **WHEN** the selected target does not support a requested debug operation
- **THEN** the editor communicates the limitation and does not display fabricated values
