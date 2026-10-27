## ADDED Requirements

### Requirement: Structural composite execution matches native semantics
The verified-IR interpreter SHALL execute every supported structural union and intersection operation with the same observable behavior as native execution. Union tags SHALL select the correct payload and remain consistent through values, calls, returns, storage, and matching. Managed references inside structural payloads SHALL preserve identity and remain valid for the execution lifetime. Intersections SHALL remain views of the original value. Invalid verified-IR composites SHALL be rejected by verification, not interpreted with guessed layouts.

#### Scenario: Structural alternatives round-trip through interpretation
- **WHEN** a program constructs, stores, passes, and returns distinct structural union alternatives
- **THEN** each value narrows and projects the same properties as in native execution

#### Scenario: Structural union match selects the matching payload
- **WHEN** an interpreted match selects a structural alternative by its discriminant
- **THEN** only that arm executes and its bindings use the alternative's checked types

#### Scenario: Managed object identity survives a union payload
- **WHEN** a structural union contains a managed object reference and the payload is passed or projected
- **THEN** mutations through either valid view observe the same object identity

#### Scenario: Intersection does not clone the underlying object
- **WHEN** an interpreted value is read and mutated through compatible intersection views
- **THEN** all views observe the same underlying object and mutation

#### Scenario: Interpreter and native outcomes agree
- **WHEN** the same supported structural-composite fixture runs through both engines
- **THEN** results, output, selected match arms, and catchable failures are equivalent
