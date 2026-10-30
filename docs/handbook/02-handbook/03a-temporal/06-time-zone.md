# TimeZone

`TimeZone` identifies a rule set for converting between absolute instants and
local civil time. A fixed offset such as `-04:00` has no historical or future
DST rules, so it remains distinct from `America/Santiago`.

## Delivered language surface

`TimeZone.parse(text)` accepts canonicalizable IANA identifiers, `UTC`/`Z`, and
fixed offsets in `±HH:MM[:SS]` form. It returns
`Result<TimeZone, LocaleError>`; unknown IANA names and malformed offsets
surface as `UnknownTimeZoneError` and `InvalidTimeZoneError` subtypes.
IANA aliases resolve to their canonical ID;
fixed offsets retain their explicit offset identity. The bundled IANA rule data
is pinned to 2025b.

```zirk
import { stdout } from std.io;
import { TimeZone } from std.time;

fn main(): Void {
    stdout.println(TimeZone.database_version());
    match TimeZone.parse("America/Santiago") {
        Ok(zone) => {
            stdout.println(zone.canonical_identifier);
            match Instant.parse("2024-07-15T12:00:00Z") {
                Ok(instant) => {
                    stdout.println(zone.offset_seconds_at(instant));
                    stdout.println(zone.is_dst_at(instant));
                    stdout.println(zone.abbreviation_at(instant));
                    stdout.println(zone.local_datetime_at(instant).to_iso_string());
                }
                Error(_) => stdout.println("instant parse failed");
            };
        }
        Error(_) => stdout.println("zone parse failed");
    };
}
main();
```

| Member | Result | Status |
| --- | --- | --- |
| `TimeZone.parse(text)` | `Result<TimeZone, LocaleError>` | Implemented; typed unknown-zone and invalid-offset subtypes |
| `TimeZone.database_version()` | `String` | Implemented; returns pinned IANA version |
| `zone.canonical_identifier` | `String` | Implemented |
| `zone.offset_seconds_at(instant)` | `Int32` | Implemented |
| `zone.is_dst_at(instant)` | `Boolean` | Implemented |
| `zone.abbreviation_at(instant)` | `String` | Implemented |
| `zone.local_datetime_at(instant)` | `DateTime` | Implemented |
| `TimeZone.UTC`, `TimeZone.system()` | `TimeZone` | Target API; not currently exposed |

The runtime and browser interpreter share the pinned rules. Local native and
interpreter parity covers aliases, invalid identifiers, fixed offsets,
seasonal transitions, abbreviations, and local projection. The full supported
native-host matrix remains open; local evidence does not establish host parity.

---

**Previous:** [← Instant](05-instant.md) · **Next:** [ ZonedDateTime](07-zoned-date-time.md)
