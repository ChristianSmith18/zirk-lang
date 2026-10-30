# Temporal Errors

Runtime-core temporal failures have distinct error variants. At the language
boundary, ordinary ISO parsing uses `ParseError`. Zone parsing and local zone
resolution use the static `LocaleError` type and preserve concrete runtime
subtypes. Localized `Date`/`Time`/`DateTime.parse_locale` uses
`InvalidLocalizedInputError` and `AmbiguousLocalizedInputError` under
`LocaleError`; unsupported locale data remains a `LocaleError`. The language
API never guesses a DST policy silently.

| Error | Meaning |
| --- | --- |
| `InvalidDate` | impossible/out-of-range calendar components |
| `InvalidTime` | impossible/out-of-range clock components |
| `InvalidDateTime` | invalid combined local components |
| `UnknownTimeZoneError` | unknown IANA identity |
| `InvalidTimeZoneError` | malformed fixed offset or zone identity |
| `AmbiguousLocalTimeError` | local value maps to two instants and policy rejects it |
| `NonexistentLocalTimeError` | local value maps to no instant and policy rejects it |
| `InvalidDstPolicyError` | unsupported overlap or gap policy |
| `TemporalRangeError` | zone conversion is outside the supported temporal range |
| `InvalidLocalizedInputError` | localized text does not match the requested CLDR style or is invalid |
| `AmbiguousLocalizedInputError` | localized text has multiple interpretations or an unresolved two-digit year |
| `DurationOverflow` | exact quantity exceeds supported range |
| `TemporalParseError` | input does not match required form |
| `TemporalFormatError` | pattern asks for invalid/unknown fields |

`TimeZone.parse(text)` returns `Result<TimeZone, LocaleError>` and exposes
`UnknownTimeZoneError` or `InvalidTimeZoneError` at runtime. Local resolution
returns `Result<ZonedDateTime, LocaleError>` with the corresponding DST subtype.
`Date(2026, 2, 29)` and `Time(25, 0)` demonstrate checked civil construction.

Parsing APIs return typed expected failures; programmer-constructed invalid
constants can be diagnosed at compile time, and data-dependent failures occur
in a controlled runtime path. Application code should preserve the cause and
offending input in diagnostics without exposing sensitive context.

---

**Previous:** [← Zones and DST](13-zones-and-dst.md) · **Next:** [ Nullability](../04-nullability/README.md)
