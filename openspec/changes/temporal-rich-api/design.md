# temporal-rich-api — diseño

## Context

`date-and-time-types` (archived 2026-09-06) delivered the civil core:
`Date` = i64 days since epoch, `Time` = i64 ns since midnight,
`DateTime` = i128 epoch-ns civil; validated constructors throwing
`InvalidDateError`/`InvalidTimeError`; component properties via runtime
projection; `to_string()` ISO; `Date.today()`/`Time.now_*()`/
`DateTime.now_*()`; `Date ± Int`, `Date - Date` → `Int64`,
`Time ± Duration` → `Time` (wrap), `Time - Time` → `Duration`,
`Date + Time` → `DateTime`; total comparisons.

This change fills the rest of the civil surface the handbook documents
(`docs/handbook/02-handbook/03a-temporal/`), using day.js's
manipulate/query/display surface as the reference and keeping everything
zone-free.

## Goals / Non-Goals

**Goals:**

- Calendrical properties (`day_of_week`, `day_of_year`, `week_of_year`,
  `quarter`, `days_in_month`, `days_in_year`, `is_leap_year`,
  `millisecond`, `microsecond`).
- Query methods (`is_before`/`is_after`/`is_same`/`is_same_or_*`/
  `is_between`, `is_weekday`/`is_weekend`).
- `with_*` validated replacement and `start_of`/`end_of` boundaries.
- `Date.parse`/`Time.parse`/`DateTime.parse` → `Result<_, ParseError>`
  (canonical ISO forms only) and `format(pattern)` → `String` with a
  minimal token set.
- Duration interop: `DateTime ± Duration` → `DateTime`,
  `Date ± Duration` → `DateTime` (midnight promotion),
  `DateTime - DateTime` → `Duration`.

**Non-Goals:**

- `Period`, `Weekday`/`TemporalUnit` enum types, zone-aware types
  (`Instant`, `ZonedDateTime`, `TimeZone`) — Phase 7 as scheduled.
- Custom parse patterns (`parse(text, format)`), locales, relative-time
  display, `month_name`, `Date(text)` construction.
- `TimeShift` — the `Time ± Duration` wrap semantics shipped in
  `date-and-time-types` stays (decided with the user).

## Decisions

### D1 — `day_of_week` is `Int32` ISO 8601 (1 = Monday … 7 = Sunday)

