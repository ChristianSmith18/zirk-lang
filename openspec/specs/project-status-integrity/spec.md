# project-status-integrity Specification

## Purpose
TBD - created by archiving change fix-docs-tooling-drift. Update Purpose after archive.
## Requirements
### Requirement: Single feature-status catalog

The project SHALL maintain a single, versioned catalog of language feature
status (`docs/init/ZIRK_FEATURE_STATUS.md` or equivalent), and that catalog
SHALL be the only source enumerating, per feature, its owning phase and its
status at each pipeline stage (recognized by the lexer, parsed,
semantically checked, lowered to IR, supported by backend/runtime, available
through CLI/tooling).

No other document (`README.md`, `docs/init/ZIRK_ROADMAP.md`,
`docs/init/ZIRK_AGENT_PROMPT.md`, `docs/decisions/README.md`) SHALL restate
that enumeration in its own words; instead it SHALL link to the catalog.

#### Scenario: A contributor looks up a feature's status
- **WHEN** a contributor or agent wants to know whether a language feature is already implemented
- **THEN** they find a single entry in the status catalog that answers it, without reconciling different wording across other documents

#### Scenario: A new feature is added to the roadmap
- **WHEN** an owning phase is assigned to a new feature in `docs/init/ZIRK_ROADMAP.md`
- **THEN** the status catalog gains a corresponding entry in the same change
- **AND** no document describes that feature as implemented before the catalog marks it so

### Requirement: README reflects the current phase

`README.md` SHALL declare the current implementation phase according to
`docs/init/ZIRK_ROADMAP.md`, and SHALL NOT describe as nonexistent any
construct the status catalog marks as implemented.

#### Scenario: The roadmap advances a phase
- **WHEN** a phase is marked complete in `docs/init/ZIRK_ROADMAP.md`
- **THEN** `README.md` is updated in the same change to declare that phase as current
- **AND** the README's "does not yet exist" section is updated so it does not contradict the status catalog

#### Scenario: A new contributor reads the README
- **WHEN** a new contributor reads `README.md` as their first contact with the project
- **THEN** the phase and language subset it declares match what `cargo test --workspace` verifies today

### Requirement: The ADR index matches each ADR's real status

`docs/decisions/README.md` SHALL enumerate exactly the ADRs present under
`docs/decisions/`, and the status (open/accepted/closed) it declares for
each one SHALL match the status the ADR's own file declares.

#### Scenario: An ADR is closed
- **WHEN** an ADR's body moves from "open" to "closed" or "accepted"
- **THEN** `docs/decisions/README.md` is updated in the same change to reflect that new status

#### Scenario: A new ADR is added
- **WHEN** a new `ADR-0NN-*.md` file is created under `docs/decisions/`
- **THEN** `docs/decisions/README.md` includes an entry for it before the change is considered complete

### Requirement: A phase's status is not declared inconsistently across documents

`docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_AGENT_PROMPT.md`, the status catalog, and `README.md` SHALL declare the same completion status for any phase all four mention.

#### Scenario: Conflicting wording found during an audit
- **WHEN** an audit finds that two documents declare different statuses for the same phase
- **THEN** the code and tests are investigated to determine which status is correct
- **AND** every conflicting document is corrected to the same status in the same change

### Requirement: The pending-type table does not repeat already-available types

The checker's `pending_type` table (`crates/zirk-sema/src/types.rs`) SHALL exclude any type or contract the checker already registers and resolves as natively available.

This rule is a direct application of the existing `zirk-feature-phasing`
requirement that the pending-type table reflect the language as it stands:
a type the checker already resolves correctly cannot simultaneously
announce a future arrival phase.

#### Scenario: A native contract is registered ahead of the pending-type check
- **WHEN** the checker registers `Iterable`, `Iterator`, or `Resource` as an available native contract
- **THEN** no type annotation naming them receives a "future phase" diagnostic

#### Scenario: Future regression
- **WHEN** a new native type or contract is added to the checker
- **THEN** the same change verifies it does not also remain in the pending-type table

### Requirement: Identified technical debt is recorded with an owner

An engineering risk discovered during a status audit and requiring its own design decision (memory strategy, concurrency safety, modularizing a large module, the scope of a language construct) SHALL be recorded in a follow-up document with a proposed future change name, instead of being resolved inside the same change that fixes documentation.

#### Scenario: An audit finds an out-of-scope engineering risk
- **WHEN** a status audit finds a risk that is neither a documentation fix nor a narrowly scoped bugfix
- **THEN** it is documented as debt with a description of the risk and the suggested name of a future OpenSpec change
- **AND** the change that performed the audit does not attempt to resolve that risk in the same commit

### Requirement: New type chapters stay consistent with the feature-status catalog

Any expanded or new type chapter that makes an implementation-status claim (e.g., "`List<T>` is delivered" or "`Regex` is specified") SHALL reference `docs/init/ZIRK_FEATURE_STATUS.md` and SHALL be updated in the same change if the status catalog is updated. The chapter SHALL NOT promote a feature to implemented unless the catalog already marks it implemented.

#### Scenario: A chapter mentions a delivered collection
- **WHEN** the `List` chapter states that `List<T>` is implemented
- **THEN** the feature-status catalog contains the same status and the change includes both updates

#### Scenario: A chapter describes a specified but pending feature
- **WHEN** the `Regex` or `Map` chapter documents behavior not yet implemented
- **THEN** the page displays a visible implementation-status notice and does not claim the feature runs today

