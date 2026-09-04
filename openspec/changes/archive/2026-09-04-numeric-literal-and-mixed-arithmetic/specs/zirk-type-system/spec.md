# zirk-type-system

## MODIFIED Requirements

### Requirement: Absence of implicit conversions

The checker SHALL NOT insert implicit conversions between distinct non-numeric types or between numeric types that could lose information. Implicit conversions between numeric types are permitted only when the destination can represent every value of the source type exactly, or when the compiler can prove at compile time that the specific value fits. All other conversions between numeric types require an explicit `as` cast.

#### Scenario: Assignment of an incompatible type
- **WHEN** `mut total: Int32 = "cuarenta";` is declared
- **THEN** a diagnostic pointing to the initializer is emitted
- **AND** the cause indicates that there is no implicit conversion from `String` to `Int32`

#### Scenario: Operands of distinct non-numeric types
- **WHEN** `String` and `Int32` are used with `+`
- **THEN** compilation fails because there is no common type between them

#### Scenario: Operands of distinct numeric types with a common type
- **WHEN** `Int8` and `Int32` are used with `+`
- **THEN** the operation is accepted and the result type is `Int32`

#### Scenario: Operands of distinct numeric types with no safe common type
- **WHEN** `UInt64` and `Int64` are used with `+`
- **THEN** compilation fails because no type can represent every value of both exactly

#### Scenario: Narrowing conversion proven safe at compile time
- **WHEN** `mut a: Int8 = 5;` is written with a constant `Int32` literal
- **THEN** the assignment succeeds because `5` is known to fit in `Int8`

#### Scenario: Widening across signedness is safe
- **WHEN** `mut a: Int16 = (5 as UInt8);` is written
- **THEN** the assignment succeeds because every `UInt8` value fits in `Int16`
