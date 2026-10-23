## MODIFIED Requirements

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

## ADDED Requirements

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
