## REMOVED Requirements

### Requirement: 'Float' family without a valid 'NaN'

**Reason**: The names `Float` and `BinaryFloatN` are being replaced with `Decimal`/`Dec` and `FloatN` to match the conventional meaning of `Float` as a binary floating-point type.

**Migration**: Use `Decimal` for the exact base-ten scalar and `FloatN` for the IEEE 754 binary family. `Float` is now an alias for `Float64`.

## ADDED Requirements

### Requirement: Fractional scalar family names

The type system SHALL recognize `Decimal` and its alias `Dec` as the exact base-ten decimal scalar. It SHALL recognize `Float16`, `Float32`, `Float64`, and `Float128` as the IEEE 754 binary family, with `Float` as an alias for `Float64`. The spellings `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`, and `BinaryFloat` SHALL NOT resolve. The type `Decimal` SHALL have neither `NaN` nor `Infinity`; a `Decimal` operation that would require either SHALL be a controlled runtime error. For a `Float` value, `NaN` SHALL NOT be valid, but infinities are valid; an operation that would produce `NaN` under IEEE 754 SHALL be a controlled runtime error.

#### Scenario: Exact-decimal division by zero
- **WHEN** a `Decimal` is divided by `0.0`
- **THEN** the program raises a controlled `DivisionByZeroError` at that operation

#### Scenario: Binary division by zero
- **WHEN** a non-zero `Float64` is divided by `0.0f` or `0.0f64`
- **THEN** the result is infinite, not an error

#### Scenario: Binary indeterminate operation
- **WHEN** a `Float` operation would produce `NaN` under standard IEEE 754 semantics (for example, `0.0f / 0.0f`)
- **THEN** the program terminates with a controlled error at the point of that operation; it does not propagate a `NaN` value

#### Scenario: Fractional literal without context
- **WHEN** a fractional literal appears without an annotation or context that fixes its type
- **THEN** its type is `Decimal` (exact base-ten decimal)

#### Scenario: Former binary spelling is redirected
- **WHEN** an annotation names `BinaryFloat64`
- **THEN** a diagnostic states that the binary type is now `Float64` and the exact base-ten type is `Decimal`

#### Scenario: Former exact spelling is redirected
- **WHEN** an annotation names `Float` in a context where `Decimal` is expected after the rename
- **THEN** a diagnostic states that the exact base-ten type is now `Decimal` and the binary type is `FloatN`
