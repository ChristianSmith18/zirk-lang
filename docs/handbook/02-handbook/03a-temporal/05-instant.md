# Instant

`Instant` is one absolute point on the UTC timeline, independent of a calendar
display or time zone. Its conceptual storage is an `Int128` nanosecond offset
from Unix epoch; physical representation is an implementation detail.

`from_unix_seconds`, `from_unix_milliseconds`, and `from_unix_nanoseconds`
return `Result<Instant, ArithmeticOverflowError>` and check the implementation's
supported timeline range. `Instant.parse` requires an explicit `Z` or UTC
offset. `Instant.now()` reads the native system wall clock and returns its UTC
timeline point. The browser interpreter denies ambient clock access.
Unix-unit properties return whole units rounded down for negative fractional
timestamps; `unix_nanoseconds` preserves the full `Int128` value.

## Operators and API

Instant supports equality/order, `Instant ± Duration -> Instant`, and
`Instant - Instant -> Duration`. It does not accept Period because a month has
no exact timeline length without calendar and zone context.

```zirk
import { stdout } from std.io;
import { Instant } from std.time;

fn main(): Void {
    inmut started = Instant.now();
    inmut later = started + 250ms;
    inmut latency = later - started; // signed Duration
    stdout.println(latency);
}
main();
```

`in_zone(zone)` creates a `ZonedDateTime` without changing the represented
point. `to_iso_string()` emits the same canonical UTC RFC 3339 form as
`to_string()`.
Elapsed measurements should use the monotonic clock API rather than wall-clock
Instant when system-clock adjustment would distort measurement.

## API

One absolute point on the UTC timeline; conceptual `Int128` nanosecond offset
from the Unix epoch.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `unix_seconds` | `Int64` | Whole seconds since epoch, rounded down | delivered |
| `unix_milliseconds` | `Int64` | Whole milliseconds since epoch, rounded down | delivered |
| `unix_nanoseconds` | `Int128` | Exact nanoseconds since epoch | delivered |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Instant.now()` | `Instant` | Current wall-clock instant | delivered on native targets |
| `Instant.from_unix_seconds(Int64)` | `Result<Instant, ArithmeticOverflowError>` | Checked construction | delivered |
| `Instant.from_unix_milliseconds(Int64)` | `Result<Instant, ArithmeticOverflowError>` | Checked construction | delivered |
| `Instant.from_unix_nanoseconds(Int128)` | `Result<Instant, ArithmeticOverflowError>` | Checked construction with exact nanosecond precision | delivered |
| `Instant.parse(text)` | `Result<Instant, ParseError>` | RFC 3339 instant; requires `Z` or an offset | delivered |
| `i + duration` / `i - duration` | `Instant` | Exact shift | delivered |
| `i - other` | `Duration` | Signed elapsed difference | delivered |
| `i.is_before(other)` / `is_after` / `is_same` | `Boolean` | Timeline ordering | delivered |
| `i.in_zone(zone)` | `ZonedDateTime` | Projection; preserves the instant | delivered |
| `i.utc()` | `ZonedDateTime` | `in_zone(TimeZone.UTC)` | target API; use `TimeZone.parse("UTC")` with `in_zone` today |
| `i.to_iso_string()` | `String` | Canonical UTC RFC 3339 form | delivered |
| `i.to_string()` | `String` | Canonical UTC RFC 3339 form | delivered |

`Instant` does not accept `Period` — a month has no exact timeline length
without calendar and zone context. Elapsed measurement should use the monotonic
clock API, not wall-clock `Instant.now()`.

### Examples

```zirk
import { stdout } from std.io;

fn main(): Void {
    match Instant.from_unix_seconds(0) {
        Ok(epoch) => {
            inmut later = epoch + 250ms;
            stdout.println(later > epoch);
        }
        Error(_) => stdout.println("timestamp out of range");
    };
    match Instant.parse("2026-08-15T18:30:00Z") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(_) => stdout.println("invalid timestamp");
    };
}
main();
```

---

**Previous:** [← DateTime](04-date-time.md) · **Next:** [ TimeZone](06-time-zone.md)
