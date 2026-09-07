# editor-tooling

## Purpose

Define editor support for writing Zirk: recognition of `.zrk` files, syntax highlighting, and language configuration.

The highlighter covers the full language defined by the specs, not just the subset the compiler implements. The split is deliberate: **the editor shows the language; the compiler says what is available.**

It does not include a language server —autocomplete, navigation, renaming—, which require the incremental frontend and belong to Phase 9. In the meantime, compiler diagnostics reach the editor through the `editor-cli-bridge`.

## Requirements

### Requirement: Source file recognition

Editor support SHALL be associated with the `.zrk` extension, with `init.zrk` manifest files, and with `.zkinit` project-initialization files. `.zkinit` files SHALL be a distinct editor language surface (`zirk-init`) so they carry their own file icon, while reusing the Zirk grammar and language configuration.

#### Scenario: Opening a Zirk file
- **WHEN** a file with the `.zrk` extension is opened
- **THEN** the editor recognizes it as the Zirk language

#### Scenario: Opening the project manifest
- **WHEN** the `init.zrk` file is opened
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

#### Scenario: init.zrk manifest keyword
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

The editor SHALL know the language's comment delimiters and its opening/closing pairs.

#### Scenario: Commenting a selection
- **WHEN** the comment command is used
- **THEN** the `//` or `/* */` delimiters from `ZIRK_LANGUAGE_SPEC.md` section 1 are inserted

#### Scenario: Auto-closing
- **WHEN** `{`, `(`, `[`, or a quote is typed
- **THEN** the editor offers the corresponding closing character

### Requirement: The extension's formatter is provisional

While `zirk format` does not exist, editor support MAY offer its own formatter, which SHALL NOT be considered the definition of Zirk's style.

`ZIRK_COMPILER_SPEC.md` section 10 defines the official formatter as canonical, idempotent, and free of configuration that would fragment the style. An editor formatter that respects user configuration does not meet that contract.

#### Scenario: Provisional nature documented
- **WHEN** the extension's documentation is consulted
- **THEN** it states that its formatter is provisional and that the canonical one arrives with `zirk format`

#### Scenario: Arrival of the canonical formatter
- **WHEN** `zirk format` becomes available
- **THEN** the extension SHALL delegate to it and retire its own formatter

### Requirement: Scope without a language server

This capability SHALL NOT include autocomplete, hover, navigation, or renaming provided by a language server. Compiler diagnostics SHALL be shown through the `editor-cli-bridge` while a full LSP is not available.

#### Scenario: Diagnostics while editing
- **WHEN** code with a type error is written and saved
- **THEN** the editor flags it by invoking the CLI bridge
- **AND** the diagnostic is shown in the editor

#### Scenario: Features that still require an LSP
- **WHEN** the user requests autocomplete, hover, go-to-definition, or rename
- **THEN** the editor does not provide them
- **AND** it documents that they arrive with the LSP in Phase 9

### Requirement: Command palette integration
The editor SHALL provide commands to check, build, and run the current `.zrk` file through the `editor-cli-bridge`.

#### Scenario: Running Zirk commands from the editor
- **WHEN** the user invokes `Zirk: Check File`, `Zirk: Build File`, or `Zirk: Run File`
- **THEN** the editor invokes the appropriate CLI command
- **AND** it displays the result in an output channel or status bar
