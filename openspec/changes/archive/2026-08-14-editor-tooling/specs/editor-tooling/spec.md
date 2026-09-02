## ADDED Requirements

### Requirement: Source file recognition

Editor support SHALL be associated with the `.zrk` extension defined by `ZIRK_SPEC_FINAL.md`.

#### Scenario: Opening a Zirk file
- **WHEN** a file with extension `.zrk` is opened
- **THEN** the editor recognizes it as the Zirk language

### Requirement: Complete lexicon coverage

Highlighting SHALL cover **all** keywords recognized by the lexer, not only those of the implemented subset.

Covering the whole language is deliberate: the editor shows the language as the specs define it, and it is the compiler that indicates which construct is not available yet and in which phase it arrives.

#### Scenario: Keyword of the implemented subset
- **WHEN** the source contains `fn`, `mut`, `inmut`, `if`, `else` or `return`
- **THEN** they are highlighted as keywords

#### Scenario: Keyword of a later phase
- **WHEN** the source contains `class`, `match`, `task`, `parallel` or another not-yet-implemented construct
- **THEN** they are highlighted the same as subset keywords

#### Scenario: Correspondence with the lexer
- **WHEN** the highlighter's keywords are compared with those of `zirk-lexer`
- **THEN** every keyword the lexer recognizes is covered by the highlighter

### Requirement: Distinction by role

Highlighting SHALL distinguish lexical categories from one another: control flow, modifiers, types, literals, operators and comments.

#### Scenario: Differentiated categories
- **WHEN** a file with constructs from several categories is highlighted
- **THEN** control flow, modifiers and types receive distinct scopes

#### Scenario: The current instance is not control flow
- **WHEN** the source contains `this`
- **THEN** it is highlighted as a reference to the current value, not as a control-flow word

### Requirement: Language configuration

The editor SHALL know the comment delimiters and the opening/closing pairs of the language.

#### Scenario: Commenting a selection
- **WHEN** the comment command is used
- **THEN** the delimiters `//` or `/* */` from `ZIRK_LANGUAGE_SPEC.md` section 1 are inserted

#### Scenario: Auto-closing
- **WHEN** `{`, `(`, `[` or a quote is typed
- **THEN** the editor offers the corresponding closing character

### Requirement: The extension formatter is provisional

Until `zirk format` exists, editor support MAY offer its own formatter, which SHALL NOT be considered the definition of Zirk style.

`ZIRK_COMPILER_SPEC.md` section 10 defines the official formatter as canonical, idempotent and without configuration that fragments style. An editor formatter that respects user configuration does not meet that contract.

#### Scenario: Documented provisional nature
- **WHEN** the extension documentation is consulted
- **THEN** it indicates that its formatter is provisional and that the canonical one arrives with `zirk format`

#### Scenario: Arrival of the canonical formatter
- **WHEN** `zirk format` is available
- **THEN** the extension SHALL delegate to it and remove its own formatter

### Requirement: Scope without language server

This capability SHALL NOT include diagnostics inside the editor, autocomplete, navigation or renaming.

All of that requires the LSP over an incremental frontend, which `ZIRK_COMPILER_SPEC.md` section 10 defines and the roadmap places in Phase 9.

#### Scenario: Diagnostics during editing
- **WHEN** code with a type error is written
- **THEN** the editor does not flag it by itself
- **AND** the error appears when compiling with `zirk build` or `zirk run`
