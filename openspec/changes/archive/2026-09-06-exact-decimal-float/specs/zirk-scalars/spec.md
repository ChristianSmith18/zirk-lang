## MODIFIED Requirements

### Requirement: `Float` family without a valid `NaN`

The type system SHALL recognize `Float` as an exact base-ten decimal scalar (see
the `exact-decimal-arithmetic` capability) and SHALL recognize `BinaryFloat16`,
`BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`, with `BinaryFloat` as an
alias for `BinaryFloat64`, as the IEEE 754 binary family. For a `BinaryFloat`
value, an operation that would produce `NaN` under IEEE 754 SHALL instead be a
controlled runtime error at the point where it occurs, and positive and negative
infinity SHALL be valid, observable values. `Float` SHALL have neither `NaN` nor
infinity; a `Float` operation that would require either SHALL be a controlled
runtime error. The spellings `Float16`, `Float32`, `Float64`, and `Float128`
SHALL NOT resolve.

#### Scenario: Exact-decimal division by zero

- **WHEN** a `Float` is divided by `0.0`
- **THEN** the program raises a controlled `DivisionByZeroError` at that operation

#### Scenario: Binary division by zero

- **WHEN** a non-zero `BinaryFloat64` is divided by `0.0b`
- **THEN** the result is infinite, not an error

#### Scenario: Binary indeterminate operation

- **WHEN** a `BinaryFloat` operation would produce `NaN` under standard IEEE 754
  semantics (for example, `0.0b / 0.0b`)
- **THEN** the program terminates with a controlled error at the point of that
  operation; it does not propagate a `NaN` value

#### Scenario: Fractional literal without context

- **WHEN** a fractional literal appears without an annotation or context that
  fixes its type
- **THEN** its type is `Float` (exact base-ten decimal)

#### Scenario: Former binary spelling is redirected

- **WHEN** an annotation names `Float64`
- **THEN** a diagnostic states that the binary type is now `BinaryFloat64` and
  the exact base-ten type is `Float`

### Requirement: Conversion between integer widths

The checker SHALL allow implicit conversion from one numeric type to another when the destination can represent every value of the source type without loss, and SHALL require an explicit `as` cast for narrowing or cross-family conversions that the compiler cannot prove are safe at compile time. An integer literal SHALL adopt the width of its expected type when the value fits. An integer value or literal SHALL convert implicitly and exactly to `Float`, and a `Float` literal with zero fractional part SHALL become an integer for an integer target. Conversion between `Float` and any `BinaryFloat` width SHALL require an explicit cast or constructor in both directions.

#### Scenario: Unambiguous implicit widening

- **WHEN** an `Int8` is passed where `Int32` is expected
- **THEN** the conversion is implicit and no cast is required

#### Scenario: Integer widens to exact `Float` implicitly

- **WHEN** an `Int32` is passed where `Float` is expected
- **THEN** the conversion is implicit and the value is exact at scale 0

#### Scenario: Literal that does not fit is rejected

- **WHEN** `mut a: Int8 = 1000;` is written
- **THEN** a compile-time diagnostic is emitted because `1000` does not fit in `Int8`

#### Scenario: Exact and binary floats do not mix implicitly

- **WHEN** a `Float` value is used where `BinaryFloat64` is expected without a cast
- **THEN** a diagnostic is emitted naming the explicit conversion required
