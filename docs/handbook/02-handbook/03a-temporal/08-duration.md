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

Days are exactly 24 hours and weeks exactly 7 days. Months/years belong to
Period because their elapsed length depends on calendar context.

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

`abs()`, `sign()`, `is_zero()`, `is_positive()`, `is_negative()`, `min()`,
`max()`, `clamp()`, `round(unit)`, `floor(unit)`, `ceil(unit)`,
`truncate(unit)`, `format()`, `humanize()`, `to_iso_string()` and
`to_string()` complete the core surface. Humanization accepts locale and unit
limits and is presentation, never parsing input back implicitly.

---

**Previous:** [← ZonedDateTime](07-zoned-date-time.md) · **Next:** [ Period](09-period.md)
