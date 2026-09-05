# Instant

`Instant` is one absolute point on the UTC timeline, independent of a calendar
display or time zone. Its conceptual storage is an `Int128` nanosecond offset
from Unix epoch; physical representation is an implementation detail.

```zirk
inmut now = Instant.now();
inmut parsed = Instant.parse("2026-08-15T18:30:00Z");
inmut epoch = Instant.from_unix_seconds(0);
```

Constructors and properties cover Unix seconds, milliseconds and nanoseconds.
Conversion checks range; ISO input for Instant must identify UTC/offset rather
than an unzoned local value.

## Operators and API

Instant supports equality/order, `Instant ± Duration -> Instant`, and
`Instant - Instant -> Duration`. It does not accept Period because a month has
no exact timeline length without calendar and zone context.

```zirk
inmut latency = finished - started; // signed Duration
inmut later = started + 250ms;
```

`in_zone(zone)` creates ZonedDateTime without changing the represented point;
`utc()` uses `TimeZone.UTC`. `to_iso_string()` emits an unambiguous UTC form.
Elapsed measurements should use the monotonic clock API rather than wall-clock
Instant when system-clock adjustment would distort measurement.

## API

One absolute point on the UTC timeline; conceptual `Int128` nanosecond offset
from the Unix epoch.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `unix_seconds` | `Int64` | Whole seconds since epoch | specified |
| `unix_milliseconds` | `Int64` | Milliseconds since epoch | specified |
| `unix_nanoseconds` | `Int128` | Nanoseconds since epoch | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Instant.now()` | `Instant` | Current wall-clock instant | specified |
| `Instant.from_unix_seconds(s)` | `Instant` | Checked construction | specified |
| `Instant.from_unix_milliseconds(ms)` | `Instant` | Checked construction | specified |
| `Instant.from_unix_nanoseconds(ns)` | `Instant` | Checked construction | specified |
| `Instant.parse(text)` | `Result<Instant, ParseError>` | ISO instant; requires `Z` or an offset | specified |
| `i + duration` / `i - duration` | `Instant` | Exact shift | specified |
| `i - other` | `Duration` | Signed elapsed difference | specified |
| `i.is_before(other)` / `is_after` / `is_same` | `Boolean` | Timeline ordering | specified |
| `i.in_zone(zone)` | `ZonedDateTime` | Projection; preserves the instant | specified |
| `i.utc()` | `ZonedDateTime` | `in_zone(TimeZone.UTC)` | specified |
| `i.to_iso_string()` | `String` | Unambiguous UTC form | specified |
| `i.to_string()` | `String` | Default rendering | specified |

`Instant` does not accept `Period` — a month has no exact timeline length
without calendar and zone context. Elapsed measurement should use the monotonic
clock API, not wall-clock `Instant.now()`.

### Examples

```zirk
inmut now = Instant.now();
inmut parsed = Instant.parse("2026-08-15T18:30:00Z");
inmut later = now + 250ms;
inmut latency = Instant.now() - now;   // signed Duration
```

---

**Previous:** [← DateTime](04-date-time.md) · **Next:** [ TimeZone](06-time-zone.md)
