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

---

**Previous:** [← DateTime](04-date-time.md) · **Next:** [ TimeZone](06-time-zone.md)
