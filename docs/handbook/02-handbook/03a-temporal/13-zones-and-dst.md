# Zones and DST

An offset answers “how far from UTC now”; a TimeZone answers how offsets change
over history and future rules. `-04:00` is not equivalent to
`America/Santiago`.

## Repeated local time

When clocks move backward, one DateTime can correspond to two instants. Zirk
reports `AmbiguousLocalTime` unless the caller selects `Earlier` or `Later`.

## Skipped local time

When clocks move forward, a local range may not exist. Zirk reports
`NonexistentLocalTime` unless the caller selects `PreviousValid` or `NextValid`.

```zirk
local.in_zone(
    zone,
    ambiguity: TimeAmbiguity.Later,
    missing: MissingLocalTime.NextValid
)
```

`Reject` is the default for both. Policies are visible typed enums, never
environment flags. Converting an existing Instant to a zone is never ambiguous;
ambiguity occurs only when local components are used to discover an Instant.

Persist an Instant for absolute events and retain the zone ID when future local
display or recurrence depends on it. Persisting only an offset loses the rules.

---

**Previous:** [← Temporal Parsing and Formatting](12-parsing-and-formatting.md) · **Next:** [ Temporal Errors](14-temporal-errors.md)
