# Choosing a Temporal Type

All temporal values are immutable value types in a sealed `Temporal` family,
but each type answers a different question.

| Need | Type | Example |
| --- | --- | --- |
| A calendar day | `Date` | birthday, invoice date |
| A clock reading | `Time` | shop opens at 09:00 |
| Local date and time, zone not yet chosen | `DateTime` | form input |
| One absolute point | `Instant` | event timestamp |
| An instant displayed under zone rules | `ZonedDateTime` | scheduled meeting |
| IANA zone rules | `TimeZone` | `America/Santiago` |
| Exact oriented elapsed time | `Duration` | timeout, latency, difference |
| Calendar movement | `Period` | one month, next year |

```text
Temporal
├── CalendarTemporal: Date, DateTime, ZonedDateTime, Period
├── ClockTemporal: Time, DateTime, ZonedDateTime
├── TimelineTemporal: Instant, ZonedDateTime, Duration
└── ZoneTemporal: TimeZone, ZonedDateTime
```

The capability grouping supports generic formatting or serialization; it does
not permit arbitrary arithmetic. `Instant + Duration` is meaningful, while
`Instant + Period` lacks calendar/zone context and is rejected.

Values have nanosecond maximum precision where a clock component exists.
Transformations return new values:

```zirk
inmut today = Date.today();
inmut tomorrow = today.add(Period.days(1));
```

No operation mutates `today`, even if its binding is `mut`; `mut` only permits
assigning a new temporal value to the name.

---

**Previous:** [← Temporal Types](README.md) · **Next:** [ Date](02-date.md)
