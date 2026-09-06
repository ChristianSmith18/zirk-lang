# Duration

`Duration` is a signed, exact, oriented timeline quantity with nanosecond
precision and conceptual `Int128` range. Positive values move forward; negative
values move backward.

## Literals and construction

```zirk
10ns
10us
500ms
5s
5m
2h
1d
2w

Duration(hours: 2, minutes: 30)
Duration.parse("2h 30m")
Duration("PT2H30M")
```

Supported suffixes are `ns`, `us`, `ms`, `s`, `m`, `h`, `d` and `w`. Days are
exactly 24 hours and weeks exactly 7 days. Months/years belong to `Period`
because their elapsed length depends on calendar context.

## Signed differences

```zirk
inmut remaining = deadline - Instant.now();
if remaining.is_negative() {
    stdout.println("expired {remaining.abs()} ago");
}
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
Duration/Duration ratio returning Float; Duration remainder; equality/order;
and compound assignments.

```zirk
2h + 30m; // 2h30m
3 * 2h;   // 6h
3h / 2;   // 1h30m
3h / 30m; // 6.0: Float64
65m % 1h; // 5m
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
| `Duration(hours:, minutes:, seconds:, …)` | `Duration` | Named-component construction | specified |
| `Duration.parse(text)` | `Result<Duration, ParseError>` | Compact (`"2h 30m"`) input | specified |
| `Duration(text)` | `Duration` | ISO 8601 input (`"PT2H30M"`) | specified |
| `d.abs()` | `Duration` | Magnitude | implemented |
| `d.sign()` | `Int32` | `-1`, `0`, `1` | implemented |
| `d.is_zero()` / `is_positive()` / `is_negative()` | `Boolean` | Sign tests | implemented |
| `d.min(other)` / `d.max(other)` / `d.clamp(lo, hi)` | `Duration` | Bounds | specified |
| `d.total_weeks()` … `d.total_nanoseconds()` | `Float64` | Complete quantity; may be fractional (`90m.total_hours() == 1.5`) | specified |
| `d.whole_weeks()` … `d.whole_nanoseconds()` | integer | Truncated-toward-zero whole units | specified |
| `d.round(unit)` / `floor(unit)` / `ceil(unit)` / `truncate(unit)` | `Duration` | Unit rounding | specified |
| `d.format(template)` | `String` | Typed-template presentation | specified |
| `d.humanize(locale:, max_units:)` | `String` | Locale-aware readable form; presentation only, never parse input | specified |
| `d.to_iso_string()` | `String` | ISO 8601 duration | specified |
| `d.to_string()` | `String` | Human-readable default used by `stdout.println` | implemented |

Operators: unary sign; `Duration ± Duration`; `×`/`÷` by integer or Float
scalar in either appropriate order; `Duration / Duration → Float64`;
`Duration % Duration`; equality/order; compound assignment. Zero division,
non-finite scalar, precision/range loss, and overflow are controlled errors.

> **Status note:** literals, the operator set, sign tests, `abs()`, `sign()`,
> component properties, `total_*`/`whole_*` and printing are delivered.
> Rounding, `format()`, `humanize()`, and `to_iso_string()` are specified
> pending the Phase 7 `std.time` delivery.

### Examples

```zirk
inmut remaining = deadline - Instant.now();
if remaining.is_negative() {
    stdout.println("expired {remaining.abs()} ago");
}

Duration(hours: 2, minutes: 30);
Duration.parse("2h 30m");
Duration("PT2H30M");
(26h + 15m).days;        // 1 — normalized components (specified)
(90m).total_hours();     // 1.5 (specified)
```

## Printing

A `Duration` value prints through `to_string()`:

```zirk
inmut timeout = 90s;
stdout.println(timeout);       // "1m 30s" or an ISO-style rendering
stdout.println("wait {timeout}");
```

`format()` accepts a typed template and `humanize()` produces a readable,
locale-aware description. `to_iso_string()` returns the ISO 8601 duration form.

---

**Previous:** [← ZonedDateTime](07-zoned-date-time.md) · **Next:** [ Period](09-period.md)
