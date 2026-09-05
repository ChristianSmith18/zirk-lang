# ZonedDateTime

`ZonedDateTime` combines one Instant, one TimeZone and the corresponding local
calendar/clock representation.

```zirk
inmut chile = ZonedDateTime.now("America/Santiago");
inmut parsed = ZonedDateTime.parse(
    "2026-08-15T14:30:00-04:00[America/Santiago]"
);
```

Properties include `instant`, `zone`, `offset`, `date`, `time`, local
components through nanoseconds and `is_dst`.

## Zone conversion and equality

```zirk
inmut tokyo = chile.in_zone("Asia/Tokyo");
chile == tokyo;              // true when the instant is identical
chile.zone == tokyo.zone;    // false
```

`in_zone()` preserves the instant and changes its local display. Equality and
ordering use the instant; compare zone/local properties explicitly when those
are part of the domain.

## Two meanings of “one day”

`zoned + Duration.hours(24)` advances exactly 24 hours. `zoned +
Period.days(1)` advances one calendar day while preserving the intended local
time when possible. Across DST these can differ because the local day may be
23 or 25 hours.

Parsing/constructing from local components requires an explicit policy when a
time is ambiguous or nonexistent. Absolute input with a zone and inconsistent
offset is rejected rather than silently corrected.

## API

One `Instant`, one `TimeZone`, and the corresponding local representation.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `instant` | `Instant` | Absolute point | specified |
| `zone` | `TimeZone` | Active zone | specified |
| `offset` | `Duration` | Current UTC offset | specified |
| `date` | `Date` | Local date part | specified |
| `time` | `Time` | Local time part | specified |
| `is_dst` | `Boolean` | DST active at this instant | specified |
| `year` … `nanosecond` | `Int32` | Local components | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `ZonedDateTime.now(zone?)` | `ZonedDateTime` | Current instant displayed in `zone` | specified |
| `ZonedDateTime.parse(text)` | `Result<ZonedDateTime, ParseError>` | Zoned ISO input; inconsistent offset is rejected | specified |
| `ZonedDateTime(datetime, zone, policy)` | `Result<ZonedDateTime, TemporalError>` | Local construction requiring an explicit DST policy | specified |
| `z.in_zone(zone)` | `ZonedDateTime` | Instant-preserving zone conversion | specified |
| `z.with_zone_local(zone, policy)` | `Result<ZonedDateTime, TemporalError>` | Re-localizes components; needs a policy again | specified |
| `z.with_*(component)` | `Result<ZonedDateTime, TemporalError>` | Local component replacement re-resolves DST | specified |
| `z ± Duration` | `ZonedDateTime` | Exact timeline shift | specified |
| `z ± Period` | `ZonedDateTime` | Calendar shift under the zone's rules | specified |
| `z - other` | `Duration` | Instant difference | specified |
| `z.format(pattern, locale?)` | `String` | Presentation; zone tokens allowed | specified |
| `z.to_iso_string()` / `z.to_string()` | `String` | Machine/default rendering including offset and zone ID | specified |

Equality and ordering use the instant; compare `zone` or local properties
explicitly when they are part of the domain. Ambiguous local input requires
`earlier`/`later`; nonexistent input requires `previous_valid`/`next_valid`.

### Examples

```zirk
inmut chile = ZonedDateTime.now("America/Santiago");
inmut tokyo = chile.in_zone("Asia/Tokyo");
chile == tokyo;            // true when the instant is identical
chile.zone == tokyo.zone;  // false

inmut parsed = ZonedDateTime.parse("2026-08-15T14:30:00-04:00[America/Santiago]");
```

---

**Previous:** [← TimeZone](06-time-zone.md) · **Next:** [ Duration](08-duration.md)
