# Time

`Time` is a local clock reading without date or zone.

```zirk
Time(14, 30)
Time(14, 30, 45)
Time(14, 30, 45, 250_000_000)
```

Components are validated; `Time(25, 0)` is `InvalidTime`, never tomorrow at
01:00. Properties are `hour`, `minute`, `second`, `millisecond`, `microsecond`
and `nanosecond`.

Time supports equality and local ordering, `with_*` component replacements,
explicit parsing, formatting and ISO output. Subtracting two Time values yields
a signed Duration under their same-day interpretation.

## Crossing midnight

`Time ± Duration` wraps modulo one day — `Time(23, 30) + 2h` is `01:30`. The
current phase returns the wrapped `Time` directly; a `TimeShift` carrying the
visible `day_offset` is specified for a later phase:

```zirk
inmut shifted = Time(23, 30) + 2h; // 01:30 — day carry is dropped
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
| `Time.parse(text, format?)` | `Result<Time, ParseError>` | Explicit-pattern parsing | specified (Phase 7) |
| `t.with_hour(h)` / `with_minute` / `with_second` / `with_nanosecond` | `Time` | Component replacement; an invalid result throws `InvalidTimeError` | implemented |
| `t + duration` / `t - duration` | `Time` | Wraps modulo one day | implemented |
| `t + duration` → `TimeShift` | `TimeShift` | Wraps and exposes `time`/`day_offset` | specified (Phase 7) |
| `t - other` | `Duration` | Signed same-day difference | implemented |
| `t.is_before(other)` / `is_after` / `is_same` | `Boolean` | Local ordering | implemented |
| `t.is_same_or_before` / `is_same_or_after` / `is_between` | `Boolean` | Inclusive ordering and range | implemented |
| `t.start_of(unit)` / `t.end_of(unit)` | `Result<Time, ParseError>` | Boundary of `unit` — `"day"`, `"hour"`, `"minute"`, `"second"` | implemented |
| `t.format(pattern)` | `String` | Pattern formatting (`YYYY`, `MM`, `DD`, `HH`, `mm`, `ss`, `SSS`); unrecognized text is literal | implemented |
| `t.format(pattern, locale)` | `String` | Localized formatting | specified (Phase 7) |
| `t.to_iso_string()` | `String` | Canonical ISO time | implemented |
| `t.to_string()` | `String` | Default rendering (the ISO form) | implemented |

`Time + Period` and `Time + TimeZone` are invalid: neither operation has enough
calendar information.

### `TimeShift`

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `time` | `Time` | Wrapped clock reading | specified |
| `day_offset` | `Int32` | `+1`, `0`, or `-1` day carry | specified |

### Examples

```zirk
inmut shifted = Time(23, 30) + 2h;   // 01:30 — wraps modulo one day
inmut bounds = shifted.start_of("hour").unwrap(); // 01:00
shifted.is_between(Time(0, 0), Time(6, 0));       // true
```

---

**Previous:** [← Date](02-date.md) · **Next:** [ DateTime](04-date-time.md)
