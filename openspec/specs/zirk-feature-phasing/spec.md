# zirk-feature-phasing

## Purpose

Defines the discipline that keeps the compiler honest about what it does not implement yet.

The language is documented in full while it is built in phases. That gap is deliberate, and it is only safe as long as every documented feature has an owning phase and the compiler names that phase instead of failing as if the feature did not exist.
## Requirements
### Requirement: Every documented feature has an owning phase

Every feature defined by the normative sources SHALL have exactly one
construction phase assigned to it in `docs/init/ZIRK_ROADMAP.md`.

A feature without an assigned phase is not a deferred feature: it is a
feature that nobody will build. Assigning it is what prevents it from
sneaking into the active phase and overflowing it, or from remaining as debt
with no due date.

#### Scenario: Normative feature without a phase
- **WHEN** a normative source defines a feature that no phase of the roadmap names
- **THEN** the roadmap is corrected by assigning it a phase before implementing any part of it

#### Scenario: Features orphaned by normative refinement
- **WHEN** the `Float` family, graphemic `Char`, deep contextual conversion, the bitwise and shift operators, and string interpolation are consulted
- **THEN** the roadmap assigns them to Phase 3b
- **AND** assigns `inmut::strict` to Phase 4 and the temporal family to Phase 7

### Requirement: Phase diagnostic for what is not implemented

The compiler SHALL emit a diagnostic naming the construct and its arrival
phase for any construct, keyword, operator, literal, or type that belongs to
the language but not to the implemented phase.

The compiler SHALL NOT produce a generic syntax error, an "unrecognized
character" error, or a silent interpretation different from the construct as
written.

#### Scenario: Construct from a later phase
- **WHEN** the source contains a language construct that the current phase does not implement
- **THEN** the diagnostic names it and indicates the phase in which it arrives

#### Scenario: Absence of silent interpretation
- **WHEN** the source contains a literal or an operator of the language that the current phase does not implement
- **THEN** the compiler recognizes it as such and defers it along with its phase
- **AND** it does NOT reinterpret it as a different sequence of tokens

### Requirement: The table of pending types reflects the current language

The table of known-but-not-implemented types SHALL contain only types that
the language defines today, each with the phase that actually brings it.

A type withdrawn from the language SHALL disappear from the table:
announcing its arrival would teach a language that does not exist.

#### Scenario: Type withdrawn from the language
- **WHEN** an annotation names `Decimal64` or another member of the old `Decimal` family
- **THEN** the diagnostic treats it as a nonexistent type
- **AND** it does NOT announce any arrival phase

#### Scenario: Correctly declared phase
- **WHEN** an annotation names a pending type
- **THEN** the phase indicated by the diagnostic matches the one the roadmap assigns to it

### Requirement: Memory and concurrency implementation order
The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured task semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts task scheduling
- **THEN** typed scopes, cancellation, transfer/share analysis, and cleanup behavior are already specified as prerequisites
