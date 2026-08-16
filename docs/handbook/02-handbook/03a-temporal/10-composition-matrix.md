# Temporal Composition Matrix

Composition is explicit and type-directed:

| Left | Operation | Right | Result |
| --- | --- | --- | --- |
| `Date` | `+` | `Time` | `DateTime` |
| `DateTime` | `in_zone` | `TimeZone` | `ZonedDateTime` |
| `Instant` | `in_zone` | `TimeZone` | `ZonedDateTime` |
| `Date` | `±` | `Period` | `Date` |
| `DateTime` | `±` | `Period`/`Duration` | `DateTime` |
| `Instant` | `±` | `Duration` | `Instant` |
| `ZonedDateTime` | `±` | `Period`/`Duration` | `ZonedDateTime` |
| `Time` | `±` | `Duration` | `TimeShift` |
| two timeline points | `-` | same kind | `Duration` |

Readable methods mirror operators: `date.at(time)`, `datetime.in_zone(zone)`,
`instant.in_zone(zone)`, `value.add(amount)` and `value.subtract(amount)`.

Invalid combinations are compile-time errors:

```zirk
date + duration;      // lacks clock and zone
instant + period;     // lacks calendar/zone context
zone + duration;      // zone rules are not a timeline point
date + instant;       // unrelated meanings
```

The sealed Temporal family exists for shared contracts and generic APIs, not
for dynamic operator guessing.

---

**Previous:** [← Period](09-period.md) · **Next:** [ Temporal Arithmetic](11-temporal-arithmetic.md)
