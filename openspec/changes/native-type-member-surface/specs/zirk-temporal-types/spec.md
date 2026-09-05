# zirk-temporal-types delta

## ADDED Requirements

### Requirement: Duration sign and magnitude members

A `Duration` SHALL expose `abs()`, `sign()`, `is_zero()`, `is_positive()`,
and `is_negative()` in addition to its existing literal/arithmetic surface.

#### Scenario: Magnitude

- **WHEN** `(-5s).abs()` is evaluated
- **THEN** the result is `5s`

#### Scenario: Sign tests

- **WHEN** `(-1s).is_negative()`, `(0s).is_zero()`, and
  `(2s).is_positive()` run
- **THEN** all produce `true`

#### Scenario: Sign value

- **WHEN** `(-3s).sign()` is evaluated
- **THEN** the result is `-1` as `Int32`
