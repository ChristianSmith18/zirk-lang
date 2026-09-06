## MODIFIED Requirements

### Requirement: Every documented feature has an owning phase

Every feature defined by the normative sources SHALL have exactly one
construction phase assigned to it in `docs/init/ZIRK_ROADMAP.md`.

A feature without an assigned phase is not a deferred feature: it is a
feature that nobody will build. Assigning it is what prevents it from
sneaking into the active phase and overflowing it, or from remaining as debt
with no due date.

The infix exponentiation operator `**` and its compound form `**=` are
delivered by Phase 3b, together with the numeric families they operate on.
Once delivered, `**` is part of the implemented subset and is no longer a
deferred construct.

#### Scenario: Normative feature without a phase
- **WHEN** a normative source defines a feature that no phase of the roadmap names
- **THEN** the roadmap is corrected by assigning it a phase before implementing any part of it

#### Scenario: Features orphaned by normative refinement
- **WHEN** the `Float` family, graphemic `Char`, deep contextual conversion, the bitwise and shift operators, and string interpolation are consulted
- **THEN** the roadmap assigns them to Phase 3b
- **AND** assigns `inmut::strict` to Phase 4 and the temporal family to Phase 7

#### Scenario: Exponentiation operator is delivered, not deferred
- **WHEN** a program in the implemented subset uses `**` or `**=`
- **THEN** no phase diagnostic is emitted and the operator is compiled
