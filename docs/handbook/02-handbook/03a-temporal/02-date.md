# Date

`Date` is a proleptic Gregorian calendar date with no time, offset or zone.
Birthdays, civil deadlines and reporting days should not be forced through a
midnight timestamp.

```zirk
import { stdout } from std.io;
import { Date } from std.time;

fn main(): Void {
    inmut release = Date(2026, 8, 15);
    inmut local_today = Date.today();
    stdout.println(release.to_iso_string());
    stdout.println(local_today.to_iso_string());
    match Date.parse("2026-08-15") {
        Ok(parsed) => stdout.println(parsed.to_iso_string());
        Error(_) => stdout.println("invalid date");
    };
}
main();
```

Month numbers are 1-based. Invalid dates never normalize silently:

```text
Date(2026, 2, 29); // InvalidDate: 2026 is not leap
Date(2026, 13, 1); // InvalidDate: month out of range
```

Both constructors throw `InvalidDateError`; these lines illustrate rejected
inputs and are not a runnable program.

## Properties and API

`year`, `month`, `day`, `day_of_week`, `day_of_year`, `week_of_year`,
`quarter`, `days_in_month`, `days_in_year` and `is_leap_year` expose calendar
facts. `day_of_week` is the ISO `Int32` — `1` is Monday, `7` is Sunday. There
is no `Weekday` enum in the current API.

`with_year()`, `with_month()`, `with_day()`, `start_of()`, `end_of()`,
`is_before()`, `is_after()`, `is_same()`, `is_between()`, `is_weekday()`,
`is_weekend()`, `format()`, `format_locale()`, and `to_iso_string()` are
available. Standalone `month_name()`, `at()`, and `to_datetime()` remain
specified but are not available; use `Date + Time` for composition.

## Arithmetic

`Date ± Int` returns Date; `Date - Date` returns the `Int64` day count;
`Date + Time` returns DateTime. `Date ± Duration` promotes to `DateTime` —
the date is read at local midnight, so `Date(2026, 9, 6) + 5h` is
`2026-09-06T05:00:00`.

`Period` is available for construction, component access, ISO parsing, and
formatting. Direct `Date ± Period` arithmetic and `add_strict()` are not part
of the current language API; calendar application is available through the
calendar form of `TimeShift` on a `ZonedDateTime`.

## API

A proleptic Gregorian calendar date with no time, offset, or zone.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `year` | `Int32` | Proleptic year | implemented |
| `month` | `Int32` | Month, 1–12 | implemented |
| `day` | `Int32` | Day of month, 1–31 | implemented |
| `day_of_week` | `Int32` | ISO weekday, 1 = Monday … 7 = Sunday; no `Weekday` enum is exposed | implemented |
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
| `Date.parse(text, format?)` | `Result<Date, ParseError>` | Explicit-pattern parsing | not implemented; strict ISO parsing is available |
| `Date.today()` | `Date` | Current local date | implemented |
| `Date.today(zone)` | `Date` | Current date in an explicit zone | not implemented; `Date.today()` uses the host local date |
| `d.with_year(y)` / `d.with_month(m)` / `d.with_day(d)` | `Date` | Component replacement; an invalid result throws `InvalidDateError`, never clamps | implemented |
| `d.start_of(unit)` / `d.end_of(unit)` | `Result<Date, ParseError>` | Boundary of `unit` — `"year"`, `"month"`, `"week"` (ISO), `"day"`; an unknown unit is an `Err` | implemented |
| `d.is_before(other)` / `d.is_after(other)` / `d.is_same(other)` | `Boolean` | Calendar comparisons | implemented |
| `d.is_same_or_before(other)` / `d.is_same_or_after(other)` | `Boolean` | Inclusive comparisons | implemented |
| `d.is_between(start, end)` | `Boolean` | Inclusive range test | implemented |
| `d.is_weekday()` / `d.is_weekend()` | `Boolean` | Weekday classification | implemented |
| `d.month_name(locale?)` | `String` | Standalone localized month name | not implemented; use `format_locale(locale, style)` for localized date output |
| `d.add(period)` / `d - period` | `Date` | Calendar arithmetic | not implemented |
| `d.add_strict(period)` | `Result<Date, TemporalError>` | Rejects a clamped/absent target day | not implemented |
| `d - other` | `Int64` | Exact day difference | implemented |
| `d + int` / `d - int` | `Date` | Day-step arithmetic | implemented |
| `d + duration` / `d - duration` | `DateTime` | Read at local midnight, then shifted | implemented |
| `d + time` | `DateTime` | Composition with a `Time` | implemented |
| `d.at(time)` / `d.to_datetime(time)` | `DateTime` | Named composition alternatives | not implemented; use `d + time` |
| `d.format(pattern)` | `String` | Pattern formatting (`YYYY`, `MM`, `DD`, `HH`, `mm`, `ss`, `SSS`); unrecognized text is literal | implemented |
| `d.format_locale(locale, style)` | `Result<String, LocaleError>` | Explicit-locale CLDR formatting with short/medium/long/full styles | implemented |
| `d.to_iso_string()` | `String` | Canonical `YYYY-MM-DD` | implemented |
| `d.to_string()` | `String` | Default rendering (the ISO form) | implemented |

### Examples

```zirk
import { stdout } from std.io;
import { Date } from std.time;

fn main(): Void {
    inmut release = Date(2026, 8, 15);
    stdout.println(release.day_of_week); // Int32 ISO: 1 = Monday … 7 = Sunday
    stdout.println((release + 5h).to_iso_string()); // Date promotes to DateTime
}
main();
```

---

**Previous:** [← Choosing a Temporal Type](01-choosing-a-temporal-type.md) · **Next:** [ Time](03-time.md)
