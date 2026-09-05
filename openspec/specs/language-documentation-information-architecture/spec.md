## Purpose

Define how the complete Zirk language, runtime, standard library, project model, packages, and toolchain are organized into a discoverable public documentation system.
## Requirements
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
Documentation examples and claims MUST remain compatible with the normative Zirk specifications, pages SHALL link to the relevant source specification where deeper normative detail exists, and coverage SHALL be audited against `docs/01_plantilla_zirk.md` as the exhaustive historical topic inventory.

#### Scenario: Documentation is audited
- **WHEN** syntax examples and semantic claims are compared with the repository specifications
- **THEN** no example introduces conflicting keywords, types, operators, or runtime guarantees

#### Scenario: Historical and final documents conflict
- **WHEN** `docs/01_plantilla_zirk.md` disagrees with a final language, runtime, standard-library, compiler, or consolidated specification
- **THEN** the handbook follows the final specification unless a newer language-author correction explicitly supersedes that snapshot

### Requirement: Progressive type-system learning path
The handbook SHALL introduce the Zirk type taxonomy, conceptual tree, primitive/native/user-defined/special categories, value/reference behavior, contracts, conversions, and native operator model before relying on those concepts in individual type chapters.

#### Scenario: Reader enters everyday types
- **WHEN** a reader follows the canonical handbook sequence into the type-system unit
- **THEN** the conceptual model precedes numeric, Boolean, Char, String, and special-type API chapters

### Requirement: Dedicated temporal unit
The handbook SHALL provide a dedicated ordered temporal unit covering `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period`, followed by composition, arithmetic, parsing/formatting, DST, and error guidance.

#### Scenario: Reader chooses temporal type
- **WHEN** a developer needs to represent a birthday, local appointment, absolute event, timeout, or calendar recurrence
- **THEN** the temporal overview directs them to a distinct appropriate type and explains why

### Requirement: Canonical type cross-links
Every detailed type chapter SHALL link to its conceptual owner, closely interacting types, operator reference, and adjacent previous/next handbook chapters without duplicating normative definitions inconsistently.

#### Scenario: String reader follows semantics
- **WHEN** a reader needs strict mutability or operator details from the String chapter
- **THEN** direct links reach the binding model and operator reference while the String chapter remains the primary owner of String behavior

### Requirement: Completed core-language learning route
The handbook SHALL provide an ordered, linked route through callables, objects/contracts, generics, tuples/records/enums/unions, collections/iteration, and pattern matching, with conceptual pages before detailed APIs and reference tables after explanatory chapters.

#### Scenario: Agent follows implementation route
- **WHEN** an implementation agent starts from SUMMARY or the core-language overview
- **THEN** it can reach every normative owner, explanatory chapter, API/operator table, valid/invalid example, and implementation-status note without relying on planned placeholders

### Requirement: Obsolete placeholders are resolved
Unlinked planned entries that duplicate published units SHALL be removed or converted into intentional links, while genuinely future areas SHALL remain clearly identified as planned rather than appearing complete.

#### Scenario: Duplicate type-model placeholder
- **WHEN** the type model already has a published canonical unit
- **THEN** SUMMARY does not retain a second unlinked placeholder for the same material

### Requirement: Canonical ownership includes failures resources and authority
The documentation source map SHALL place accepted error/resource/permission semantics after the master and consolidated contributor checkpoint, assign normative ownership across language/runtime/stdlib/compiler specifications, and classify template and old phase answers as historical when conflicting.

#### Scenario: Agent resolves permission conflict
- **WHEN** an agent finds `compile_permissions` in an older document and `permissions` with `during` in the current checkpoint
- **THEN** the source map directs the agent to use the current two-block model

### Requirement: Navigation exposes the complete implementation path
The handbook summary, reference indexes, glossary, feature status, and contributor reading order SHALL link error, resource, permission, environment/secret, CLI approval, and security chapters without duplicate placeholders.

#### Scenario: Runtime implementer follows reading order
- **WHEN** a runtime contributor begins at the consolidated checkpoint
- **THEN** they can reach normative failure propagation, resource cleanup, permission enforcement, and handbook examples through explicit links

