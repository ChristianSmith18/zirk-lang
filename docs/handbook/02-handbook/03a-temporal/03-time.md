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

Adding/subtracting Duration returns `TimeShift` so lost days are visible:

```zirk
inmut shifted = Time(23, 30) + 2h;
shifted.time;       // Time(1, 30)
shifted.day_offset; // 1
```

A negative shift can produce `day_offset == -1`. Apply it to a Date explicitly:

```zirk
inmut result = date.add(Period.days(shifted.day_offset)).at(shifted.time);
```

`Time + Period` and `Time + TimeZone` are invalid: neither operation has enough
calendar information.

## API

A local clock reading without date or zone, validated at construction.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `hour` | `Int32` | 0–23 | specified |
| `minute` | `Int32` | 0–59 | specified |
| `second` | `Int32` | 0–59 (leap seconds per temporal policy) | specified |
| `millisecond` | `Int32` | 0–999 | specified |
| `microsecond` | `Int32` | 0–999 | specified |
| `nanosecond` | `Int32` | 0–999 | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Time(hour, minute, second?, nanosecond?)` | `Time` | Validated construction; `Time(25, 0)` is `InvalidTime` | specified |
| `Time.parse(text, format?)` | `Result<Time, ParseError>` | Explicit parsing | specified |
| `t.with_hour(h)` / `with_minute` / `with_second` / `with_nanosecond` | `Time` | Component replacement | specified |
| `t + duration` / `t - duration` | `TimeShift` | Wraps across midnight; exposes `time` and `day_offset` | specified |
| `t - other` | `Duration` | Signed same-day difference | specified |
| `t.is_before(other)` / `is_after` / `is_same` | `Boolean` | Local ordering | specified |
| `t.format(pattern, locale?)` | `String` | Presentation formatting | specified |
| `t.to_iso_string()` | `String` | Canonical ISO time | specified |
| `t.to_string()` | `String` | Default rendering | specified |

`Time + Period` and `Time + TimeZone` are invalid: neither operation has enough
calendar information.

### `TimeShift`

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `time` | `Time` | Wrapped clock reading | specified |
| `day_offset` | `Int32` | `+1`, `0`, or `-1` day carry | specified |

### Examples

```zirk
inmut shifted = Time(23, 30) + 2h;
shifted.time;        // Time(1, 30)
shifted.day_offset;  // 1

inmut result = date.add(Period.days(shifted.day_offset)).at(shifted.time);
```

---

**Previous:** [← Date](02-date.md) · **Next:** [ DateTime](04-date-time.md)
