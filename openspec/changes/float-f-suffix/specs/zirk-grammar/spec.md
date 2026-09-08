# Delta: zirk-grammar

## MODIFIED Requirements

### Requirement: Integer-width and fractional literals

The grammar SHALL recognize an integer literal as any of the signed or unsigned
widths when the context determines it. The grammar SHALL recognize a fractional
literal (with optional scientific notation) and no suffix as an exact-decimal
`Decimal` literal, and the same form with an `f`, `f16`, `f32`, `f64`, or `f128`
suffix as a `FloatN` literal. `Decimal` is the default fractional type and
SHALL have no literal suffix — a trailing `d` keeps its existing meaning as
the `Duration` days unit, never a decimal marker. The old `b` and `bN`
suffixes SHALL NOT resolve and SHALL be rejected with a diagnostic pointing to
`f`/`fN` for binary floats. `_` SHALL be accepted as a visual separator in all
numeric literals.

#### Scenario: Visual separator in a wide literal

- **WHEN** `1_000_000` is written
- **THEN** it is lexed as the integer `1000000`

#### Scenario: Scientific notation is exact

- **WHEN** `1e2` is written
- **THEN** it is lexed as an exact-decimal `Decimal` literal with value `100`

#### Scenario: Binary-float literal

- **WHEN** `1.5f32` is written
- **THEN** it is lexed as a `Float32` literal

#### Scenario: `d` is a duration unit, not a decimal suffix

- **WHEN** `1.5d` is written
- **THEN** it is lexed as a `Duration` literal of one and a half days, not a decimal literal
