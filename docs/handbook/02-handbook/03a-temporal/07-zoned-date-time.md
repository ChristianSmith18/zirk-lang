# ZonedDateTime

`ZonedDateTime` combines one absolute `Instant` with a `TimeZone` and the local
civil representation derived from that pair. The language surface covers
parsing, instant projection, zone changes, local-time resolution, instant
comparison, and calendar shifts through `TimeShift`.

## Parsing and instant projection

`ZonedDateTime.parse` accepts RFC 3339 text followed by a bracketed zone ID. It
checks that the textual UTC offset agrees with the bundled rule data and
returns `Result<ZonedDateTime, ParseError>`.

```zirk
import { stdout } from std.io;
import { ZonedDateTime } from std.time;

fn main(): Void {
    match ZonedDateTime.parse("2024-03-10T03:30:00-04:00[America/New_York]") {
        Ok(value) => {
            stdout.println(value.local_datetime.to_iso_string());
            stdout.println(value.offset_seconds);
        }
        Error(_) => stdout.println("invalid zoned timestamp");
    };
}
main();
```

`ZonedDateTime.from_instant(instant, zone)` and `instant.in_zone(zone)`
project an instant into a zone. `value.with_zone(zone)` changes the displayed
zone while preserving the instant.

## Resolving local civil time

A local time can map to zero, one, or two instants. `DateTime.in_zone(zone,
overlap, gap)` requires both policies and returns
`Result<ZonedDateTime, LocaleError>`:

- overlap: `reject`, `earlier`, or `later`
- gap: `reject`, `forward`, or `backward`

`forward` and `backward` shift by the size of the zone transition. The static
error type remains `LocaleError`, with runtime subtypes
`InvalidDstPolicyError`, `AmbiguousLocalTimeError`,
`NonexistentLocalTimeError`, `UnknownTimeZoneError`,
`InvalidTimeZoneError`, and `TemporalRangeError` for their corresponding
failure cases. Callers can use `is` checks to distinguish these outcomes.

`TimeShift.calendar(period, duration, overlap, gap)` keeps calendar movement
separate from elapsed time. `apply_to` first adjusts local calendar fields,
resolves DST with the stored policies, then adds the exact duration to the
instant. Applying the clock form produced by `Time ± Duration` returns
`TimeShiftError` because it has no calendar shift to apply to a zoned value.

## Delivered API

| Member | Result | Status |
| --- | --- | --- |
| `ZonedDateTime.parse(text)` | `Result<ZonedDateTime, ParseError>` | Implemented; validates offset against zone rules |
| `ZonedDateTime.from_instant(instant, zone)` | `ZonedDateTime` | Implemented |
| `instant.in_zone(zone)` | `ZonedDateTime` | Implemented |
| `value.with_zone(zone)` | `ZonedDateTime` | Implemented; preserves instant |
| `value.instant` | `Instant` | Implemented |
| `value.zone` | `TimeZone` | Implemented |
| `value.local_datetime` | `DateTime` | Implemented |
| `value.offset_seconds` | `Int32` | Implemented |
| `value.is_dst` / `value.abbreviation` | `Boolean` / `String` | Implemented |
| `value.is_before(other)`, `is_after(other)`, `is_same(other)` | `Boolean` | Implemented; compares instants |
| `local.in_zone(zone, overlap, gap)` | `Result<ZonedDateTime, LocaleError>` | Implemented; concrete DST/zone/range error subtypes |
| `shift.apply_to(zoned)` | `Result<ZonedDateTime, TimeShiftError>` | Applies calendar period, DST resolution, then exact duration |

`now`, local component replacement, period-aware DST outcome types, and direct
zoned formatting remain pending.
Native/interpreter local parity covers parsing, offset validation, projection,
zone changes, overlap selection, gap shifting, and rejection behavior. The
supported native-host matrix remains open.

---

**Previous:** [← TimeZone](06-time-zone.md) · **Next:** [ Duration](08-duration.md)
