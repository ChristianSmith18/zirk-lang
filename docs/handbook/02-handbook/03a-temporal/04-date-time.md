# DateTime

`DateTime` combines a Date and Time but deliberately has no zone. It represents
“2026-08-15 at 14:30” as entered locally, not one universal instant.

```zirk
inmut local = DateTime(Date(2026, 8, 15), Time(14, 30));
inmut composed = Date(2026, 8, 15) + Time(14, 30);
```

It exposes `date`, `time` and their component properties through nanoseconds.
`with_date()`, `with_time()`, component replacement, parsing, formatting,
`start_of()` and `end_of()` return new DateTime values.

## Arithmetic

`DateTime ± Duration` moves the local components by the exact quantity and
`DateTime - DateTime` yields the local-component `Duration` — not a globally
meaningful elapsed time across zones. Calendar `Period` arithmetic is
specified for a later phase.

## Acquiring a zone

```zirk
inmut zoned = local.in_zone(TimeZone("America/Santiago"));
```

Zone assignment can fail if the local time is skipped or repeated by DST. Zirk
rejects without an explicit policy rather than guessing. Once resolved, the
result is ZonedDateTime and has an absolute `instant`.

Do not record globally ordered events as bare DateTime. Use Instant or
ZonedDateTime once the zone is known.

## API

A `Date` + `Time` pair, deliberately without a zone.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `date` | `Date` | Date part | implemented |
| `time` | `Time` | Time part | implemented |
| `year` … `second` | `Int32` | All `Date`/`Time` component properties | implemented |
| `day_of_week` … `days_in_year` | `Int32` | All `Date` calendrical properties | implemented |
| `is_leap_year` | `Boolean` | Leap-year test | implemented |
| `millisecond` / `microsecond` | `Int32` | Sub-second components | implemented |
| `nanosecond` | `Int64` | Sub-second nanosecond fraction | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `DateTime(date, time)` | `DateTime` | Composition construction | implemented |
| `DateTime.parse(text)` | `Result<DateTime, ParseError>` | Strict ISO parsing | implemented |
| `date + time` | `DateTime` | Composition | implemented |
| `DateTime.parse(text, format)` | `Result<DateTime, ParseError>` | Explicit-pattern parsing | specified (Phase 7) |
| `dt.with_date(date)` / `dt.with_time(time)` | `DateTime` | Part replacement | implemented |
| `dt.with_year`/`with_month`/`with_day` | `DateTime` | Date-component replacement; invalid result throws `InvalidDateError` | implemented |
| `dt.with_hour`/`with_minute`/`with_second`/`with_nanosecond` | `DateTime` | Time-component replacement; invalid result throws `InvalidTimeError` | implemented |
| `dt.start_of(unit)` / `dt.end_of(unit)` | `Result<DateTime, ParseError>` | Boundary of `unit` — date units (`"year"`, `"month"`, `"week"`, `"day"`) and clock units (`"hour"`, `"minute"`, `"second"`) | implemented |
| `dt ± Duration` | `DateTime` | Exact local-component shift | implemented |
| `dt ± Period` | `DateTime` | Calendar arithmetic preserving local clock | specified (Phase 7) |
| `dt - other` | `Duration` | Local component difference — not a global elapsed time | implemented |
| `dt.in_zone(zone, policy?)` | `Result<ZonedDateTime, TemporalError>` | Localization; fails without an explicit DST policy on gap/overlap | specified (Phase 7) |
| `dt.format(pattern)` | `String` | Pattern formatting (`YYYY`/`MM`/`DD`/`HH`/`mm`/`ss`/`SSS`); unrecognized text is literal | implemented |
| `dt.format(pattern, locale)` | `String` | Localized formatting | specified (Phase 7) |
| `dt.to_iso_string()` / `dt.to_string()` | `String` | Machine/default rendering (the ISO form) | implemented |
| `dt.is_before`/`is_after`/`is_same`/`is_same_or_before`/`is_same_or_after`/`is_between` | `Boolean` | Local ordering and range | implemented |
| `dt.is_weekday()` / `dt.is_weekend()` | `Boolean` | Weekday classification | implemented |

### Examples

```zirk
inmut local = DateTime(Date(2026, 8, 15), Time(14, 30));
inmut composed = Date(2026, 8, 15) + Time(14, 30);
local.start_of("month").unwrap();   // 2026-08-01T00:00
local + 2h;                         // 2026-08-15T16:30
// `local.in_zone(...)`: specified — `TimeZone` arrives in Phase 7.
```

---

**Previous:** [← Time](03-time.md) · **Next:** [ Instant](05-instant.md)
