# zirk-lsp

## Purpose

Provide a Zirk language server that answers editor requests — diagnostics,
hover, symbols, and formatting — from the compiler's own frontend, running in
process on the text the editor holds rather than on saved files. It is the
deliberately scoped MVP that precedes the full incremental LSP of Phase 9.

## Requirements

### Requirement: Protocol lifecycle

The server SHALL speak the Language Server Protocol over stdio, SHALL advertise only capabilities that are implemented for the active project state, and SHALL exit cleanly on `shutdown` followed by `exit`. Capability registration SHALL reflect support for diagnostics, hover, document symbols, formatting, semantic tokens, definitions, references, highlights, rename, completion, signature help, inlay hints, workspace symbols, and code actions as those features become available.

#### Scenario: Handshake advertises the MVP surface
- **WHEN** an editor client sends `initialize`
- **THEN** the server advertises each implemented capability it can serve for the active workspace
- **AND** it does not advertise a capability whose provider is unavailable

#### Scenario: Client capability negotiation
- **WHEN** a client does not support an optional protocol feature or token type
- **THEN** the server negotiates a compatible response or omits that feature without failing initialization

#### Scenario: Clean shutdown
- **WHEN** the client sends `shutdown` and then `exit`
- **THEN** the server cancels outstanding work, releases workspace resources, and terminates without writing diagnostics or responses afterwards

### Requirement: In-memory document analysis

The server SHALL analyze the text reported by `textDocument/didOpen` and
`textDocument/didChange` directly, without requiring the document to exist on
disk. Unsaved edits SHALL be visible to every request the server answers.

#### Scenario: Hover on unsaved text

- **WHEN** a document was edited in the editor but never saved
- **THEN** hover answers from the in-memory text, not from the file on disk

#### Scenario: Closing a document

- **WHEN** the client sends `textDocument/didClose`
- **THEN** the server drops the document's state and clears its diagnostics

### Requirement: Live diagnostics

The server SHALL publish the compiler's diagnostics for an open document when
it is opened and after its text changes, mapping each diagnostic's span to
LSP positions. Diagnostics whose severity is error SHALL map to an LSP error;
warnings SHALL map to warnings.

#### Scenario: Typing introduces an error

- **WHEN** the user edits a document so that it no longer checks cleanly
- **THEN** the server publishes the new diagnostics for the in-memory text

#### Scenario: Errors are fixed

- **WHEN** the user edits a document so that it checks cleanly
- **THEN** the server publishes an empty diagnostic set for that document

### Requirement: Hover

The server SHALL answer `textDocument/hover` with the type the checker
resolved for the span under the position, formatted as a Zirk code block, with
inferred types marked as inferred. When the position maps to no typed span,
or no analysis result is available for the current text, the server SHALL
return no hover rather than a stale or incorrect one.

#### Scenario: Hover over an inferred function name

- **WHEN** the user hovers over `hola` in `fn hola() { ... }`
- **THEN** the hover shows `fn hola(): Void` marked as inferred

#### Scenario: Hover outside any typed span

- **WHEN** the position maps to no resolved type
- **THEN** the server returns an empty hover result

#### Scenario: Analysis unavailable for current text

- **WHEN** the current text could not be analyzed at all
- **THEN** hover returns no result instead of reporting a type from an older
  snapshot

### Requirement: Document symbols

The server SHALL answer `textDocument/documentSymbol` with the document's
top-level declarations — functions, classes and their members, enums,
contracts, type aliases, and externs — each with its kind and its name and
selection ranges.

#### Scenario: Symbols for a mixed file

- **WHEN** a document declares a function, a class with methods, and an enum
- **THEN** the symbol list contains an entry for each, with the class members
  nested under the class

### Requirement: Formatting

The server SHALL answer `textDocument/formatting` with edits equivalent to
`zirk format` applied to the in-memory text — the full canonical document
formatting, not only triple-quoted-string layout. When the text cannot be
formatted, the server SHALL return no edits.

#### Scenario: Formatting normalizes layout

