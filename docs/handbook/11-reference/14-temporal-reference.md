# Temporal Reference

This page describes the target temporal API contract. The delivered language
surface is narrower: `Date`, `Time`, `DateTime`, and `Duration` are available;
`Instant` has a partial language API, including checked Unix-unit factories,
`now()`, UTC rendering, ordering, and `Duration` arithmetic. Locale-aware civil
formatting is available, and `DateTime.format_locale_in_zone` provides a
named-zone formatting path with explicit daylight-saving resolution policies.
`TimeZone` supports parsing/canonicalization, database version reporting,
instant-specific offset/DST/abbreviation queries, and local projection.
`ZonedDateTime` supports checked zoned parsing, conversion to/from `Instant`,
instant-preserving zone changes, local projection, and instant comparison.
`DateTime.in_zone` resolves local values only with explicit overlap/gap
policies. `Period` provides construction, component access, ISO parsing, and
formatting; direct `Date`/`DateTime ± Period` arithmetic remains unavailable.
`TimeShift` is a unified tagged language type with clock and calendar forms;
calendar shifts apply to `ZonedDateTime` with explicit overlap/gap policies.
Localized parsing remains pending. See [Current
Limitations](../13-appendices/07-current-limitations.md) and the [feature
status catalog](../../init/ZIRK_FEATURE_STATUS.md) for implementation and
verification details.

## Constructors and parsing

| Type | Representative constructors | Parsing |
|---|---|---|
| `Date` | `Date(year, month, day)` | `Date.parse(text, format?)` |
| `Time` | `Time(hour, minute, second?, nanosecond?)` | `Time.parse(text, format?)` |
| `DateTime` | `DateTime(date, time)` or components | `DateTime.parse(text, format?)` |
| `Instant` | Unix seconds/milliseconds/nanoseconds | ISO instant parser |
| `TimeZone` | `TimeZone.parse(text)` | IANA names, `UTC`/`Z`, or fixed `±HH:MM[:SS]` offsets; returns `Result<TimeZone, LocaleError>` with typed unknown-zone/invalid-offset subtypes |
| `ZonedDateTime` | `ZonedDateTime.from_instant(instant, zone)` or `instant.in_zone(zone)` | RFC 3339 timestamp plus `[Zone/Name]`; validates the pinned zone offset |
| `Duration` | unit constructors or named components | compact and ISO 8601 duration parsers |
| `Period` | years/months/weeks/days | ISO 8601 period parser |

## Legal composition

| Left | Right | Operation | Result |
|---|---|---|---|
| `Date` | `Time` | composition | `DateTime` |
| `DateTime` | `TimeZone` + overlap and gap policy strings | `in_zone` | `Result<ZonedDateTime, LocaleError>` |
| `Instant` | `TimeZone` | projection | `ZonedDateTime` |
| `ZonedDateTime` | calendar `TimeShift` | `apply_to` | `Result<ZonedDateTime, TimeShiftError>` |
| `Time`, `DateTime`, `Instant`, `ZonedDateTime` | `Duration` | `+` / `-` | left type, or `TimeShift` for `Time` |
| compatible timeline values | same family | `-` | `Duration` where defined |

Unsupported combinations are errors; Zirk never guesses a zone, calendar anchor, or DST policy.

## Duration units

`Duration` is exact, signed, and conceptually backed by a checked `Int128` nanosecond count. Constructors cover nanoseconds, microseconds, milliseconds, seconds, minutes, hours, days, and weeks. Total-unit methods return `Float`; whole-unit methods return integers. APIs that wait or allocate a timeout may reject a negative duration even though the type itself permits one.

## DST policies

Ambiguous or nonexistent local times return `LocaleError` unless the caller explicitly chooses `earlier` or `later` for an overlap, and `forward` or `backward` for a gap. Both policy arguments are required; use `reject` to request failure. Zone conversion preserves the instant; replacing local components reconstructs a local time and therefore needs the policy again.

## Errors

Construction and arithmetic use controlled errors for invalid components, invalid zones, parse/format mismatch, ambiguity, nonexistent local time, unsupported composition, and temporal overflow. Calendar `Period` addition clamps an invalid end-of-month date by default; `add_strict()` rejects it.

For worked examples, read [Temporal Types](../02-handbook/03a-temporal/README.md).

---

**Previous:** [← Type Member Index](13-type-member-index.md) · **Next:** [ Explanations](../12-explanations/README.md)
