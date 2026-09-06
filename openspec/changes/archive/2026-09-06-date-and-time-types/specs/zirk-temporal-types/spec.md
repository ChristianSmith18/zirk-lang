# Delta spec: zirk-temporal-types

## MODIFIED Requirements

### Requirement: Out-of-scope temporal types

`Instant`, `ZonedDateTime`, `TimeZone`, and `Period` SHALL remain unimplemented in this change. Syntactic names MAY be gated to a future phase. `Date`, `Time`, and `DateTime` are no longer pending: they are implemented per "Civil temporal types are implemented" below.

#### Scenario: Zone-aware type usage is rejected
- **WHEN** `Instant.now()` or `TimeZone("America/Santiago")` is written
- **THEN** a diagnostic indicates that the type is not yet available

## ADDED Requirements

### Requirement: Civil temporal types are implemented

`Date`, `Time`, and `DateTime` SHALL be available as immutable value types: construction validates calendar and clock fields, components are exposed as read-only properties, values compare totally, and `to_string()` produces ISO 8601 text. `Date(2026, 2, 29)` SHALL be a controlled `InvalidDate` error, and `Time(25, 0)` a controlled `InvalidTime` error — neither may silently normalize.

#### Scenario: Construct and read a date
- **WHEN** `Date(2026, 9, 6)` is evaluated
- **THEN** `.year` is `2026`, `.month` is `9`, `.day` is `6`, and `to_string()` is `"2026-09-06"`

#### Scenario: Reject an invalid leap day
- **WHEN** `Date(2026, 2, 29)` is constructed
- **THEN** a controlled `InvalidDate` error is produced

#### Scenario: Construct and read a time
- **WHEN** `Time(14, 30, 45)` is evaluated
- **THEN** `.hour` is `14`, `.minute` is `30`, `.second` is `45`, and `to_string()` is `"14:30:45"`

#### Scenario: Reject an out-of-range hour
- **WHEN** `Time(25, 0)` is constructed
- **THEN** a controlled `InvalidTime` error is produced

#### Scenario: Compose a date and time
- **WHEN** `Date(2026, 9, 6)` is combined with `Time(14, 30)`
- **THEN** the result is a `DateTime` whose `to_string()` is `"2026-09-06T14:30:00"`

### Requirement: Civil temporal arithmetic

`Date` SHALL support `+`/`-` by a whole-day count and `Date - Date` SHALL yield the day difference as `Int64`. `Time` SHALL support `+`/`-` by `Duration`, wrapping modulo 24 hours. Comparisons between same-type civil values SHALL be total and calendar-correct.

#### Scenario: Add days across a month boundary
- **WHEN** `Date(2026, 1, 31) + 2` is evaluated
- **THEN** the result is `Date(2026, 2, 2)`

#### Scenario: Difference between dates
- **WHEN** `Date(2026, 9, 6) - Date(2026, 9, 1)` is evaluated
- **THEN** the result is `5` as `Int64`

#### Scenario: Time addition wraps the day
- **WHEN** `Time(23, 30) + 2h` is evaluated
- **THEN** the result is `Time(1, 30)`

### Requirement: Host-clock civil constructors

`Date.today()` and `Time.now_local()` SHALL return the host's current civil date and time through the runtime, without interpreting or validating a named zone.

#### Scenario: Today is a valid date
- **WHEN** `Date.today()` is evaluated
- **THEN** the result is a `Date` whose fields form a valid calendar day
