## MODIFIED Requirements

### Requirement: Conversion rules use new scalar names

The checker SHALL allow implicit conversion from one numeric type to another when the destination can represent every value of the source type without loss, and SHALL require an explicit `as` cast for narrowing or cross-family conversions that the compiler cannot prove are safe at compile time. An integer literal SHALL adopt the width of its expected type when the value fits. An integer value or literal SHALL convert implicitly and exactly to `Decimal`, and a `Decimal` literal with zero fractional part SHALL become an integer for an integer target. Conversion between `Decimal` and any `Float` width SHALL require an explicit cast or constructor in both directions.

#### Scenario: Integer widens to exact `Decimal` implicitly
- **WHEN** an `Int32` is passed where `Decimal` is expected
- **THEN** the conversion is implicit and the value is exact at scale 0

#### Scenario: Exact and binary floats do not mix implicitly
- **WHEN** a `Decimal` value is used where `Float64` is expected without a cast
- **THEN** a diagnostic is emitted naming the explicit conversion required

#### Scenario: Fractional literal infers `Decimal` by default
- **WHEN** a fractional literal without context is assigned to `mut a = 0.1`
- **THEN** the inferred type is `Decimal`
