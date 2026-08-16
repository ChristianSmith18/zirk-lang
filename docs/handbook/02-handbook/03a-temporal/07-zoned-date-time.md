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

---

**Previous:** [← TimeZone](06-time-zone.md) · **Next:** [ Duration](08-duration.md)
