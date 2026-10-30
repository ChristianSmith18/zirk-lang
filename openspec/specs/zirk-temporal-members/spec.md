# zirk-temporal-members Specification

## Purpose
Defines the properties and methods exposed by Date, Time, and DateTime values.
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

### Requirement: Temporal formatting and parsing accept explicit locales

Temporal formatting and locale-aware parsing SHALL accept an explicit validated BCP 47 locale and SHALL use a versioned CLDR data set. Locale-sensitive operations SHALL NOT read or mutate an implicit process-global locale. The public temporal locale surface SHALL cover localized date, time, and date-time styles, month/day names, locale numbering preferences, and time-zone display names where applicable; ISO serialization and the existing locale-neutral parse/format APIs SHALL remain stable. Localized parsing SHALL report ambiguous, invalid, or unsupported input with typed errors.

Malformed tags SHALL return `LocaleError.InvalidTag`. A valid tag absent from the pinned CLDR data SHALL return `LocaleError.UnsupportedLocale`; unavailable versioned locale data SHALL return `LocaleError.DataUnavailable`. The API SHALL NOT silently fall back to a different locale or process-global settings.

#### Scenario: Format with an explicit locale

- **WHEN** the same temporal value is formatted with two supported locales
- **THEN** output follows each locale's documented date/time conventions without changing the underlying temporal value

#### Scenario: Reject an invalid or unsupported locale tag

- **WHEN** a locale-sensitive temporal API receives a malformed or unsupported BCP 47 tag
- **THEN** it returns `LocaleError.InvalidTag` for malformed syntax or `LocaleError.UnsupportedLocale` when valid data is absent

#### Scenario: Parse ambiguous localized input

- **WHEN** localized text can represent more than one valid date or time under the selected locale and parse options
- **THEN** parsing reports ambiguity rather than selecting a value silently

#### Scenario: Locale data version is observable

- **WHEN** application code queries the locale data version used for formatting
- **THEN** the bundled CLDR version is available so output changes can be audited across toolchain upgrades

#### Scenario: Report unavailable pinned locale data

- **WHEN** the requested locale-data version is unavailable
- **THEN** the operation returns `LocaleError.DataUnavailable` without consulting a host locale or network source
