# Temporal Types

Zirk separates calendar labels, local clock readings, absolute instants, zones,
exact elapsed quantities and calendar movements. This prevents the familiar
mistake of treating “tomorrow at 09:00”, “24 hours later” and one Unix timestamp
as interchangeable.

Start with [Choosing a Temporal Type](./01-choosing-a-temporal-type.md), then
read only the concrete types your application needs. The later chapters define
composition, operators, parsing, DST and errors across the family.

> **Implemented today** (`date-and-time-types` + `temporal-rich-api`):
> `Date`, `Time` and `DateTime` — validated `Date(y, m, d)`/
> `Time(h, m, s?, ns?)`/`DateTime(d, t)` constructors that throw
> `InvalidDateError`/`InvalidTimeError`; the `year`/`month`/`day`,
> `hour`/`minute`/`second`, `millisecond`/`microsecond`/`nanosecond` and
> `date`/`time` properties; the ISO calendrical `day_of_week`,
> `day_of_year`, `week_of_year`, `quarter`, `days_in_month`,
> `days_in_year`, `is_leap_year`; the `is_before`/`is_after`/`is_same`/
> `is_same_or_before`/`is_same_or_after`/`is_between` queries plus
> `is_weekday`/`is_weekend` on `Date`/`DateTime`; the validated
> `with_*` component replacements; `start_of`/`end_of(unit)` returning
> `Result<_, ParseError>`; strict ISO `Date.parse`/`Time.parse`/
> `DateTime.parse`; `format(pattern)` with the `YYYY`/`MM`/`DD`/`HH`/
> `mm`/`ss`/`SSS` tokens; `to_iso_string`/`to_string`; `Date ± Int`,
> `Date - Date` (`Int64`), `Time ± Duration` (wrapping modulo a day),
> `Time - Time`, `Date + Time`, `DateTime ± Duration`, `Date ± Duration`
> (promoting to `DateTime` read at local midnight), `DateTime - DateTime`
> (`Duration`), total same-type comparisons, and the host-clock
> `Date.today()`, `Time.now_local()`/`now_utc()` and
> `DateTime.now_local()`/`now_utc()`. `Instant` is partially delivered with
> RFC 3339 parsing, checked Unix-unit factories/properties, ordering, exact
> `Duration` arithmetic and native `now()`. Explicit-locale formatting is
> available for civil values, and `DateTime.format_locale_in_zone` supports
> IANA/fixed-offset output with named DST policies using ICU4X 2.4.0 / CLDR 49.
> `TimeZone` and `ZonedDateTime` now have partial language APIs for pinned zone
> parsing, projection, instant-preserving conversion, and explicit local-time
> resolution. `Period` exposes construction, components, ISO parsing, and
> formatting; unified `TimeShift` exposes clock and calendar forms. Localized
> civil parsing is implemented; the named zone/locale/Period/TimeShift cases
> passed on all five native hosts in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962). Direct `Date`/`DateTime` `Period`
> operators remain open. The chapters below mark delivered and
> remaining operations individually.

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

Executable evidence: [native/interpreter temporal parity](../../../../crates/zirk-cli/tests/object_engine_parity.rs), [locale parsing and formatting tests](../../../../crates/zirk-runtime-core/src/locale.rs), and [zone/period/TimeShift tests](../../../../crates/zirk-runtime-core/src/zoned_temporal.rs).

---

**Previous:** [← Regex](../03-everyday-types/12-regex.md) · **Next:** [ Choosing a Temporal Type](01-choosing-a-temporal-type.md)
