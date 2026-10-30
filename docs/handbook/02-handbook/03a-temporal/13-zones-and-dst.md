# Zones and DST

An offset answers “how far from UTC now”; a TimeZone answers how offsets change
over history and future rules. `-04:00` is not equivalent to
`America/Santiago`.

`TimeZone.parse` accepts a canonicalizable IANA name, `UTC`/`Z`, or a fixed
`±HH:MM[:SS]` offset and returns a `Result<TimeZone, LocaleError>`. Unknown
IANA names and malformed offsets preserve `UnknownTimeZoneError` and
`InvalidTimeZoneError` runtime subtypes. The value
exposes `canonical_identifier`, instant-specific `offset_seconds_at(instant)`,
`is_dst_at(instant)`, `abbreviation_at(instant)`, and
`local_datetime_at(instant)`. `TimeZone.database_version()` reports the pinned
IANA rule release, currently 2025b. Fixed-offset zones retain their own
identity and are not treated as named zones.

`ZonedDateTime.parse` accepts an RFC 3339 timestamp followed by a bracketed
zone identifier, such as
`2024-03-10T03:30:00-04:00[America/New_York]`. It checks that the timestamp's
offset matches the pinned zone rules. `ZonedDateTime.from_instant(instant,
zone)`, `instant.in_zone(zone)`, and `value.with_zone(zone)` create or reproject
values while preserving the instant. The value exposes `instant`, `zone`,
`local_datetime`, `offset_seconds`, `is_dst`, and `abbreviation`; comparison
methods compare instants.

Converting a local `DateTime` requires both DST decisions:
`local.in_zone(zone, overlap, gap)` returns
`Result<ZonedDateTime, LocaleError>`. The accepted overlap strings are
`reject`, `earlier`, and `later`; gap strings are `reject`, `forward`, and
`backward`. `DateTime.format_locale_in_zone(zone, locale, style, overlap,
gap)` uses the same resolution choices and bundled ICU4X 2.4.0 / CLDR 49 data.
Invalid policies, ambiguous local times, and nonexistent local times return an
error instead of selecting an instant implicitly.

Native/interpreter parity covers zone aliases, invalid identifiers, fixed
offsets, winter/summer offsets, DST, abbreviations, instant-preserving
reprojection, RFC 3339 offset validation, overlap selection, gap shifting, and
default rejection. Typed subtypes include `AmbiguousLocalTimeError`,
`NonexistentLocalTimeError`, and `InvalidDstPolicyError`; local parity does not
close the supported native-host matrix.

## Repeated local time

When clocks move backward, one DateTime can correspond to two instants. Zirk
returns `LocaleError` unless the caller selects `earlier` or `later`.

## Skipped local time

When clocks move forward, a local range may not exist. Zirk returns
`LocaleError` unless the caller selects `forward` or `backward`.

```zirk
import { stdout } from std.io;
import { Date, DateTime, Time, TimeZone } from std.time;

fn main(): Void {
    inmut local = DateTime(Date(2024, 11, 3), Time(1, 30));
    match TimeZone.parse("America/Santiago") {
        Ok(zone) => match local.in_zone(zone, "later", "forward") {
            Ok(value) => stdout.println(value.local_datetime.to_iso_string());
            Error(_) => stdout.println("local time could not be resolved");
        };
        Error(_) => stdout.println("unknown time zone");
    };
}
main();
```

Both policies are required arguments; `"reject"` explicitly requests failure
for that case. Policies are not environment flags. Converting an existing
Instant to a zone is never ambiguous; ambiguity occurs only when local
components are used to discover an Instant.

Persist an Instant for absolute events and retain the zone ID when future local
display or recurrence depends on it. Persisting only an offset loses the rules.

---

**Previous:** [← Temporal Parsing and Formatting](12-parsing-and-formatting.md) · **Next:** [ Temporal Errors](14-temporal-errors.md)
