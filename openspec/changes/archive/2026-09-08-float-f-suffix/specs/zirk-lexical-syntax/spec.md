# Delta: zirk-lexical-syntax

## MODIFIED Requirements

### Requirement: Fractional literal suffixes match the renamed type families

A fractional literal suffixed with `f` or `fN` SHALL be classified as a binary float literal and its type SHALL be the corresponding `FloatN` width (`Float64` for `f` and `f64`, `Float16` for `f16`, `Float32` for `f32`, `Float128` for `f128`). An unsuffixed fractional literal SHALL be classified as `Decimal`. `Decimal` is the default fractional type and SHALL have no literal suffix — a trailing `d` keeps its existing meaning as the `Duration` days unit (`1.5d` is a `Duration`), never a decimal marker. The token `b` and `bN` suffixes from the previous naming scheme SHALL NOT resolve and SHALL be rejected with a diagnostic pointing to `f`/`fN`.

#### Scenario: `f` suffix resolves to `Float64`
- **WHEN** the literal `1.5f` is tokenized and typed
- **THEN** the resulting type is `Float64`

#### Scenario: `f32` suffix resolves to `Float32`
- **WHEN** the literal `1.5f32` is tokenized and typed
- **THEN** the resulting type is `Float32`

#### Scenario: `f128` suffix resolves to `Float128`
- **WHEN** the literal `0.1f128` is tokenized and typed
- **THEN** the resulting type is `Float128`

#### Scenario: Unsuffixed fractional literal resolves to `Decimal`
- **WHEN** the literal `1.5` is tokenized and typed without context
- **THEN** the resulting type is `Decimal`

#### Scenario: Old `b` suffix is rejected
- **WHEN** the literal `1.5b` is tokenized
- **THEN** a diagnostic states that the binary-float suffix is now `f`/`fN`

#### Scenario: `d` stays a duration unit, not a decimal suffix
- **WHEN** the literal `1.5d` is tokenized
- **THEN** it is a `Duration` literal of one and a half days; there is no decimal `d` suffix
