# Delta spec: zirk-temporal-types

## MODIFIED Requirements

### Requirement: Civil temporal arithmetic

`Date` SHALL support `+`/`-` by a whole-day count and `Date - Date` SHALL yield the day difference as `Int64`. `Time` SHALL support `+`/`-` by `Duration`, wrapping modulo 24 hours. `DateTime` SHALL support `+`/`-` by `Duration` as an exact nanosecond shift of its local components, and `DateTime - DateTime` SHALL yield the exact difference as `Duration`. `Date ± Duration` SHALL promote to `DateTime`: the date is read at local midnight and the duration shifts it — `Date + 5h` is `DateTime` at 05:00 the same day, never a silently truncated `Date`. Comparisons between same-type civil values SHALL be total and calendar-correct.

#### Scenario: Add days across a month boundary
- **WHEN** `Date(2026, 1, 31) + 2` is evaluated
- **THEN** the result is `Date(2026, 2, 2)`

#### Scenario: Difference between dates
- **WHEN** `Date(2026, 9, 6) - Date(2026, 9, 1)` is evaluated
- **THEN** the result is `5` as `Int64`

#### Scenario: Time addition wraps the day
- **WHEN** `Time(23, 30) + 2h` is evaluated
- **THEN** the result is `Time(1, 30)`

#### Scenario: Date plus duration promotes to DateTime
- **WHEN** `Date(2026, 9, 6) + 5h` is evaluated
- **THEN** the result is `DateTime` equal to `DateTime(Date(2026, 9, 6), Time(5, 0))`

#### Scenario: DateTime shifts across midnight
- **WHEN** `DateTime(Date(2026, 9, 6), Time(23, 0)) + 2h` is evaluated
- **THEN** the result is `DateTime` equal to `DateTime(Date(2026, 9, 7), Time(1, 0))`

#### Scenario: DateTime difference is a Duration
- **WHEN** `DateTime(Date(2026, 9, 7), Time(0, 0)) - DateTime(Date(2026, 9, 6), Time(0, 0))` is evaluated
- **THEN** the result is a `Duration` of exactly 24 hours
