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

## API

A calendar displacement in years, months, weeks, and days — no exact length
until applied to a calendar/zone context.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `years` | `Int32` | Year component | specified |
| `months` | `Int32` | Month component | specified |
| `weeks` | `Int32` | Week component | specified |
| `days` | `Int32` | Day component | specified |
| `is_zero` | `Boolean` | All components zero | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Period(years:, months:, weeks:, days:)` | `Period` | Named-component construction | specified |
| `Period.years(n)` / `.months(n)` / `.weeks(n)` / `.days(n)` | `Period` | Single-unit constructors | specified |
| `Period(text)` / `Period.parse(text)` | `Period` / `Result<Period, ParseError>` | ISO 8601 period input (`"P1Y2M3D"`) | specified |
| `p + q` / `p - q` / `-p` | `Period` | Component-wise arithmetic, unary sign | specified |
| `p * n` | `Period` | Integer scaling | specified |
| `p.duration_from(start, zone)` | `Result<Duration, TemporalError>` | Exact conversion only with required context | specified |
| `p.to_iso_string()` / `p.to_string()` | `String` | ISO/default rendering | specified |

`Period` has no natural order, no scalar division, and no context-free total
seconds: `Period.months(1) < Period.days(31)` and `.total_seconds()` are errors.
`Date`/`DateTime`/`ZonedDateTime` accept `Period` arithmetic with end-of-month
clamping by default; `add_strict()` rejects.

### Examples

```zirk
Period.years(1);
Period(weeks: 1, days: 3);
Period("P1Y2M3D");
Date(2026, 1, 31) + Period.months(1);   // 2026-02-28
```

---

**Previous:** [← Duration](08-duration.md) · **Next:** [ Temporal Composition Matrix](10-composition-matrix.md)
