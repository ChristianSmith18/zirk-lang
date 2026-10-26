## ADDED Requirements

### Requirement: Tagged unions are safe managed values
The runtime and collector SHALL use a verified union tag to trace only managed-reference fields in the active payload. Union copies, moves, captures, returns, and nested aggregate storage SHALL preserve the active tag and payload together. Invalid internal tags SHALL fail as controlled runtime corruption diagnostics and SHALL NOT cause the collector to interpret arbitrary payload bytes as pointers.

#### Scenario: Reference alternative remains live
- **WHEN** a union containing the only live reference to an object is stored in a local or suspended task and collection runs
- **THEN** the object remains reachable while that union value is live

#### Scenario: Scalar payload is not traced as a pointer
- **WHEN** a scalar alternative's bits happen to resemble a managed address
- **THEN** the collector does not treat them as a reference

#### Scenario: Nested union traces its active member
- **WHEN** a union payload contains another union with a managed reference
- **THEN** tracing follows the active tags recursively and retains the nested reference

#### Scenario: Corrupt tag does not dereference payload
- **WHEN** runtime validation encounters an out-of-range union tag
- **THEN** execution fails in a controlled way before payload tracing or projection
