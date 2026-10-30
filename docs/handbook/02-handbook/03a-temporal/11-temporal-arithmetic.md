# Temporal Arithmetic

Temporal arithmetic distinguishes exact elapsed movement from calendar
movement.

## Exact movement

Duration always means a fixed nanosecond quantity:

```zirk
import { stdout } from std.io;
import { Instant } from std.time;

fn main(): Void {
    inmut instant = Instant.now();
    inmut later = instant + 24h;
    stdout.println(later - instant);
}
main();
```

`Instant` supports exact duration arithmetic. Direct `ZonedDateTime ± Duration`
is not currently exposed; a calendar `TimeShift` can combine a `Period` and an
exact `Duration` with explicit DST policies.

## Calendar movement

`Period` is currently a calendar-component value. Direct arithmetic such as
`date + Period.months(1)` is not available. For zoned calendar movement, use:

```zirk
import { stdout } from std.io;
import { Period, TimeShift, ZonedDateTime } from std.time;

fn main(): Void {
    match ZonedDateTime.parse("2024-03-09T02:30:00-05:00[America/New_York]") {
        Ok(zoned) => match TimeShift.calendar(Period.days(1), 0ns, "later", "forward") {
            Ok(shift) => match shift.apply_to(zoned) {
                Ok(result) => stdout.println(result.local_datetime.to_iso_string());
                Error(_) => stdout.println("calendar shift failed");
            };
            Error(_) => stdout.println("invalid calendar shift");
        };
        Error(_) => stdout.println("invalid zoned date-time");
    };
}
main();
```

The shift applies the period, resolves a gap or overlap under the supplied
policy, then applies the exact duration. A mismatch or invalid/out-of-range
result is returned as `TimeShiftError`.

## Differences

`Instant - Instant` and `ZonedDateTime - ZonedDateTime` compare absolute points.
`Date - Date` counts exact civil days. `Time - Time` and `DateTime - DateTime`
use their local same-context components and should not be mistaken for a zoned
elapsed measurement.

Arithmetic returns new immutable values. Overflow and unresolved DST outcomes
are controlled errors. Direct `Period` arithmetic and `add_strict()` remain
unavailable.

---

**Previous:** [← Temporal Composition Matrix](10-composition-matrix.md) · **Next:** [ Temporal Parsing and Formatting](12-parsing-and-formatting.md)
