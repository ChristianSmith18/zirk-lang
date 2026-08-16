# Temporal Reference

## Constructors and parsing

| Type | Representative constructors | Parsing |
|---|---|---|
| `Date` | `Date(year, month, day)` | `Date.parse(text, format?)` |
| `Time` | `Time(hour, minute, second?, nanosecond?)` | `Time.parse(text, format?)` |
| `DateTime` | `DateTime(date, time)` or components | `DateTime.parse(text, format?)` |
| `Instant` | Unix seconds/milliseconds/nanoseconds | ISO instant parser |
| `TimeZone` | `TimeZone("Area/Location")`, `.utc`, `.system()` | canonical IANA identifier |
| `ZonedDateTime` | local `DateTime` + zone + policy, or `Instant` + zone | zoned ISO parser |
| `Duration` | unit constructors or named components | compact and ISO 8601 duration parsers |
| `Period` | years/months/weeks/days | ISO 8601 period parser |

## Legal composition

| Left | Right | Operation | Result |
|---|---|---|---|
| `Date` | `Time` | composition | `DateTime` |
| `DateTime` | `TimeZone` + ambiguity policy | localization | `ZonedDateTime` |
| `Instant` | `TimeZone` | projection | `ZonedDateTime` |
| `Date`, `DateTime`, `ZonedDateTime` | `Period` | `+` / `-` | left type |
| `Time`, `DateTime`, `Instant`, `ZonedDateTime` | `Duration` | `+` / `-` | left type, or `TimeShift` for `Time` |
| compatible timeline values | same family | `-` | `Duration` where defined |

Unsupported combinations are errors; Zirk never guesses a zone, calendar anchor, or DST policy.

## Duration units

`Duration` is exact, signed, and conceptually backed by a checked `Int128` nanosecond count. Constructors cover nanoseconds, microseconds, milliseconds, seconds, minutes, hours, days, and weeks. Total-unit methods return `Float`; whole-unit methods return integers. APIs that wait or allocate a timeout may reject a negative duration even though the type itself permits one.

## DST policies

Ambiguous or nonexistent local times fail by default. A caller must explicitly choose `earlier` or `later` for an overlap, and `previous_valid` or `next_valid` for a gap. Zone conversion preserves the instant; replacing local components reconstructs a local time and therefore needs the policy again.

## Errors

Construction and arithmetic use controlled errors for invalid components, invalid zones, parse/format mismatch, ambiguity, nonexistent local time, unsupported composition, and temporal overflow. Calendar `Period` addition clamps an invalid end-of-month date by default; `add_strict()` rejects it.

For worked examples, read [Temporal Types](../02-handbook/03a-temporal/README.md).

---

**Previous:** [← Type Member Index](13-type-member-index.md) · **Next:** [ Explanations](../12-explanations/README.md)
