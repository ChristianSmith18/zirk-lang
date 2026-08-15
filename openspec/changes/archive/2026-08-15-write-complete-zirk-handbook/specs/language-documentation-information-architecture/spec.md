## MODIFIED Requirements

### Requirement: Complete language documentation taxonomy
The documentation SHALL organize Zirk material under `docs/handbook/` into discoverable sections for orientation, audience-specific learning paths, progressive language syntax and semantics, the type system and object model, error handling, effects and permissions, concurrency and runtime behavior, projects and builds, the standard library, native and low-level programming, metaprogramming, tooling, testing, packages, tutorials, reference material, explanations, and appendices.

#### Scenario: Reader explores the language
- **WHEN** a reader opens `docs/handbook/README.md` or `docs/handbook/SUMMARY.md`
- **THEN** every required subject area is reachable through a named section without knowledge of historical repository filenames

#### Scenario: Reader chooses an intent
- **WHEN** a reader wants to learn sequentially, solve a task, understand a design decision, or look up exact behavior
- **THEN** the handbook identifies an appropriate learning, tutorial, explanation, or reference path

### Requirement: Progressive learning path
The documentation SHALL provide an ordered path from understanding Zirk's purpose and status through installation, a first program, core syntax, values, control flow, functions, types, objects, errors, effects, resources, concurrency, modules, and building a complete project.

#### Scenario: New reader starts learning
- **WHEN** a reader selects the primary getting-started path
- **THEN** the handbook presents ordered lessons with prerequisites, specification-compatible Zirk examples, expected outcomes, implementation-status notes where needed, and links to deeper reference material

#### Scenario: Experienced programmer starts learning
- **WHEN** a reader selects an accelerated path for programmers familiar with another typed language
- **THEN** the handbook highlights Zirk-specific semantics and links directly to the necessary core chapters

### Requirement: Reference-grade topic pages
Each documented language feature SHALL include its purpose, canonical syntax, semantics, constraints, representative valid examples, common mistakes and expected diagnostics where applicable, important interactions with other features, implementation status, and links to related topics and normative sources.

#### Scenario: Experienced reader looks up a feature
- **WHEN** a reader opens a reference topic such as nullability, `match`, generics, tasks, resources, or permissions
- **THEN** the page provides sufficient syntax and behavioral detail to use the feature without consulting repository specifications for ordinary cases

#### Scenario: Topic complexity differs
- **WHEN** two features require different amounts of explanation
- **THEN** each page uses the depth and section structure its subject requires rather than an imposed uniform size

### Requirement: Cross-referenced source fidelity
Documentation examples and claims MUST remain compatible with the normative Zirk specifications, pages SHALL link to the relevant source specification where deeper normative detail exists, and coverage SHALL be audited against `docs/01_plantilla_zirk.md` as the exhaustive historical topic inventory.

#### Scenario: Documentation is audited
- **WHEN** syntax examples and semantic claims are compared with the repository specifications
- **THEN** no example introduces conflicting keywords, types, operators, or runtime guarantees

#### Scenario: Historical and final documents conflict
- **WHEN** `docs/01_plantilla_zirk.md` disagrees with a final language, runtime, standard-library, compiler, or consolidated specification
- **THEN** the handbook follows the final specification and records the historical topic only when it remains useful
