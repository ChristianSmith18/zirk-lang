# zirk-temporal-types Specification

## Purpose
Defines the sealed temporal family, exact and calendar arithmetic, time zones,
comparison domains, parsing, formatting, and temporal errors.
## Requirements
### Requirement: Current-time APIs separate values from clock sources
`Instant`, `Date`, `Time`, `DateTime`, and `ZonedDateTime` SHALL expose friendly
current-value construction with an optional injectable clock; civil values
SHALL require an explicit zone or a fallible explicitly local helper.
`Clock.system`, `Clock.monotonic`, and virtual test clocks SHALL remain distinct,
and virtual clocks SHALL NOT mutate host-global time.

#### Scenario: Current local time is unavailable
- **WHEN** `Time.now_local()` cannot discover a valid host time zone
- **THEN** it returns a typed timezone error rather than silently using UTC

### Requirement: Timer resources report scheduling behavior
One-shot timers and repeating tickers SHALL suspend tasks, observe structured
cancellation, require non-negative durations, and support explicit fixed-rate
and fixed-delay policies. Fixed-rate ticks SHALL report missed intervals rather
than enqueueing an unbounded backlog.

#### Scenario: Fixed-rate consumer misses ticks
- **WHEN** a ticker consumer resumes after multiple scheduled intervals
- **THEN** the next tick reports its scheduled and observed instants plus the
  missed count

### Requirement: Distinct temporal value types
Zirk SHALL define `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period` as distinct immutable value types in a sealed `Temporal` capability family. Membership in that family SHALL NOT make unsupported cross-type operations valid.

#### Scenario: Compose a date and time
- **WHEN** a `Date` is combined with a `Time`
- **THEN** the result is a `DateTime` with those calendar and clock components

#### Scenario: Reject meaningless composition
- **WHEN** application code attempts to add a `TimeZone` and a `Duration`
- **THEN** type checking rejects the operation

### Requirement: Calendar, local, instant, and zoned distinctions
`Date` SHALL contain only a calendar date, `Time` only a clock time, `DateTime` a date and time without a zone, `Instant` an absolute timeline point, and `ZonedDateTime` an instant plus an IANA time zone and its local representation.

#### Scenario: Local date-time needs a zone
- **WHEN** a `DateTime` is converted with `in_zone(TimeZone("America/Santiago"))`
- **THEN** the result is a `ZonedDateTime` or a controlled ambiguity/nonexistence error

#### Scenario: Same instant in two zones
- **WHEN** one `Instant` is represented in Santiago and Tokyo
- **THEN** both zoned values compare equal by instant while exposing different local components and zones

### Requirement: Exact Duration and calendar Period
`Duration` SHALL be a signed exact nanosecond-precision timeline quantity with conceptual `Int128` range. `Period` SHALL represent calendar years, months, weeks, and days and SHALL require calendar context before conversion to exact elapsed time.

#### Scenario: Negative elapsed difference
- **WHEN** an earlier instant is subtracted from a later operand in reverse order
- **THEN** the resulting `Duration` is negative and preserves `a - b == -(b - a)`

#### Scenario: Month is not an exact duration
- **WHEN** code requests total seconds from `Period.months(1)` without a starting date and zone
- **THEN** type checking rejects the operation

### Requirement: Temporal arithmetic and month adjustment
Temporal arithmetic SHALL accept only the documented composition matrix. Adding a month to a date SHALL clamp to the last valid day by default, while `add_strict` SHALL diagnose a missing target day.

#### Scenario: Comfortable month addition
- **WHEN** one month is added to `Date(2026, 1, 31)`
- **THEN** the result is `Date(2026, 2, 28)`

#### Scenario: Strict month addition
- **WHEN** `add_strict(Period.months(1))` is applied to `Date(2026, 1, 31)`
- **THEN** a controlled invalid-calendar-operation error is produced

### Requirement: Time zones and daylight-saving ambiguity
`TimeZone` SHALL use canonical IANA zone identities and rules rather than treating an offset as a zone. Converting a nonexistent or ambiguous local time SHALL reject by default and SHALL accept explicit earlier/later or previous/next-valid policies where applicable.

#### Scenario: Unknown zone
- **WHEN** `TimeZone("Mars/Olympus")` is constructed
- **THEN** a controlled invalid-time-zone error is produced

#### Scenario: Ambiguous local time
- **WHEN** a repeated local time is assigned to a zone without an ambiguity policy
- **THEN** the operation reports `AmbiguousLocalTime` rather than selecting silently

### Requirement: Temporal parsing, formatting, and errors
Every temporal type SHALL provide appropriate ISO serialization, explicit-pattern parsing/formatting, typed properties, and controlled errors for invalid values, parsing, formatting, overflow, zones, and DST resolution.

#### Scenario: Invalid leap day
- **WHEN** `Date(2026, 2, 29)` is constructed
- **THEN** an `InvalidDate` error is produced instead of normalizing the date

#### Scenario: ISO duration
- **WHEN** `Duration("PT2H30M")` is parsed
- **THEN** the result equals two hours and thirty minutes

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

