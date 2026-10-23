# zirk-ide-query Specification

## Purpose

Provide compiler-backed, editor-independent queries that explain how names, types, and members in a Zirk project relate to their declarations. The query contract is the shared semantic foundation for LSP clients, editor integrations, and future tooling.

## Requirements

#

#

#

#

#

#

### Requirement: Snapshot-scoped symbol identity

The IDE query service SHALL assign a distinct identity to every declaration in an analyzed project snapshot and SHALL associate each resolved identifier use with the declaration it names. Identity SHALL distinguish same-spelling declarations in distinct legal scopes and SHALL include declarations and uses across loaded modules and imports. The query service SHALL preserve compiler diagnostics for declarations that violate Zirk's scope rules.

#### Scenario: Same-spelling locals in separate functions
- **WHEN** separate functions each declare a local with the same spelling
- **THEN** each use resolves to the local declared by its own function
- **AND** references for either local exclude the other function's declaration and uses

#### Scenario: Invalid nested shadowing
- **WHEN** an edit declares a local that violates Zirk's shadowing rules
- **THEN** the compiler diagnostic remains visible
- **AND** the IDE does not treat the invalid declaration as a successful rename target

#### Scenario: Imported symbol use
- **WHEN** a source file imports a symbol from another project module and references it
- **THEN** the reference resolves to the imported declaration's identity and source location

#### Scenario: Incomplete source
- **WHEN** an edit leaves part of a file syntactically invalid
- **THEN** the service preserves queryable identities for unaffected declarations where analysis can safely recover
- **AND** it does not return a stale identity for an unresolved or ambiguous use

### Requirement: Definition and reference queries

The IDE query service SHALL answer definition, references, and document-highlight queries from resolved symbol identities rather than matching identifier text. Queries SHALL honor project visibility and SHALL return source ranges with document identities.

#### Scenario: Go to a local declaration
- **WHEN** the user requests definition on a resolved local variable use
- **THEN** the query returns the declaration range of that exact binding

#### Scenario: Find references across modules
- **WHEN** the user requests references for an exported declaration
- **THEN** the result includes its declaration and all resolved uses in the analyzed project

#### Scenario: Same spelling in unrelated scopes
- **WHEN** another symbol has the same name but a different identity
- **THEN** its declaration and uses are excluded from the result

### Requirement: Safe symbol rename

The IDE query service SHALL prepare and apply rename edits for a resolved symbol across every affected project document. It SHALL reject invalid identifiers, reserved words, unresolved targets, and edits that would capture a different symbol or create a name collision.

#### Scenario: Rename an imported symbol
- **WHEN** an exported declaration is renamed
- **THEN** the service updates its declaration, project references, and corresponding import names while preserving explicit local aliases

#### Scenario: Rename a shadowed binding
- **WHEN** a local binding is renamed
- **THEN** only references resolved to that binding are edited
- **AND** outer or nested bindings with the same spelling remain unchanged

#### Scenario: Rename would change resolution
- **WHEN** a proposed name would make a use resolve to another declaration or violate a scope rule
- **THEN** the service rejects the rename without returning a partial edit set

### Requirement: Contextual completion and signature queries

The IDE query service SHALL provide completion and signature information using the current syntax context, lexical scope, inferred types, member visibility, project exports, standard-library declarations, and resolved dependencies. Results SHALL carry completion kind, insertion text, and available documentation or type detail.

#### Scenario: Member completion after a typed receiver
- **WHEN** the user requests completion after a member-access operator on a resolved expression
- **THEN** the result contains only members available on that expression's type and visible from the current context

#### Scenario: Completion in an import
- **WHEN** the user requests completion in an import path or imported-name list
- **THEN** the result contains modules or exported names available through the project's declared roots and dependencies

#### Scenario: Signature help
- **WHEN** the cursor is inside a call to a resolved callable
- **THEN** the result identifies the active signature and current parameter, including generic substitutions available at that call site

### Requirement: Semantic classifications and inferred information

The IDE query service SHALL classify declarations and resolved uses by semantic kind and modifiers, independently of theme colors. It SHALL expose inferred type and parameter information for editor hints only when the compiler has a reliable result.

#### Scenario: Symbol-aware highlighting
- **WHEN** a declaration and a reference are both present in a project snapshot
- **THEN** both receive the same semantic kind, with a declaration modifier on the declaration

#### Scenario: Inferred local type
- **WHEN** a local has a reliable inferred type and no written annotation
- **THEN** the query exposes that type as an optional inlay hint

#### Scenario: Unavailable inferred type
- **WHEN** analysis cannot determine a reliable type
- **THEN** the service omits the hint instead of inventing a type

### Requirement: Workspace symbol search

The IDE query service SHALL search declarations in the active project and its explicitly resolved dependencies, with result locations and semantic kinds. Search SHALL respect configured visibility and SHALL not index files outside authorized project roots.

#### Scenario: Search project declarations
- **WHEN** a user searches for a symbol by name in the workspace
- **THEN** matching project declarations are returned with their document locations

#### Scenario: Dependency boundary
- **WHEN** a dependency is unresolved or outside the project's authorized roots
- **THEN** its files are not indexed or offered as project symbols
