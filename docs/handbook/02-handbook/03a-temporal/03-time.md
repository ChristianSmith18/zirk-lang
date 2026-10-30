# Time

`Time` is a local clock reading without date or zone.

```zirk
import { stdout } from std.io;
import { Time } from std.time;

fn main(): Void {
    stdout.println(Time(14, 30).to_iso_string());
    stdout.println(Time(14, 30, 45).to_iso_string());
    stdout.println(Time(14, 30, 45, 250_000_000).to_iso_string());
}
main();
```

Components are validated; `Time(25, 0)` is `InvalidTime`, never tomorrow at
01:00. Properties are `hour`, `minute`, `second`, `millisecond`, `microsecond`
and `nanosecond`.

Time supports equality and local ordering, `with_*` component replacements,
explicit parsing, formatting and ISO output. Subtracting two Time values yields
a signed Duration under their same-day interpretation.

## Crossing midnight

`Time ± Duration` returns the clock form of `TimeShift`. It preserves the
wrapped `Time`, the signed `day_offset`, and the signed exact `duration`:

```zirk
inmut shifted = Time(23, 30) + 2h; // 01:30, with day_offset == 1
```

`Time + Period` and `Time + TimeZone` are invalid: neither operation has enough
calendar information.

## API

A local clock reading without date or zone, validated at construction.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `hour` | `Int32` | 0–23 | implemented |
| `minute` | `Int32` | 0–59 | implemented |
| `second` | `Int32` | 0–59 (leap seconds per temporal policy) | implemented |
| `millisecond` | `Int32` | Milliseconds within the second, 0–999 | implemented |
| `microsecond` | `Int32` | Microseconds within the millisecond, 0–999 | implemented |
| `nanosecond` | `Int64` | Sub-second nanosecond fraction, 0–999_999_999 | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Time(hour, minute, second?, nanosecond?)` | `Time` | Validated construction; `Time(25, 0)` throws `InvalidTimeError` | implemented |
| `Time.parse(text)` | `Result<Time, ParseError>` | Strict ISO `HH:MM[:SS[.frac]]` parsing | implemented |
| `Time.parse(text, format?)` | `Result<Time, ParseError>` | Explicit-pattern parsing | not implemented; strict ISO parsing is available |
| `t.with_hour(h)` / `with_minute` / `with_second` / `with_nanosecond` | `Time` | Component replacement; an invalid result throws `InvalidTimeError` | implemented |
| `t + duration` / `t - duration` | `TimeShift` | Clock form preserves wrapped time, signed duration, and signed day carry | implemented |
| `t - other` | `Duration` | Signed same-day difference | implemented |
| `t.is_before(other)` / `is_after` / `is_same` | `Boolean` | Local ordering | implemented |
| `t.is_same_or_before` / `is_same_or_after` / `is_between` | `Boolean` | Inclusive ordering and range | implemented |
| `t.start_of(unit)` / `t.end_of(unit)` | `Result<Time, ParseError>` | Boundary of `unit` — `"day"`, `"hour"`, `"minute"`, `"second"` | implemented |
| `t.format(pattern)` | `String` | Pattern formatting (`YYYY`, `MM`, `DD`, `HH`, `mm`, `ss`, `SSS`); unrecognized text is literal | implemented |
| `t.format_locale(locale, style)` | `Result<String, LocaleError>` | Explicit-locale CLDR formatting with short/medium/long/full styles | implemented |
| `t.to_iso_string()` | `String` | Canonical ISO time | implemented |
| `t.to_string()` | `String` | Default rendering (the ISO form) | implemented |

`Time + Period` and `Time + TimeZone` are invalid: neither operation has enough
calendar information.

### `TimeShift`

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `kind` | `String` | `"clock"` or `"calendar"` | implemented |
| `time` | `Time?` | Wrapped clock reading for the clock form; null for calendar shifts | implemented |
| `day_offset` | `Int32?` | Signed day carry for the clock form; null for calendar shifts | implemented |
| `period` | `Period?` | Calendar component for the calendar form; null for clock shifts | implemented |
| `duration` | `Duration` | Exact signed elapsed component shared by both forms | implemented |
| `overlap_policy` / `gap_policy` | `String?` | Explicit DST choices for the calendar form; null for clock shifts | implemented |
| `TimeShift.calendar(period, duration, overlap, gap)` | `Result<TimeShift, TimeShiftError>` | Constructs a calendar shift after validating both policy strings | implemented |
| `shift.apply_to(zoned)` | `Result<ZonedDateTime, TimeShiftError>` | Applies calendar period, resolves DST, then adds exact duration; clock form returns a typed mismatch error | implemented |
| `shift.to_string()` | `String` | Canonical source-facing form that includes the active tag and its fields; does not expose the private runtime token | implemented |

### Examples

```zirk
import { stdout } from std.io;
import { Time } from std.time;

fn main(): Void {
    inmut shifted = Time(23, 30) + 2h; // TimeShift: 01:30, day_offset == 1
    inmut clock_time: Time = shifted.time ?? Time(0, 0);
    stdout.println(clock_time.is_between(Time(0, 0), Time(6, 0)));
    stdout.println(shifted.day_offset ?? 0);
}
main();
```

---

**Previous:** [← Date](02-date.md) · **Next:** [ DateTime](04-date-time.md)
