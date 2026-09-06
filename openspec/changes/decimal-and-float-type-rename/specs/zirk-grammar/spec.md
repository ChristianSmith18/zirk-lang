## MODIFIED Requirements

### Requirement: Type names are valid identifiers

The grammar SHALL accept `Decimal` and `Dec` as type names for the exact base-ten decimal scalar. It SHALL accept `Float16`, `Float32`, `Float64`, `Float128`, and `Float` as type names for the IEEE 754 binary family. The spellings `BinaryFloat`, `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, and `BinaryFloat128` SHALL NOT be accepted as type names and SHALL be rejected with a diagnostic.

#### Scenario: New exact decimal type name
- **WHEN** a type annotation or type expression uses `Decimal`
- **THEN** it is parsed as the exact base-ten decimal type

#### Scenario: New exact decimal alias
- **WHEN** a type annotation or type expression uses `Dec`
- **THEN** it is parsed as an alias for the exact base-ten decimal type

#### Scenario: New binary float type name
- **WHEN** a type annotation or type expression uses `Float64`
- **THEN** it is parsed as the 64-bit IEEE 754 binary floating-point type

#### Scenario: Old binary name is rejected
- **WHEN** a type annotation uses `BinaryFloat64`
- **THEN** the parser emits a diagnostic naming `Float64` as the replacement
