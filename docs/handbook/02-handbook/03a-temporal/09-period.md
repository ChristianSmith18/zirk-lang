# Period

`Period` is a calendar displacement expressed in years, months, weeks and days.
It does not have an exact nanosecond length until applied to calendar/zone
context.

```zirk
import { stdout } from std.io;
import { Period } from std.time;

fn main(): Void {
    stdout.println(Period.years(1).to_iso_string());
    stdout.println(Period.months(2).to_iso_string());
    stdout.println(Period.days(10).to_iso_string());
    match Period.parse("P1Y2M3D") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(_) => stdout.println("invalid period");
    };
}
main();
```

The currently delivered immutable value surface supports construction,
component access, strict ISO parsing, and canonical ISO formatting. Applying a
period directly to `Date` or `DateTime`, scaling/composing periods, and
converting one to an exact duration are not yet available. Calendar shifts can
be applied through `TimeShift.calendar(...)` to a `ZonedDateTime`.

## API

A calendar displacement in years, months, weeks, and days — no exact length
until applied to a calendar/zone context.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `years` | `Int32` | Year component | implemented |
| `months` | `Int32` | Month component | implemented |
| `weeks` | `Int32` | Week component | implemented |
| `days` | `Int32` | Day component | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Period(years:, months:, weeks:, days:)` | `Period` | Component construction | implemented |
| `Period.years(n)` / `.months(n)` / `.weeks(n)` / `.days(n)` | `Period` | Single-component constructors | implemented |
| `Period.parse(text)` | `Result<Period, ParseError>` | ISO 8601 period input | implemented |
| `p.to_iso_string()` / `p.to_string()` | `String` | ISO/default rendering | implemented |

### Examples

```zirk
import { stdout } from std.io;
import { Period } from std.time;

fn main(): Void {
    inmut period = Period.days(10);
    stdout.println(period.to_iso_string());
    match Period.parse("P1Y2M3D") {
        Ok(parsed) => stdout.println(parsed.to_iso_string());
        Error(_) => stdout.println("invalid period");
    };
}
main();
```

---

**Previous:** [← Duration](08-duration.md) · **Next:** [ Temporal Composition Matrix](10-composition-matrix.md)