The handbook's aspirational API names a `Weekday` enum. Registering a
native enum is real work for marginal gain today; `Int32` with the ISO
convention is unambiguous, comparable, and can be wrapped by a `Weekday`
enum later without breaking callers (the enum's `value` matches).
`is_weekday`/`is_weekend` are pure Boolean queries over it.

### D2 — `start_of`/`end_of` take a `String` unit, return `Result<T, ParseError>`

Alternative considered: a `TemporalUnit` enum. A string unit matches
day.js and needs no new enum machinery; an unknown unit is an
expected-failure `Result`, not an exception — the same channel
`Int32.parse` uses. Supported units: `"year"`, `"month"`, `"week"`
(ISO, Monday-start), `"day"`; `Time`/`DateTime` add `"hour"`, `"minute"`,
`"second"`. `start_of("week")` on Sunday goes back to the preceding
Monday (ISO, not locale-dependent).

### D3 — `Date ± Duration` promotes to `DateTime` (user decision)

The handbook marks `Date + Duration` invalid, arguing a timeline quantity
needs clock+zone. The user explicitly wants `Date(2026,9,6) + 5h` →
`DateTime 2026-09-06T05:00:00`: the date is read at local midnight, the
duration shifts it, and the wider `DateTime` type preserves everything —
no silent truncation, no ambiguity. `Date - Duration` symmetric.
This is a spec-level addition to `zirk-temporal-types`, not a handbook
contradiction — the handbook's rationale targets `Date + Duration` →
`Date` (which *would* lose information); promotion loses nothing.

### D4 — `DateTime ± Duration` is exact nanosecond arithmetic

`DateTime` is already epoch-ns `i128`; `Duration` is ns `i64`. `±` is a
widened `i128` add/sub — no runtime call needed, no wrap (a `DateTime`
has a date to carry into, unlike `Time`). `DateTime - DateTime` is the
i128 difference narrowed to `i64` ns `Duration` — overflow beyond ±292
years is a controlled `ArithmeticOverflowError`, consistent with the
existing `Duration` overflow story.

### D5 — `parse` reuses `build_result_from_flag`

`Int32.parse`/`Regex.parse` already lower `Result<T, ParseError>` through
`FunctionLowering::build_result_from_flag` + a runtime `(ok, value)`
probe. `Date.parse`/`Time.parse`/`DateTime.parse` follow the same shape:
`zirk_rt_date_parse_ok(ZirkStr)` + `zirk_rt_date_parse_value(ZirkStr)`
(strict ISO: `YYYY-MM-DD`; `HH:MM[:SS[.fraction]]`;
`YYYY-MM-DD[T ]HH:MM[:SS[.fraction]]`). The `E` class is the existing
`ParseError` — no new exception classes.

### D6 — `format(pattern)` is infallible, unknown text is literal

day.js passes unknown characters through; we do the same, so `format`
returns `String`, not `Result`. Minimal tokens: `YYYY`, `MM`, `DD`,
`HH`, `mm`, `ss`, `SSS` (ms). `[…]` escaping and locale names deferred.
Runtime: `zirk_rt_*_format(value, ZirkStr) -> ZirkStr` — one entrypoint
per type, pattern scanned once.

### D7 — New members follow the existing member dispatch

Checker: `member_type` gains arms alongside the `Date`/`Time`/`DateTime`
property arms added in `date-and-time-types`; `with_*`/`start_of`/
`end_of`/`is_*` are method calls on scalar receivers — the
`native-type-member-surface` scalar-method path already covers the
dispatch shape (`s.find("x")` precedent). `expr_types` records the result
type; lowering maps each member to a runtime entrypoint or pure IR:

| Member | Lowering |
|---|---|
| `day_of_week`, `day_of_year`, `week_of_year`, `quarter`, `days_in_month`, `days_in_year`, `is_leap_year` | `zirk_rt_date_*` calls on the i64 day count (DateTime delegates via its date half) |
| `millisecond`, `microsecond` | pure IR: `ns / 1e6 mod 1e3`, `ns / 1e3 mod 1e3` |
| `is_*` queries | pure IR comparisons on the operand pair |
| `with_*` | rebuild: project components → replace → `zirk_rt_*_is_valid` guard → `zirk_rt_*_days`/`nanos` (reuses the constructor path) |
| `start_of`/`end_of` | `zirk_rt_*_start_of`/`_end_of(value, unit_str)` returning `(ok, value)` → `Result` via `build_result_from_flag` |
| `parse` | `(ok, value)` probe → `build_result_from_flag` |
| `format` | `zirk_rt_*_format` → `ZirkStr` |
| `DateTime ± Duration` | i128 add/sub of ns |
| `Date ± Duration` → `DateTime` | widen days→ns i128, add, `zirk_rt_datetime_*` identity |

### D8 — `Weekday`, `Period`, locales stay pending

`pending_type` already gates `Period`/`Instant`/`ZonedDateTime`/
`TimeZone`; `Weekday` is *not* a known type name — it stays unknown
(`Type::from_name` untouched), documented as deferred in the handbook
README status note.

## Risks / Trade-offs

- **`Date + Duration` promotion surprises.** A user expecting `Date` gets
  `DateTime`. → Mitigation: the type is explicit in inference and the
  handbook gets a callout; `Date ± Int` remains the day-stepping path.
- **`format` token collisions** (e.g. a literal "mm" in prose). →
  Mitigation: day.js-compatible `[…]` escape deferred but the scanner is
  designed to accept it later without breaking valid patterns.
- **`start_of`/`end_of` as `Result`** is heavier than day.js. →
  Alternative (exception on unknown unit) rejected: an unknown unit is a
  programmer-correctable, expected failure — `Result` is the documented
  channel for that shape.
- **Runtime surface grows** (~15 new `zirk_rt_*` externs). → All are
  leaf pure functions over the existing representations; no new IR types.
