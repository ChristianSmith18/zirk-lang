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
| Calendar movement value | `Period` | ISO calendar components |
| Zoned calendar shift | `TimeShift` | period plus exact duration and DST policy |

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
import { stdout } from std.io;
import { Duration, Period, TimeShift } from std.time;

fn main(): Void {
    inmut today = Date.today();
    stdout.println(today);
    match TimeShift.calendar(Period.days(1), 0ns, "reject", "forward") {
        Ok(shift) => stdout.println(shift.kind);
        Error(_) => stdout.println("invalid calendar shift");
    };
}
main();
```

`Period` currently represents and parses calendar components; direct
`Date.add(Period)` is not available. Apply a calendar `TimeShift` to a
`ZonedDateTime` when zone rules and explicit DST behavior are required. No
operation mutates `today`, even if its binding is `mut`; `mut` only permits
assigning a new temporal value to the name.

---

**Previous:** [← Temporal Types](README.md) · **Next:** [ Date](02-date.md)
