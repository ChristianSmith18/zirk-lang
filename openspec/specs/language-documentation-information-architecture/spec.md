## Purpose

Define how the complete Zirk language, runtime, standard library, project model, packages, and toolchain are organized into a discoverable public documentation system.

## Requirements

### Requirement: Complete language documentation taxonomy
The documentation SHALL organize Zirk material into discoverable sections for getting started, language syntax and semantics, type system and object model, error handling, concurrency and runtime, standard library, tooling, project configuration, packages, interoperability, and implementation status.

#### Scenario: Reader explores the language
- **WHEN** a reader opens the primary documentation navigation
- **THEN** every required subject area is reachable through a named section without knowledge of repository filenames

### Requirement: Progressive learning path
The documentation SHALL provide a guided path from installation and first program through core syntax, types, functions, objects, error handling, concurrency, and building a complete project.

#### Scenario: New reader starts learning
- **WHEN** a reader selects the getting-started path
- **THEN** the site presents ordered lessons with runnable-looking Zirk examples, prerequisites, expected outcomes, and links to deeper reference material

### Requirement: Reference-grade topic pages
Each documented language feature SHALL include its purpose, canonical syntax, semantics, constraints, at least one representative example, common pitfalls where applicable, and links to related topics.

#### Scenario: Experienced reader looks up a feature
- **WHEN** a reader opens a reference topic such as nullability, `match`, generics, tasks, or permissions
- **THEN** the page provides sufficient syntax and behavioral detail to use the feature without consulting the repository specification for ordinary cases

### Requirement: Standard library catalog
The documentation SHALL catalog every initial standard-library module named by the normative standard-library specification and describe its purpose, principal types or operations, error model, permission requirements, blocking or cancellation behavior, and related examples.

#### Scenario: Reader browses a standard module
- **WHEN** a reader opens a module such as `std.fs` or `std.task`
- **THEN** the page identifies the module contract and its operational constraints in a consistent reference format

### Requirement: Specification and implementation distinction
The documentation SHALL distinguish normative language design from currently implemented compiler support on every documentation page.

#### Scenario: Reader views an aspirational feature
- **WHEN** a page describes a feature not yet implemented in the current roadmap phase
- **THEN** a visible status notice makes clear that the page documents the target language and links to implementation status

### Requirement: Cross-referenced source fidelity
Documentation examples and claims MUST remain compatible with the normative Zirk specifications, and pages SHALL link to the relevant source specification where deeper normative detail exists.

#### Scenario: Documentation is audited
- **WHEN** syntax examples and semantic claims are compared with the repository specifications
- **THEN** no example introduces conflicting keywords, types, operators, or runtime guarantees
