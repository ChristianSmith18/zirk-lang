# Temporal Arithmetic

Temporal arithmetic distinguishes exact elapsed movement from calendar
movement.

## Exact movement

Duration always means a fixed nanosecond quantity:

```zirk
instant + Duration.hours(24)
zoned + Duration.hours(24)
```

The latter may display a different local hour after a DST transition.

## Calendar movement

Period applies calendar fields:

```zirk
date + Period.months(1)
zoned + Period.days(1)
```

For ZonedDateTime, one calendar day preserves the intended local reading when
it exists. Its exact elapsed Duration may be 23, 24, or 25 hours.

## Differences

`Instant - Instant` and `ZonedDateTime - ZonedDateTime` compare absolute points.
`Date - Date` counts exact civil days. `Time - Time` and `DateTime - DateTime`
use their local same-context components and should not be mistaken for a zoned
elapsed measurement.

Arithmetic returns new immutable values. Overflow, invalid calendar targets,
missing context and unresolved DST are controlled errors. Use `add_strict()`
when end-of-month clamping is unacceptable.

---

**Previous:** [← Temporal Composition Matrix](10-composition-matrix.md) · **Next:** [ Temporal Parsing and Formatting](12-parsing-and-formatting.md)
