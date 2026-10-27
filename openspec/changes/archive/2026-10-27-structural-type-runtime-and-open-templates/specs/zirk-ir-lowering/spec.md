## ADDED Requirements

### Requirement: Every accepted structural composite lowers to verified portable IR
Every structural union or intersection accepted by semantic analysis SHALL have a portable-IR path for construction, assignment, parameter passing, return, local storage, capture, narrowing, projection, and matching wherever the source operation is valid. Union injection, tag inspection, and payload projection SHALL be explicit and verified. Intersection lowering SHALL preserve the original value and expose only the statically proven combined view. Unsupported layouts SHALL produce a source-located lowering diagnostic before backend emission; they SHALL NOT produce malformed IR or a backend panic.

#### Scenario: Structural union crosses a function boundary
- **WHEN** a value of a structural union is passed to and returned from a typed function
- **THEN** verified IR preserves its selected alternative and managed-reference metadata

#### Scenario: Structural union projection follows a checked tag
- **WHEN** a narrowed branch reads an alternative-specific property
- **THEN** IR projects the payload only after verifying the matching alternative tag

#### Scenario: Structural intersection lowers as a view
- **WHEN** one object is passed through an intersection of compatible structural types
- **THEN** IR retains the same object value and supplies the combined static member information without allocating a second object

#### Scenario: Malformed composite payload is rejected by verification
- **WHEN** a union operation contains an invalid tag, mismatched payload type, or invalid projection
- **THEN** IR verification rejects the module before interpretation or code generation

#### Scenario: Unsupported source layout receives a diagnostic
- **WHEN** semantic analysis accepts a structural composite but its target-independent representation cannot be formed
- **THEN** lowering reports a source-located implementation diagnostic instead of emitting partial IR
