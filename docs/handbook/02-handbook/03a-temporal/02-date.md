# Date

`Date` is a proleptic Gregorian calendar date with no time, offset or zone.
Birthdays, civil deadlines and reporting days should not be forced through a
midnight timestamp.

```zirk
inmut release = Date(2026, 8, 15);
inmut iso = Date.parse("2026-08-15").unwrap();
inmut local_today = Date.today();
```

Month numbers are 1-based. Invalid dates never normalize silently:

```zirk
Date(2026, 2, 29); // InvalidDate: 2026 is not leap
Date(2026, 13, 1); // InvalidDate: month out of range
```

## Properties and API

`year`, `month`, `day`, `day_of_week`, `day_of_year`, `week_of_year`,
`quarter`, `days_in_month`, `days_in_year` and `is_leap_year` expose calendar
facts. `day_of_week` is the ISO `Int32` — `1` is Monday, `7` is Sunday; a
`Weekday` enum is specified for a later phase.

`with_year()`, `with_month()`, `with_day()`, `start_of()`, `end_of()`,
`is_before()`, `is_after()`, `is_same()`, `is_between()`, `is_weekday()`,
`is_weekend()`, `month_name()`, `format()`, `to_iso_string()`, `at()` and
`to_datetime()` return values or controlled errors.

## Arithmetic

`Date ± Int` returns Date; `Date - Date` returns the `Int64` day count;
`Date + Time` returns DateTime. `Date ± Duration` promotes to `DateTime` —
the date is read at local midnight, so `Date(2026, 9, 6) + 5h` is
`2026-09-06T05:00:00`.

Calendar `Period` arithmetic is specified for a later phase: adding a month
will clamp to the final valid target day (`2026-01-31 + Period.months(1)` →
`2026-02-28`), and `add_strict()` will reject the absent February 31.

## API

A proleptic Gregorian calendar date with no time, offset, or zone.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `year` | `Int32` | Proleptic year | implemented |
| `month` | `Int32` | Month, 1–12 | implemented |
| `day` | `Int32` | Day of month, 1–31 | implemented |
| `day_of_week` | `Int32` | ISO weekday, 1 = Monday … 7 = Sunday (`Weekday` enum is Phase 7) | implemented |
| `day_of_year` | `Int32` | Ordinal day within the year | implemented |
| `week_of_year` | `Int32` | ISO week number | implemented |
| `quarter` | `Int32` | Quarter, 1–4 | implemented |
| `days_in_month` | `Int32` | Length of the containing month | implemented |
| `days_in_year` | `Int32` | Length of the containing year, 365 or 366 | implemented |
| `is_leap_year` | `Boolean` | Leap-year test for `year` | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Date(year, month, day)` | `Date` | Validated construction; invalid input throws `InvalidDateError`, never normalized | implemented |
| `Date.parse(text)` | `Result<Date, ParseError>` | Strict ISO `YYYY-MM-DD` parsing | implemented |
| `Date.parse(text, format?)` | `Result<Date, ParseError>` | Explicit-pattern parsing | specified (Phase 7) |
| `Date.today()` | `Date` | Current local date | implemented |
| `Date.today(zone)` | `Date` | Current date in `zone` | specified (Phase 7) |
| `d.with_year(y)` / `d.with_month(m)` / `d.with_day(d)` | `Date` | Component replacement; an invalid result throws `InvalidDateError`, never clamps | implemented |
| `d.start_of(unit)` / `d.end_of(unit)` | `Result<Date, ParseError>` | Boundary of `unit` — `"year"`, `"month"`, `"week"` (ISO), `"day"`; an unknown unit is an `Err` | implemented |
| `d.is_before(other)` / `d.is_after(other)` / `d.is_same(other)` | `Boolean` | Calendar comparisons | implemented |
| `d.is_same_or_before(other)` / `d.is_same_or_after(other)` | `Boolean` | Inclusive comparisons | implemented |
| `d.is_between(start, end)` | `Boolean` | Inclusive range test | implemented |
| `d.is_weekday()` / `d.is_weekend()` | `Boolean` | Weekday classification | implemented |
| `d.month_name(locale?)` | `String` | Localized month name | specified (Phase 7, needs locales) |
| `d.add(period)` / `d - period` | `Date` | Calendar arithmetic; end-of-month clamps by default | specified (Phase 7, needs `Period`) |
| `d.add_strict(period)` | `Result<Date, TemporalError>` | Rejects a clamped/absent target day | specified (Phase 7) |
| `d - other` | `Int64` | Exact day difference | implemented |
| `d + int` / `d - int` | `Date` | Day-step arithmetic | implemented |
| `d + duration` / `d - duration` | `DateTime` | Read at local midnight, then shifted | implemented |
| `d + time` | `DateTime` | Composition with a `Time` | implemented |
| `d.at(time)` | `DateTime` | Named composition alternative | specified (Phase 7) |
| `d.to_datetime(time)` | `DateTime` | Named composition alternative | specified (Phase 7) |
| `d.format(pattern)` | `String` | Pattern formatting (`YYYY`, `MM`, `DD`, `HH`, `mm`, `ss`, `SSS`); unrecognized text is literal | implemented |
| `d.format(pattern, locale)` | `String` | Localized formatting | specified (Phase 7) |
| `d.to_iso_string()` | `String` | Canonical `YYYY-MM-DD` | implemented |
| `d.to_string()` | `String` | Default rendering (the ISO form) | implemented |

### Examples

```zirk
inmut release = Date(2026, 8, 15);
inmut iso = Date.parse("2026-08-15").unwrap();
inmut local_today = Date.today();

release.day_of_week;                       // Int32 ISO: 1 = Monday … 7 = Sunday
release + 5h;                              // DateTime(2026, 8, 15, 5:00) — Date promotes
Date(2026, 2, 29);                         // InvalidDateError
```

---

**Previous:** [← Choosing a Temporal Type](01-choosing-a-temporal-type.md) · **Next:** [ Time](03-time.md)
