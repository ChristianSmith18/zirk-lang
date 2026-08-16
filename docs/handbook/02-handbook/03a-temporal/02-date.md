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

---

**Previous:** [← Choosing a Temporal Type](01-choosing-a-temporal-type.md) · **Next:** [ Time](03-time.md)
