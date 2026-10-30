# Duration

`Duration` is a signed, exact, oriented timeline quantity with nanosecond
precision and conceptual `Int128` range. Positive values move forward; negative
values move backward.

## Literals and construction

```zirk
import { stdout } from std.io;
import { Duration } from std.time;

fn main(): Void {
    inmut timeout: Duration = 90s;
    stdout.println(10ns);
    stdout.println(10us);
    stdout.println(500ms);
    stdout.println(5s);
    stdout.println(5m);
    stdout.println(2h);
    stdout.println(1d);
    stdout.println(2w);
    stdout.println(timeout);
}
main();
```

Supported suffixes are `ns`, `us`, `ms`, `s`, `m`, `h`, `d` and `w`. Days are
exactly 24 hours and weeks exactly 7 days. Months/years belong to `Period`
because their elapsed length depends on calendar context. Named-component
construction and text parsing are specified but not currently implemented;
use duration literals and arithmetic.

## Signed differences

```zirk
import { stdout } from std.io;
import { Instant } from std.time;

fn main(): Void {
    inmut deadline = Instant.now() + 2s;
    inmut remaining = deadline - Instant.now();
    if remaining.is_negative() {
        stdout.println("expired {remaining.abs()} ago");
    } else {
        stdout.println("time remains");
    }
}
main();
```

Signed values preserve `a - b == -(b - a)`. APIs representing forward waits,
such as `task.sleep`, reject negative input even though Duration itself supports
it.

## Components and totals

Normalized `days`, `hours`, `minutes`, `seconds`, `milliseconds`,
`microseconds`, and `nanoseconds` are components. For 26h15m, `days == 1`,
`hours == 2`, `minutes == 15`.

`total_weeks()` through `total_nanoseconds()` report the complete quantity and
may return Float (`90m.total_hours() == 1.5`). `whole_hours()` and analogous
whole-unit methods truncate toward zero and return integers.

## Operators

Duration supports unary sign; Duration addition/subtraction; multiplication by
an integer or Float scalar in either appropriate order; division by scalar;
Duration/Duration ratio returning Float; equality/order; and compound
assignments. Remainder is not available.

```zirk
import { stdout } from std.io;

fn main(): Void {
    stdout.println(2h + 30m);
    stdout.println(3 * 2h);
    stdout.println(3h / 2);
    stdout.println(3h / 30m);
}
main();
```

Zero division, non-finite scalar, precision/range loss and overflow are
controlled errors.

## API

A signed, exact, oriented timeline quantity with nanosecond precision and
conceptual `Int128` range. Days are exactly 24h, weeks exactly 7d; months and
years belong to `Period`.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `days` | `Int64` | Normalized day component | implemented |
| `hours` | `Int32` | Hour component (0–23) | implemented |
| `minutes` | `Int32` | Minute component (0–59) | implemented |
| `seconds` | `Int32` | Second component (0–59) | implemented |
| `milliseconds` | `Int32` | Millisecond component (0–999) | implemented |
| `microseconds` | `Int32` | Microsecond component (0–999) | implemented |
| `nanoseconds` | `Int32` | Nanosecond component (0–999) | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Duration(hours:, minutes:, seconds:, …)` | `Duration` | Named-component construction | not implemented |
| `Duration.parse(text)` | `Result<Duration, ParseError>` | Compact or ISO 8601 input | not implemented |
| `d.abs()` | `Duration` | Magnitude | implemented |
| `d.sign()` | `Int32` | `-1`, `0`, `1` | implemented |
| `d.is_zero()` / `is_positive()` / `is_negative()` | `Boolean` | Sign tests | implemented |
| `d.min(other)` / `d.max(other)` / `d.clamp(lo, hi)` | `Duration` | Bounds | specified |
| `d.total_weeks()` … `d.total_nanoseconds()` | `Float` | Complete quantity; may be fractional | implemented |
| `d.whole_weeks()` … `d.whole_nanoseconds()` | integer | Truncated-toward-zero whole units | implemented |
| `d.round(unit)` / `floor(unit)` / `ceil(unit)` / `truncate(unit)` | `Duration` | Unit rounding | implemented |
| `d.format(template)` | `String` | Typed-template presentation | implemented |
| `d.humanize(locale:, max_units:)` | `String` | Readable form with English unit abbreviations; `locale` is accepted but currently ignored | implemented, English only |
| `d.to_iso_string()` | `String` | ISO 8601 duration | implemented |
| `d.to_string()` | `String` | Human-readable default used by `stdout.println` | implemented |

Operators: unary sign; `Duration ± Duration`; `×`/`÷` by integer or Float
scalar in either appropriate order; `Duration / Duration → Float`;
`Duration % Duration`; equality/order; compound assignment. Zero division,
non-finite scalar, precision/range loss, and overflow are controlled errors.

> **Status note:** literals, the operator set, sign tests, `abs()`, `sign()`,
> component properties, `total_*`/`whole_*`, rounding and printing are delivered.
> All `Duration` formatting members are delivered. `humanize(locale:, ...)`
> currently uses English abbreviations; localized unit names remain pending.

### Examples

```zirk
import { stdout } from std.io;
import { Duration, Instant } from std.time;

fn main(): Void {
    inmut elapsed: Duration = 26h + 15m;
    stdout.println(elapsed.days);
    stdout.println(elapsed.hours);
    stdout.println(elapsed.minutes);
}
main();
```

## Printing

A `Duration` value prints through `to_string()`:

```zirk
import { stdout } from std.io;

fn main(): Void {
    inmut timeout = 90s;
    stdout.println(timeout); // "1m 30s" or an ISO-style rendering
    stdout.println("wait {timeout}");
}
main();
```

`format()` accepts a typed template. `humanize()` produces a readable string
with English unit abbreviations; its `locale` argument is reserved and ignored
because Duration unit-name localization is not part of the delivered temporal
formatter yet. Civil temporal locale data is available separately.
`to_iso_string()` returns the ISO
8601 duration form.

---

**Previous:** [← ZonedDateTime](07-zoned-date-time.md) · **Next:** [ Period](09-period.md)
