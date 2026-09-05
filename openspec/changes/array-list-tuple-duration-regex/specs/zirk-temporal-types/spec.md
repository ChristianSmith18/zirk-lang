# zirk-temporal-types Specification

## Purpose

Defines temporal value types. This change introduces `Duration`; other temporal types remain in the roadmap.

## ADDED Requirements

### Requirement: Duration primitive

`Duration` SHALL be a compiler primitive representing an exact span of time with nanosecond precision. It SHALL support literal suffixes, arithmetic, comparison, and `to_string()`. `Duration` SHALL be distinct from integer counts of time and SHALL avoid calendar ambiguity.

#### Scenario: Duration literals
- **WHEN** `inmut d: Duration = 250ms;` is declared
- **THEN** `d` represents 250 milliseconds

- **WHEN** `inmut d: Duration = 1.5s;` is declared
- **THEN** `d` represents 1500 milliseconds

- **WHEN** `inmut d: Duration = 2h;` is declared
- **THEN** `d` represents two hours in nanoseconds

### Requirement: Duration arithmetic and comparison

`Duration` SHALL support addition, subtraction, negation, multiplication by a scalar, and division by a scalar and by another `Duration`. Division of two `Duration` values SHALL yield a `Float64`. Comparisons SHALL be total.

#### Scenario: Duration arithmetic
- **WHEN** `1s + 500ms` is evaluated
- **THEN** the result is `1500ms`

#### Scenario: Duration ratio
- **WHEN** `1s / 250ms` is evaluated
- **THEN** the result is `4.0` as `Float64`

### Requirement: Duration formatting

`Duration.to_string()` SHALL produce a human-readable representation. It SHALL be lossless for exact nanosecond values and SHALL normalize larger units when possible.

#### Scenario: Duration to_string
- **WHEN** `(1500ms).to_string()` is evaluated
- **THEN** the result is `"1.5s"` or an equivalent normalized form

### Requirement: Out-of-scope temporal types

`Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, and `Period` SHALL remain unimplemented in this change. Syntactic names MAY be gated to a future phase.

#### Scenario: Date usage is rejected
- **WHEN** `Date(2026, 9, 4)` is written
- **THEN** a diagnostic indicates that `Date` is not yet available
