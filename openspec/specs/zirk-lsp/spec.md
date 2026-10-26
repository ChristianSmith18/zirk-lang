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

The server SHALL load and validate the active project's `.zkinit`, `zirk.lock`, source roots, modules, declared dependencies, application globals, and capability requirements using the compiler's project-resolution rules. It SHALL maintain versioned in-memory overlays for open source and manifest documents, analyze affected inputs after edits, and keep results associated with the snapshot that produced them. A manifest edit SHALL invalidate dependent project, type-inference, dependency, permission-tree, and execution-entry results derived from the previous model.

#### Scenario: Unsaved imported module
- **WHEN** an open imported module has unsaved edits
- **THEN** project analysis and requests in importing files use the in-memory version of that module

#### Scenario: Dependency or project configuration changes
- **WHEN** the project manifest, dependency graph, global declarations, capability requirements, or source-root configuration changes
- **THEN** the server refreshes the affected project model and invalidates results derived from the previous model

#### Scenario: Analysis cancellation
- **WHEN** a newer edit supersedes an in-flight analysis or request
- **THEN** the server cancels or abandons obsolete work and publishes only results for current document versions

#### Scenario: Unsaved manifest changes global inference
- **WHEN** an open `.zkinit` changes a global's declared type before it is saved
- **THEN** source diagnostics and type assistance use the updated in-memory declaration

#### Scenario: Project root containment
- **WHEN** the workspace loader encounters an import outside authorized roots
- **THEN** the server follows the compiler's import-containment policy and does not index or expose that source

### Requirement: The language server validates `.zkinit` with compiler semantics

When a `.zkinit` document is opened or changed, the server SHALL parse the current in-memory text with the canonical manifest parser and publish field-specific diagnostics for invalid project metadata, entry/root values, source roots, `build_targets`, dependency declarations, global types/defaults, and capability requirements. The server SHALL not advertise configuration validation if the compiler parser or project context is unavailable.

#### Scenario: Invalid project field is edited
- **WHEN** a user changes `project.type` to an unsupported value
- **THEN** the server publishes a diagnostic at that field using the current unsaved text

#### Scenario: Legacy permission syntax is present
- **WHEN** an open manifest contains a removed `permissions` section
- **THEN** the server marks it as an error and suggests the single `requires` form

### Requirement: Manifest code actions are narrow and version-aware

For a compiler-supported missing capability or manifest field diagnostic, the server MAY offer a code action that adds the smallest required entry to `.zkinit`. The action SHALL use the canonical formatter and a version-aware edit, SHALL identify whether the requester is the application or a dependency, and SHALL NOT grant runtime consent or modify dependency files.

#### Scenario: Add a capability requirement
- **WHEN** compiler metadata identifies an application source operation that requires a declared capability
- **THEN** the server offers an action to add that capability to the application's `requires` block

#### Scenario: Dependency operation needs a capability
- **WHEN** the source operation belongs to a dependency package
- **THEN** the action identifies the dependency as requester and does not silently edit its source or manifest

#### Scenario: Manifest changes before action is applied
- **WHEN** the manifest version changes after a code action was computed
- **THEN** the server discards or recomputes the stale edit

### Requirement: Editor analysis understands composite type expressions
The language server SHALL parse and report diagnostics for nested union/intersection expressions, singleton constants, structural object types, generic type operators, and flow-sensitive narrowing using the same compiler semantics as CLI analysis. Hover and inlay information SHALL show the most precise reliable type and preserve useful alias names. Completion SHALL expose only members valid on every current alternative or on the active narrowed alternative.

#### Scenario: Hover shows a narrowed alternative
- **WHEN** the cursor is inside a branch narrowed by a Boolean discriminant
- **THEN** hover reports the selected contract and its exact literal properties

#### Scenario: Completion uses common union members
- **WHEN** completion is requested on a value before narrowing
- **THEN** only members valid on every union alternative are offered

#### Scenario: Type-computation budget diagnostic is live
- **WHEN** an edit exceeds a documented type-evaluation budget
- **THEN** the server reports the current diagnostic and clears it after the expression is corrected

#### Scenario: Invalid source never returns stale narrowing
- **WHEN** an edit changes a discriminant or leaves the expression incomplete
- **THEN** hover and completion omit stale types from the previous snapshot
