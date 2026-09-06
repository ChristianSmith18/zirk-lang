# temporal-rich-api

## Why

`Date`/`Time`/`DateTime` (`date-and-time-types`) only ship construction,
raw components, comparisons and a minimal operator set. Real programs need
the rest of the surface the handbook already documents — calendrical
properties, `is_*`/`is_between` queries, `with_*`/`start_of`/`end_of`
manipulation, ISO parsing and presentation formatting — and `Duration`
compatibility beyond `Time ± Duration` (day.js's manipulate surface is the
reference).

## What Changes

- **Calendrical properties** on `Date`/`DateTime`: `day_of_week` (ISO
  1–7 `Int32`), `day_of_year`, `week_of_year` (ISO), `quarter`,
  `days_in_month`, `days_in_year`, `is_leap_year`.
- **Sub-second properties** on `Time`/`DateTime`: `millisecond`,
  `microsecond` (derived from the nanosecond field).
- **Queries** on all three civil types: `is_before`, `is_after`,
  `is_same`, `is_same_or_before`, `is_same_or_after`, `is_between(a, b)`;
  plus `is_weekday`/`is_weekend` on `Date`/`DateTime`.
- **Manipulation** (validated, immutable):
  - `with_year`/`with_month`/`with_day` on `Date`;
    `with_hour`/`with_minute`/`with_second`/`with_nanosecond` on `Time`;
    `with_date`/`with_time` plus all of the above on `DateTime`.
    An invalid result throws the same `InvalidDateError`/`InvalidTimeError`
    contract as the constructors.
  - `start_of(unit)`/`end_of(unit)` with string units
    (`"year"`, `"month"`, `"week"`, `"day"`; `DateTime`/`Time` also accept
    `"hour"`, `"minute"`, `"second"`). Unknown units return
    `Result<_, ParseError>`.
- **Duration compatibility** — **BREAKING-spec additions** (no existing
  behavior changes):
  - `DateTime ± Duration` → `DateTime` (exact nanosecond shift of the
    local components).
  - `Date ± Duration` → `DateTime` (the date read at midnight, promoted:
    `Date(2026,9,6) + 5h` → `DateTime 2026-09-06T05:00:00`).
  - `DateTime - DateTime` → `Duration` (exact epoch-nanosecond difference).
  - `Duration + Date`/`Duration + DateTime` symmetric.
  - Existing rules are unchanged: `Date ± Int` → `Date`,
    `Date - Date` → `Int64`, `Time ± Duration` → `Time` (wrap),
    `Time - Time` → `Duration`, `Date + Time` → `DateTime`.
- **Parsing**: `Date.parse(text)`, `Time.parse(text)`,
  `DateTime.parse(text)` → `Result<_, ParseError>` over the canonical ISO
  8601 forms.
- **Formatting**: `format(pattern)` → `String` on all three types with a
  minimal token set (`YYYY`, `MM`, `DD`, `HH`, `mm`, `ss`, `SSS`);
  unknown text passes through literally (day.js convention). No locale
  support in this change.
- **Out of scope**: `Period`, `Weekday`/`TemporalUnit` enums, zone-aware
  types, `format()` locale arguments, relative-time display.

## Capabilities

### New Capabilities

- `zirk-temporal-members`: calendrical properties, `is_*`/`is_between`
  queries, `with_*` replacement, `start_of`/`end_of` boundaries, ISO
  `parse` and minimal `format` for the civil temporal types.

### Modified Capabilities

- `zirk-temporal-types`: extends *Civil temporal arithmetic* with
  `DateTime ± Duration`, `Date ± Duration` → `DateTime` promotion, and
  `DateTime - DateTime` → `Duration`.
- `zirk-ir-lowering`: the civil-temporal requirement gains member-call
  and Result-returning `parse` lowering scenarios.

## Impact

- `crates/zirk-sema`: member tables, static `parse` calls, new
  arithmetic combinations, `Result` return types.
- `crates/zirk-ir`: lowering for the new members, `format`/`parse`
  runtime calls, Duration-mixing binary rules.
- `crates/zirk-runtime`: civil helpers (ISO week, day-of-year, start/end
  of unit, component replacement, ISO parsers, pattern formatter).
- `crates/zirk-cli` corpus, `crates/zirk-sema`/zirk-ir tests, handbook
  README status note, `ZIRK_FEATURE_STATUS.md`, website sync.
