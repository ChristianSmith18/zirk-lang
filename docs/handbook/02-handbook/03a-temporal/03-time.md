# Time

`Time` is a local clock reading without date or zone.

```zirk
Time(14, 30)
Time(14, 30, 45)
Time(14, 30, 45, 250_000_000)
```

Components are validated; `Time(25, 0)` is `InvalidTime`, never tomorrow at
01:00. Properties are `hour`, `minute`, `second`, `millisecond`, `microsecond`
and `nanosecond`.

Time supports equality and local ordering, `with_*` component replacements,
explicit parsing, formatting and ISO output. Subtracting two Time values yields
a signed Duration under their same-day interpretation.

## Crossing midnight

Adding/subtracting Duration returns `TimeShift` so lost days are visible:

```zirk
inmut shifted = Time(23, 30) + 2h;
shifted.time;       // Time(1, 30)
shifted.day_offset; // 1
```

A negative shift can produce `day_offset == -1`. Apply it to a Date explicitly:

```zirk
inmut result = date.add(Period.days(shifted.day_offset)).at(shifted.time);
```

`Time + Period` and `Time + TimeZone` are invalid: neither operation has enough
calendar information.

---

**Previous:** [← Date](02-date.md) · **Next:** [ DateTime](04-date-time.md)
