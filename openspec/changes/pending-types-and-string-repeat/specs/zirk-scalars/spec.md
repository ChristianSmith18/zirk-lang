# Delta spec: zirk-scalars

## MODIFIED Requirements

### Requirement: Complete family of integer widths

The type system SHALL recognize `Int8`, `Int16`, `Int32`, `Int64`, `Int128` signed and `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128` unsigned, with `Int`/`Integer` as an alias for `Int32` and `UInt`/`UInteger` as an alias for `UInt32`.

#### Scenario: Literal in an explicit width
- **WHEN** a variable is annotated `Int8` and an integer literal within its range is assigned to it
- **THEN** the check succeeds and the value is represented as 8 signed bits

#### Scenario: Literal out of range for its width
- **WHEN** a literal exceeds the representable range of the annotated width
- **THEN** a diagnostic is emitted at compile time, without waiting for runtime

#### Scenario: Unsigned alias resolves to `UInt32`
- **WHEN** a variable is annotated `UInt` or `UInteger` and assigned `42`
- **THEN** the check succeeds and the value is represented as `UInt32`, with the same range, overflow rules and member surface
