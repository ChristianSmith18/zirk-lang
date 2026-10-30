# Temporal Composition Matrix

Composition is explicit and type-directed:

| Left | Operation | Right | Result |
| --- | --- | --- | --- |
| `Date` | `+` | `Time` | `DateTime` |
| `DateTime` | `in_zone` | `TimeZone` | `ZonedDateTime` |
| `Instant` | `in_zone` | `TimeZone` | `ZonedDateTime` |
| `DateTime` | `±` | `Duration` | `DateTime` |
| `Instant` | `±` | `Duration` | `Instant` |
| `Time` | `±` | `Duration` | `TimeShift` |
| `ZonedDateTime` | `apply_to` | calendar `TimeShift` | `Result<ZonedDateTime, TimeShiftError>` |
| two timeline points | `-` | same kind | `Duration` |

`Period` currently provides value construction, component access, ISO parsing,
and formatting; direct period arithmetic on temporal values is not available.
`TimeShift.calendar(...)` makes the calendar operation and DST policies
explicit before applying it to a zoned value.

Invalid combinations are compile-time errors:

```text
date + duration; // compile-time error: no clock or zone context
instant + period; // compile-time error: no calendar/zone context
zone + duration; // compile-time error: a zone is not a timeline point
date + instant; // compile-time error: unrelated temporal meanings
```

The sealed Temporal family exists for shared contracts and generic APIs, not
for dynamic operator guessing.

---

**Previous:** [← Period](09-period.md) · **Next:** [ Temporal Arithmetic](11-temporal-arithmetic.md)