- **WHEN** the user requests formatting on a document whose layout
  deviates from canonical form
- **THEN** the server returns edits producing the canonical form

#### Scenario: Unformattable text

- **WHEN** the in-memory text cannot be formatted
- **THEN** the server returns an empty result and the document is left
  unchanged

### Requirement: Editor integration with CLI fallback

The editor extension SHALL start the language server for `.zrk` documents
when a `zirk-lsp` executable is discoverable, and SHALL fall back to the
`editor-cli-bridge` — CLI diagnostics and `--types` hover — when it is not.
Both paths SHALL NOT be active for the same document at once.

#### Scenario: Server binary present

- **WHEN** a `zirk-lsp` executable is found and a `.zrk` file is opened
- **THEN** the extension routes document requests through the server

#### Scenario: Server binary absent

- **WHEN** no `zirk-lsp` executable is found
- **THEN** the extension behaves exactly as the `editor-cli-bridge` describes,
  including its cached hover

### Requirement: Semantic navigation and editing

The server SHALL provide definition, references, document highlights, and rename from the compiler-backed IDE query service. Results SHALL use resolved symbol identities and include all affected open and closed project files known to the workspace index.

#### Scenario: Definition and references in an open document
- **WHEN** the user requests definition or references on a resolved identifier
- **THEN** the server returns locations for the declaration and uses of that symbol

#### Scenario: Rename across project files
- **WHEN** the client requests a valid rename
- **THEN** the server returns one version-aware workspace edit covering all and only references to the selected symbol

#### Scenario: Stale snapshot edit
- **WHEN** a request result was computed for a document version that has since changed
- **THEN** the server discards or recomputes the result before applying an edit

### Requirement: Contextual language assistance

The server SHALL provide completion, signature help, inlay hints, workspace symbols, and code actions when the corresponding query is supported. Results SHALL be based on the active document version, project model, resolved imports, compiler types, and visibility rules.

#### Scenario: Completion respects scope and visibility
- **WHEN** completion is requested in a local scope or member access
- **THEN** suggestions contain only names valid and visible in that context

#### Scenario: Completion in imports
- **WHEN** completion is requested within an import statement
- **THEN** suggestions include only project modules, declared dependencies, and standard-library exports resolvable by the workspace

#### Scenario: Code action has a safe edit
- **WHEN** a diagnostic has a supported quick fix
- **THEN** the server returns a version-aware edit that resolves that diagnostic without modifying unrelated text

### Requirement: Semantic tokens

The server SHALL provide semantic tokens derived from resolved declarations and uses, including declaration/reference modifiers and symbol kinds. Syntax grammar highlighting SHALL remain available as the fallback when semantic analysis is unavailable.

#### Scenario: Resolved identifier colors
- **WHEN** a declaration and its resolved uses are visible in an analyzed document
- **THEN** they receive matching semantic kinds and the declaration receives a declaration modifier

#### Scenario: Broken or incomplete source
- **WHEN** semantic analysis cannot classify a token reliably
- **THEN** the server omits that semantic token and the editor retains lexical grammar highlighting

### Requirement: Workspace model and incremental analysis

The server SHALL load the active project's manifest, source roots, modules, and declared dependencies using the compiler's existing resolution rules. It SHALL maintain versioned in-memory overlays for open documents, analyze affected inputs after edits, and keep results associated with the snapshot that produced them.

#### Scenario: Unsaved imported module
- **WHEN** an open imported module has unsaved edits
- **THEN** project analysis and requests in importing files use the in-memory version of that module

#### Scenario: Dependency or project configuration changes
- **WHEN** the project manifest, dependency graph, or source-root configuration changes
- **THEN** the server refreshes the affected project model and invalidates results derived from the previous model

#### Scenario: Analysis cancellation
- **WHEN** a newer edit supersedes an in-flight analysis or request
- **THEN** the server cancels or abandons obsolete work and publishes only results for current document versions

#### Scenario: Project root containment
- **WHEN** the workspace loader encounters an import outside authorized roots
- **THEN** the server follows the compiler's import-containment policy and does not index or expose that source
