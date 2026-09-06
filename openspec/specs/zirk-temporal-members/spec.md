# zirk-temporal-members Specification

## Purpose
TBD - created by archiving change temporal-rich-api. Update Purpose after archive.
## Requirements
### Requirement: Civil calendrical properties

`Date` SHALL expose `day_of_week` (ISO `Int32`, 1 = Monday … 7 = Sunday),
`day_of_year`, `week_of_year` (ISO week number), `quarter` (1–4),
`days_in_month`, `days_in_year`, and `is_leap_year` as read-only
properties. `DateTime` SHALL expose the same properties projected from its
date part. `Time` and `DateTime` SHALL expose `millisecond` and
`microsecond` (`Int32`, 0–999 each) derived from the nanosecond field.

#### Scenario: Weekday is ISO numbered
- **WHEN** `Date(2026, 9, 6).day_of_week` is evaluated (a Sunday)
- **THEN** the result is `7`

#### Scenario: Leap year and month length
- **WHEN** `Date(2024, 2, 10).is_leap_year` and `Date(2024, 2, 10).days_in_month` are evaluated
- **THEN** the results are `true` and `29`

#### Scenario: Sub-second properties
- **WHEN** `Time(9, 5, 3, 123_456_789).millisecond` is evaluated
- **THEN** the result is `123`

### Requirement: Civil comparison queries

`Date`, `Time`, and `DateTime` SHALL expose `is_before(other)`,
`is_after(other)`, `is_same(other)`, `is_same_or_before(other)`,
`is_same_or_after(other)`, and `is_between(start, end)` returning
`Boolean`. `is_between` SHALL be inclusive of both endpoints. `Date` and
`DateTime` SHALL also expose `is_weekday()` and `is_weekend()`.

#### Scenario: Ordered comparisons
- **WHEN** `Date(2026, 9, 6).is_before(Date(2026, 10, 1))` is evaluated
- **THEN** the result is `true`

#### Scenario: Inclusive between
- **WHEN** `d.is_between(d, d)` is evaluated for any `Date d`
- **THEN** the result is `true`

#### Scenario: Weekend detection
- **WHEN** `Date(2026, 9, 6).is_weekend()` is evaluated (a Sunday)
- **THEN** the result is `true`

### Requirement: Validated component replacement

`Date` SHALL expose `with_year(y)`, `with_month(m)`, `with_day(d)`;
`Time` SHALL expose `with_hour`, `with_minute`, `with_second`,
`with_nanosecond`; `DateTime` SHALL expose `with_date`, `with_time`, and
every component `with_*` of both parts. Each returns a new value and
SHALL throw `InvalidDateError`/`InvalidTimeError` when the result would
be invalid — `with_*` never clamps or normalizes.

#### Scenario: Replace a component
- **WHEN** `Date(2026, 9, 6).with_month(12)` is evaluated
- **THEN** the result is `Date(2026, 12, 6)`

#### Scenario: Rejected replacement
- **WHEN** `Date(2026, 3, 31).with_month(2)` is evaluated
- **THEN** a controlled `InvalidDateError` is thrown

### Requirement: Unit boundaries

`Date`, `Time`, and `DateTime` SHALL expose `start_of(unit)` and
`end_of(unit)` returning `Result<_, ParseError>` where `unit` is a
`String`. `Date` SHALL accept `"year"`, `"month"`, `"week"`,
`"day"`; `Time` SHALL accept `"day"`, `"hour"`, `"minute"`,
`"second"`; `DateTime` SHALL accept all of them. `"week"` SHALL use the
ISO convention (Monday start). An unknown unit SHALL produce an `Error`
result, never an exception.

#### Scenario: Start of month
- **WHEN** `Date(2026, 9, 6).start_of("month")` is evaluated
- **THEN** the `Ok` payload is `Date(2026, 9, 1)`

#### Scenario: End of day
- **WHEN** `DateTime(d, t).end_of("day")` is evaluated
- **THEN** the `Ok` payload is `23:59:59.999999999` on the same date

#### Scenario: Unknown unit is a Result error
- **WHEN** `Date(2026, 9, 6).start_of("decade")` is evaluated
- **THEN** the result is `Error` carrying `ParseError`

### Requirement: ISO parsing

`Date.parse(text)`, `Time.parse(text)`, and `DateTime.parse(text)` SHALL
return `Result<_, ParseError>` over the canonical ISO 8601 forms:
`YYYY-MM-DD`; `HH:MM` or `HH:MM:SS` with optional `.fraction`; and
`YYYY-MM-DD` + `T` or space + the time form. Malformed input and
out-of-range components SHALL both produce `Error(ParseError)`.

#### Scenario: Parse a date
- **WHEN** `Date.parse("2026-09-06")` is evaluated
- **THEN** the `Ok` payload equals `Date(2026, 9, 6)`

#### Scenario: Rejected parse
- **WHEN** `Date.parse("06/09/2026")` is evaluated
- **THEN** the result is `Error` carrying `ParseError`

### Requirement: Pattern formatting

`Date`, `Time`, and `DateTime` SHALL expose `format(pattern)` returning
`String`. The minimal token set SHALL be `YYYY`, `MM`, `DD`, `HH`, `mm`,
`ss`, `SSS` (milliseconds); any other text SHALL pass through literally
so `format` never fails. `to_iso_string()` SHALL remain available as the
canonical alias of `to_string()`.

#### Scenario: Custom presentation
- **WHEN** `Date(2026, 9, 6).format("DD/MM/YYYY")` is evaluated
- **THEN** the result is `"06/09/2026"`

#### Scenario: Unknown text is literal
- **WHEN** `Date(2026, 9, 6).format("YYYY year")` is evaluated
- **THEN** the result is `"2026 year"`

