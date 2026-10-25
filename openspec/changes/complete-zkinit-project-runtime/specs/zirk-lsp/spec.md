## MODIFIED Requirements

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

#### Scenario: Project root containment
- **WHEN** the workspace loader encounters an import outside authorized roots
- **THEN** the server follows the compiler's import-containment policy and does not index or expose that source

#### Scenario: Unsaved manifest changes global inference
- **WHEN** an open `.zkinit` changes a global's declared type before it is saved
- **THEN** source diagnostics and type assistance use the updated in-memory declaration

## ADDED Requirements

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
