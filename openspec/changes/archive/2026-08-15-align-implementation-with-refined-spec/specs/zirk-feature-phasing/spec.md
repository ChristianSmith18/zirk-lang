## ADDED Requirements

### Requirement: Every documented feature has an owning phase

Every feature defined by the normative sources SHALL have exactly one
construction phase assigned in `docs/init/ZIRK_ROADMAP.md`.

A feature with no assigned phase is not a deferred feature: it is a feature
no one will build. Assigning it is what prevents it from slipping into the
active phase and overflowing it, or remaining as undated debt.

#### Scenario: Normative feature without a phase
- **WHEN** a normative source defines a feature that no roadmap phase names
- **THEN** the roadmap is corrected by assigning it a phase before implementing any part of it

#### Scenario: Orphaned features from the normative refinement
- **WHEN** the `Float` family, graphemic `Char`, deep contextual conversion, the bitwise and shift operators, and string interpolation are queried
- **THEN** the roadmap assigns them to Phase 3b
- **AND** it assigns `inmut::strict` to Phase 4 and the temporal family to Phase 7

### Requirement: Phase diagnostic for what is not implemented

The compiler SHALL emit a diagnostic that names the construct and its
arrival phase for any construct, keyword, operator, literal, or type that
belongs to the language but not to the implemented phase.

The compiler SHALL NOT produce a generic syntax error, an "unrecognized
character" error, or a silent interpretation different from the construct
that was written.

#### Scenario: Construct from a later phase
- **WHEN** the source contains a language construct the current phase does not implement
- **THEN** the diagnostic names it and states the phase in which it arrives

#### Scenario: No silent interpretation
- **WHEN** the source contains a language literal or operator the current phase does not implement
- **THEN** the compiler recognizes it as such and defers it with its phase
- **AND** does NOT reinterpret it as a different sequence of tokens

### Requirement: The pending type table reflects the language as it currently stands

The known-but-not-implemented type table SHALL contain only types the
language currently defines, each with the phase that actually brings it.

A type removed from the language SHALL disappear from the table:
announcing its arrival teaches a language that does not exist.

#### Scenario: Type removed from the language
- **WHEN** an annotation names `Decimal64` or another member of the former `Decimal` family
- **THEN** the diagnostic treats it as a nonexistent type
- **AND** does NOT announce any arrival phase

#### Scenario: Correctly declared phase
- **WHEN** an annotation names a pending type
- **THEN** the phase stated by the diagnostic matches the one the roadmap assigns to it
