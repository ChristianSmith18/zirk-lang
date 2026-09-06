## MODIFIED Requirements

### Requirement: Fractional literal suffixes match the renamed type families

A fractional literal suffixed with `f` or `fN` SHALL be classified as a binary float literal and its type SHALL be the corresponding `FloatN` width (`Float64` for `f`, `Float16` for `f16`, `Float32` for `f32`, `Float128` for `f128`). A fractional literal suffixed with `d` or `dN` SHALL be classified as an exact decimal literal and its type SHALL be `Decimal`. An unsuffixed fractional literal SHALL be classified as `Decimal`. The token `b` and `bN` suffixes from the previous naming scheme SHALL NOT resolve and SHALL be rejected with a diagnostic pointing to `f`/`fN`.

#### Scenario: `f` suffix resolves to `Float64`
- **WHEN** the literal `1.5f` is tokenized and typed
- **THEN** the resulting type is `Float64`

#### Scenario: `f32` suffix resolves to `Float32`
- **WHEN** the literal `1.5f32` is tokenized and typed
- **THEN** the resulting type is `Float32`

#### Scenario: `d` suffix resolves to `Decimal`
- **WHEN** the literal `1.5d` is tokenized and typed
- **THEN** the resulting type is `Decimal`

#### Scenario: Unsuffixed fractional literal resolves to `Decimal`
- **WHEN** the literal `1.5` is tokenized and typed without context
- **THEN** the resulting type is `Decimal`

#### Scenario: Old `b` suffix is rejected
- **WHEN** the literal `1.5b` is tokenized
- **THEN** a diagnostic states that the binary suffix is now `f` and the exact decimal suffix is `d`
