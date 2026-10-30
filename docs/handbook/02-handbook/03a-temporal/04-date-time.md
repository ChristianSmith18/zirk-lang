# DateTime

`DateTime` combines a Date and Time but deliberately has no zone. It represents
“2026-08-15 at 14:30” as entered locally, not one universal instant.

```zirk
import { stdout } from std.io;
import { Date, DateTime, Time } from std.time;

fn main(): Void {
    inmut local = DateTime(Date(2026, 8, 15), Time(14, 30));
    inmut composed = Date(2026, 8, 15) + Time(14, 30);
    stdout.println(local.to_iso_string());
    stdout.println(composed.to_iso_string());
}
main();
```

It exposes `date`, `time` and their component properties through nanoseconds.
`with_date()`, `with_time()`, component replacement, parsing, formatting,
`start_of()` and `end_of()` return new DateTime values.

## Arithmetic

`DateTime ± Duration` moves the local components by the exact quantity and
`DateTime - DateTime` yields the local-component `Duration` — not a globally
meaningful elapsed time across zones. Direct calendar `Period` arithmetic is
not implemented in the current language API.

## Resolving a local time in a zone

`DateTime.in_zone(zone, overlap, gap)` is available when both daylight-saving
policies are explicit. It returns `Result<ZonedDateTime, LocaleError>`; the
current language API uses `LocaleError` for rejected ambiguity, rejected gaps,
and invalid policy strings.

```zirk
import { stdout } from std.io;
import { TimeZone } from std.time;

fn main(): Void {
    match TimeZone.parse("America/Santiago") {
        Ok(zone) => {
            inmut local = DateTime(Date(2024, 11, 3), Time(1, 30));
            match local.in_zone(zone, "earlier", "reject") {
                Ok(value) => stdout.println(value.local_datetime.to_iso_string());
                Error(_) => stdout.println("local time could not be resolved");
            };
        }
        Error(_) => stdout.println("zone could not be parsed");
    };
}
main();
```

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
| `DateTime.parse(text, format)` | `Result<DateTime, ParseError>` | Explicit-pattern parsing | not implemented; strict ISO parsing is available |
| `dt.with_date(date)` / `dt.with_time(time)` | `DateTime` | Part replacement | implemented |
| `dt.with_year`/`with_month`/`with_day` | `DateTime` | Date-component replacement; invalid result throws `InvalidDateError` | implemented |
| `dt.with_hour`/`with_minute`/`with_second`/`with_nanosecond` | `DateTime` | Time-component replacement; invalid result throws `InvalidTimeError` | implemented |
| `dt.start_of(unit)` / `dt.end_of(unit)` | `Result<DateTime, ParseError>` | Boundary of `unit` — date units (`"year"`, `"month"`, `"week"`, `"day"`) and clock units (`"hour"`, `"minute"`, `"second"`) | implemented |
| `dt ± Duration` | `DateTime` | Exact local-component shift | implemented |
| `dt ± Period` | `DateTime` | Calendar arithmetic preserving local clock | not implemented |
| `dt - other` | `Duration` | Local component difference — not a global elapsed time | implemented |
| `dt.in_zone(zone, overlap, gap)` | `Result<ZonedDateTime, LocaleError>` | Local-time resolution with explicit overlap (`reject`/`earlier`/`later`) and gap (`reject`/`forward`/`backward`) choices | implemented |
| `dt.format(pattern)` | `String` | Pattern formatting (`YYYY`/`MM`/`DD`/`HH`/`mm`/`ss`/`SSS`); unrecognized text is literal | implemented |
| `dt.format_locale(locale, style)` | `Result<String, LocaleError>` | Explicit-locale CLDR formatting with short/medium/long/full styles | implemented |
| `dt.to_iso_string()` / `dt.to_string()` | `String` | Machine/default rendering (the ISO form) | implemented |
| `dt.is_before`/`is_after`/`is_same`/`is_same_or_before`/`is_same_or_after`/`is_between` | `Boolean` | Local ordering and range | implemented |
| `dt.is_weekday()` / `dt.is_weekend()` | `Boolean` | Weekday classification | implemented |

### Examples

```zirk
import { stdout } from std.io;
import { Date, DateTime, Time } from std.time;

fn main(): Void {
    inmut local = DateTime(Date(2026, 8, 15), Time(14, 30));
    inmut composed = Date(2026, 8, 15) + Time(14, 30);
    stdout.println((local + 2h).to_iso_string());
    stdout.println(composed.to_iso_string());

    match local.start_of("month") {
        Ok(month_start) => stdout.println(month_start);
        Error(_) => stdout.println("invalid unit");
    };
}
main();
```

---

**Previous:** [← Time](03-time.md) · **Next:** [ Instant](05-instant.md)