### Requirement: Complete memory and unsafe path
The documentation SHALL lead from public automatic memory and reference behavior through weak/dependent references, native views, pointers, transactional unsafe rollback, irreversible commit, and undefined-behavior limits with valid and invalid examples.

#### Scenario: Developer prepares native interop
- **WHEN** a reader follows the memory and safety unit
- **THEN** they can identify which operations are safe, unsafe but reversible, irreversible, or fundamentally unrecoverable

### Requirement: Complete structured concurrency path
The documentation SHALL lead from tasks and await through scopes, failure, cancellation, timeout, aggregation, select, channels, transfer/share rules, parallel work, threads, synchronization, atomics, and data-race prevention.

#### Scenario: Developer designs concurrent workflow
- **WHEN** a reader follows the concurrency unit
- **THEN** they can choose an appropriate primitive and predict its lifetime, failure, cancellation, ordering, and sharing behavior

### Requirement: Deep documentation is delivered in three blocks
The handbook SHALL complete substantive documentation in this order: standard library with toolchain; testing with native and low-level development; tutorials with reference material. Each block SHALL be internally linked and audited before the next block is marked complete.

#### Scenario: First block is reviewed
- **WHEN** standard-library and toolchain tasks are marked complete
- **THEN** their module, API, command, pipeline, permission, error, performance, platform, example, status, and navigation coverage has been audited before testing work begins

### Requirement: Operational chapters support independent use
Every substantive module, tool, testing, or low-level chapter SHALL contain enough contracts, signatures or commands, examples, failures, permissions, blocking/cancellation, complexity/performance, platform behavior, implementation status, and source links for its subject, while omitting dimensions that genuinely do not apply.

#### Scenario: Reader opens a standard module
- **WHEN** a reader consults `std.fs`
- **THEN** the chapter is sufficient to select APIs, understand results and permissions, predict resource/cancellation behavior, and follow working examples without consulting historical notes

### Requirement: Tutorials compose documented contracts
Tutorials SHALL build complete progressively explained programs from APIs and semantics already documented in owning chapters, SHALL include expected output and failure handling, and SHALL link each major concept back to its reference owner.

#### Scenario: Reader completes an application tutorial
- **WHEN** the tutorial finishes
- **THEN** the reader has a coherent project layout, manifest, code, commands, expected behavior, tests, permissions, diagnostics guidance, and next references

### Requirement: Reference material is exhaustive and website-ready
Reference indexes SHALL cover the accepted grammar, keywords, operators, literals, attributes/decorators, diagnostics, CLI, configuration, permissions, standard library, feature status, compatibility, glossary, and normative sources with unique navigation entries and resolvable links.

#### Scenario: Website navigation is generated
- **WHEN** the future website consumes `SUMMARY.md` and reference pages
- **THEN** it can build a complete hierarchy and search surface without placeholder chapters or ambiguous duplicate owners

### Requirement: Ordered type deep-dive track in the handbook

The handbook information architecture SHALL provide an ordered track that teaches the type spectrum before presenting per-type API chapters, and SHALL include a hub, rewritten category chapter, a "choosing a type" decision page, and new/expanded per-type chapters for `List`, `Array`, `Regex`, `Pointer`, `NativeSlice`, `Weak`, `Dependent`, and `Fn` / `Function`.

#### Scenario: Reader follows the canonical learning path
- **WHEN** a reader opens `03-everyday-types` from `SUMMARY.md`
- **THEN** the first chapter is the new hub, followed by the category chapter, then per-type chapters in an order that respects the spectrum

#### Scenario: Reader finds a missing type
- **WHEN** a reader looks for `Regex`, `List`, `Pointer`, `NativeSlice`, `Weak`, `Dependent`, or `Fn` in the handbook
- **THEN** `SUMMARY.md` lists a dedicated canonical chapter or a clear cross-reference from the hub

### Requirement: Cross-linked built-in catalog

The reference page `11-reference/03-built-in-types.md` SHALL contain at least one runnable example for every major built-in category and SHALL link every type to its detailed handbook chapter or to an explicit implementation-status notice.

#### Scenario: Developer uses the built-in catalog as a lookup
- **WHEN** a reader opens `03-built-in-types.md` to choose a type
- **THEN** the page shows an example for each spectrum position and links to the deeper chapter

