# Temporal Types

Zirk separates calendar labels, local clock readings, absolute instants, zones,
exact elapsed quantities and calendar movements. This prevents the familiar
mistake of treating “tomorrow at 09:00”, “24 hours later” and one Unix timestamp
as interchangeable.

Start with [Choosing a Temporal Type](./01-choosing-a-temporal-type.md), then
read only the concrete types your application needs. The later chapters define
composition, operators, parsing, DST and errors across the family.

> **Implemented today** (`date-and-time-types`): `Date`, `Time` and
> `DateTime` — validated `Date(y, m, d)`/`Time(h, m, s?, ns?)`/
> `DateTime(d, t)` constructors that throw `InvalidDateError`/
> `InvalidTimeError`, the read-only `year`/`month`/`day`/`hour`/`minute`/
> `second`/`nanosecond` components (plus `date`/`time` on `DateTime`), ISO
> 8601 `to_string()`, `Date ± Int`, `Date - Date`, `Time ± Duration`
> (wrapping around midnight), `Time - Time`, `Date + Time`, total
> same-type comparisons, and the host-clock `Date.today()`,
> `Time.now_local()`/`now_utc()` and `DateTime.now_local()`/`now_utc()`.
> `Instant`, `ZonedDateTime`, `TimeZone`, `Period`, parsing, `format()`
> and the `with_*`/`is_*` helpers these pages describe remain pending — the
> chapters below document the target surface.

1. [Choosing a Temporal Type](./01-choosing-a-temporal-type.md)
2. [Date](./02-date.md)
3. [Time](./03-time.md)
4. [DateTime](./04-date-time.md)
5. [Instant](./05-instant.md)
6. [TimeZone](./06-time-zone.md)
7. [ZonedDateTime](./07-zoned-date-time.md)
8. [Duration](./08-duration.md)
9. [Period](./09-period.md)
10. [Composition Matrix](./10-composition-matrix.md)
11. [Temporal Arithmetic](./11-temporal-arithmetic.md)
12. [Parsing and Formatting](./12-parsing-and-formatting.md)
13. [Zones and DST](./13-zones-and-dst.md)
14. [Temporal Errors](./14-temporal-errors.md)

---

**Previous:** [← Regex](../03-everyday-types/12-regex.md) · **Next:** [ Choosing a Temporal Type](01-choosing-a-temporal-type.md)
