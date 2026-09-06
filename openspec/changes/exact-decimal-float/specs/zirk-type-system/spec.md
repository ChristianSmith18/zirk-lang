## MODIFIED Requirements

### Requirement: Float family replaces Decimal family

`Float` SHALL be an exact base-ten decimal scalar and SHALL be the inferred type
of an ordinary fractional literal. IEEE 754 binary floating point SHALL be the
`BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128` family, with
`BinaryFloat` aliasing `BinaryFloat64`. `NaN` SHALL NOT be a valid Zirk value;
indeterminate `BinaryFloat` operations SHALL produce controlled errors, and
`Float` SHALL have neither `NaN` nor infinity.

`Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, `Dec` and `Decimal` SHALL
NOT be recognized as types of the language, nor announced as types of a future
phase; the exact base-ten behavior they described is delivered under the `Float`
name. The spellings `Float16`, `Float32`, `Float64`, `Float128` SHALL NOT
resolve; the checker SHALL emit a diagnostic naming the `BinaryFloat`
replacement.

#### Scenario: Default fractional literal

- **WHEN** `1.5` has no contextual type
- **THEN** its inferred type is `Float` (exact base-ten decimal)

#### Scenario: Indeterminate infinity operation on `BinaryFloat`

- **WHEN** positive infinity is subtracted from positive infinity on `BinaryFloat64`
- **THEN** a controlled arithmetic error is produced instead of `NaN`

#### Scenario: Withdrawn Decimal family

- **WHEN** an annotation names `Decimal64`
- **THEN** an unknown-type diagnostic is emitted
- **AND** no arrival phase is announced for that name

#### Scenario: Former binary spelling is redirected

- **WHEN** an annotation names `Float32`
- **THEN** a diagnostic states that the binary type is `BinaryFloat32` and the
  exact base-ten type is `Float`

### Requirement: `Float` family and temporal types recognized as pending

The checker SHALL recognize `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`,
`BinaryFloat128`, `BinaryFloat`, the unimplemented integer widths, `Char`, and
the temporal types `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`,
`TimeZone`, `Duration`, and `Period` as language types (pending or implemented)
that carry a clear diagnostic. `Float` SHALL be a fully recognized, implemented
type. An annotation naming a former binary spelling (`Float16`, `Float32`,
`Float64`, `Float128`) SHALL produce a redirect diagnostic, not a
nonexistent-type diagnostic.

#### Scenario: Annotation with the exact `Float` type

- **WHEN** `mut ratio: Float = 0;` is declared
- **THEN** it compiles and `ratio` holds the exact decimal `0`

#### Scenario: Annotation with a former binary spelling

- **WHEN** `mut ratio: Float64 = 0;` is declared
- **THEN** the diagnostic names `BinaryFloat64` as the replacement and `Float` as
  the exact type
- **AND** it is NOT reported as a nonexistent type

#### Scenario: Annotation with a temporal type

- **WHEN** an annotation names `Instant` or `Duration`
- **THEN** the diagnostic indicates the phase of the temporal family

### Requirement: Fractional literal context and mixed arithmetic

An unannotated fractional literal SHALL have type `Float` (exact base-ten
decimal), and an arithmetic operation between an integer type and a `Float`
SHALL produce an exact `Float`. An operation mixing a `Float` operand and a
`BinaryFloat` operand SHALL be a type error naming the explicit conversion
required.

#### Scenario: Unannotated fractional literal

- **WHEN** `mut x = 1.5;` is written without a type annotation
- **THEN** `x` has type `Float`

#### Scenario: Mixed integer and `Float` arithmetic

- **WHEN** an `Int32` and a `Float` are added
- **THEN** the result has type `Float` and is exact

#### Scenario: Mixed exact and binary float arithmetic

- **WHEN** a `Float` and a `BinaryFloat64` are added
- **THEN** type checking rejects the operation and names the explicit conversion
