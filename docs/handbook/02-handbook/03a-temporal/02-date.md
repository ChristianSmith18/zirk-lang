# Date

`Date` is a proleptic Gregorian calendar date with no time, offset or zone.
Birthdays, civil deadlines and reporting days should not be forced through a
midnight timestamp.

```zirk
inmut release = Date(2026, 8, 15);
inmut iso = Date("2026-08-15");
inmut local_today = Date.today("America/Santiago");
```

Month numbers are 1-based. Invalid dates never normalize silently:

```zirk
Date(2026, 2, 29); // InvalidDate: 2026 is not leap
Date(2026, 13, 1); // InvalidDate: month out of range
```

## Properties and API

`year`, `month`, `day`, `day_of_week`, `day_of_year`, `week_of_year`,
`quarter`, `days_in_month`, and `is_leap_year` expose calendar facts.
`day_of_week` is a `Weekday` enum, not a magic number.

`with_year()`, `with_month()`, `with_day()`, `start_of()`, `end_of()`,
`is_before()`, `is_after()`, `is_same()`, `is_between()`, `is_weekday()`,
`is_weekend()`, `month_name()`, `format()`, `to_iso_string()`, `at()` and
`to_datetime()` return values or controlled errors.

## Arithmetic

`Date ± Period` returns Date; `Date - Date` returns an exact day Duration;
`Date + Time` returns DateTime. Adding a month clamps to the final valid target
day:

```zirk
Date(2026, 1, 31) + Period.months(1); // 2026-02-28
```

`add_strict()` rejects the absent February 31. `Date + Duration` is invalid
because a timeline quantity needs a clock reading and zone.

## API

A proleptic Gregorian calendar date with no time, offset, or zone.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `year` | `Int32` | Proleptic year | specified |
| `month` | `Int32` | Month, 1–12 | specified |
| `day` | `Int32` | Day of month, 1–31 | specified |
| `day_of_week` | `Weekday` | `Weekday` enum value, not a number | specified |
| `day_of_year` | `Int32` | Ordinal day within the year | specified |
| `week_of_year` | `Int32` | ISO week number | specified |
| `quarter` | `Int32` | Quarter, 1–4 | specified |
| `days_in_month` | `Int32` | Length of the containing month | specified |
| `is_leap_year` | `Boolean` | Leap-year test for `year` | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Date(year, month, day)` | `Date` | Validated construction; invalid input is a controlled error, never normalized | specified |
| `Date(text)` | `Date` | Canonical ISO `YYYY-MM-DD` construction | specified |
| `Date.parse(text, format?)` | `Result<Date, ParseError>` | Explicit-pattern parsing | specified |
| `Date.today(zone?)` | `Date` | Current date in `zone` (or the system zone) | specified |
| `d.with_year(y)` / `d.with_month(m)` / `d.with_day(d)` | `Date` | Component replacement; invalid result is a controlled error | specified |
| `d.start_of(unit)` / `d.end_of(unit)` | `Date` | Boundary of `unit` (`"month"`, `"year"`, …) | specified |
| `d.is_before(other)` / `d.is_after(other)` / `d.is_same(other)` | `Boolean` | Calendar comparisons | specified |
| `d.is_between(start, end)` | `Boolean` | Inclusive range test | specified |
| `d.is_weekday()` / `d.is_weekend()` | `Boolean` | Weekday classification | specified |
| `d.month_name(locale?)` | `String` | Localized month name | specified |
| `d.add(period)` / `d - period` | `Date` | Calendar arithmetic; end-of-month clamps by default | specified |
| `d.add_strict(period)` | `Result<Date, TemporalError>` | Rejects a clamped/absent target day | specified |
| `d - other` | `Duration` | Exact day difference | specified |
| `d.at(time)` / `d + time` | `DateTime` | Composition with a `Time` | specified |
| `d.to_datetime(time)` | `DateTime` | Named composition alternative | specified |
| `d.format(pattern, locale?)` | `String` | Presentation formatting; non-date tokens are an error | specified |
| `d.to_iso_string()` | `String` | Canonical `YYYY-MM-DD` | specified |
| `d.to_string()` | `String` | Default rendering | specified |

### Examples

```zirk
inmut release = Date(2026, 8, 15);
inmut iso = Date("2026-08-15");
inmut local_today = Date.today("America/Santiago");

release.day_of_week;                       // Weekday enum
Date(2026, 1, 31) + Period.months(1);      // 2026-02-28 (clamped)
Date(2026, 1, 31).add_strict(Period.months(1)); // Error — Feb 31 rejected
Date(2026, 2, 29);                         // InvalidDate
```

---

**Previous:** [← Choosing a Temporal Type](01-choosing-a-temporal-type.md) · **Next:** [ Time](03-time.md)
