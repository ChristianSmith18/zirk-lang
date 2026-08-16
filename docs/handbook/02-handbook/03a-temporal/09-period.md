# Period

`Period` is a calendar displacement expressed in years, months, weeks and days.
It does not have an exact nanosecond length until applied to calendar/zone
context.

```zirk
Period.years(1)
Period.months(2)
Period(weeks: 1, days: 3)
Period("P1Y2M3D")
```

Period is an immutable value supporting unary sign, addition/subtraction,
integer scaling, compound assignment on `mut` bindings and structural equality.
It has no natural order, scalar division, or context-free total seconds:

```zirk
Period.months(1) < Period.days(31); // error
Period.months(1).total_seconds();   // error
```

The comparison is unknowable without a starting calendar position and, for
elapsed time, a zone.

## Applying calendar rules

```zirk
Date(2026, 1, 31) + Period.months(1); // 2026-02-28
```

Default arithmetic clamps to the last valid target day. `add_strict()` rejects
when the original day does not exist. DateTime preserves the local clock;
ZonedDateTime additionally resolves DST under its explicit policy.

`duration_from(start, zone)` converts a Period to exact Duration only with the
required context. Different starts can produce different results for the same
Period.

---

**Previous:** [← Duration](08-duration.md) · **Next:** [ Temporal Composition Matrix](10-composition-matrix.md)
