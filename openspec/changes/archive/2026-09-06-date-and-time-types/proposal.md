# Proposal: date-and-time-types

## Why

`Date`, `Time` and `DateTime` are registered type names that the checker
rejects with "arrives in Phase 7". The `zirk-temporal-types` spec already
defines their normative semantics in full; what is missing is the
implementation. This change delivers the civil (zone-free) subset of that
family — the part that needs no IANA database — so that everyday code can
construct, inspect, compare and format calendar values today, and so that
`Instant.now()`-style timing finally becomes expressible once the absolute
types land.

## What Changes

- Implement `Date`, `Time` and `DateTime` as immutable value types:
  - `Date(year, month, day)` with full calendar validation (leap years,
    days-per-month, proleptic Gregorian rules) — `Date(2026, 2, 29)` is a
    controlled `InvalidDate` error, never a silent normalization.
  - `Time(hour, minute, second?, nanosecond?)` with field-range validation —
    `Time(25, 0)` is a controlled `InvalidTime` error.
  - `DateTime(date, time)` composition and `Date + Time` combination.
  - Read-only component properties (`year`, `month`, `day`, `hour`,
    `minute`, `second`, `nanosecond`) and ISO-8601 `to_string()`.
  - Total ordering comparisons and equality.
  - `Date` arithmetic by calendar days (`Date + Int`, `Date - Date` →
    day count) and `Time` arithmetic by `Duration` modulo 24h.
  - `Date.today()` and `Time.now_local()` backed by the host clock
    through the runtime (zone-free: the host's civil time is taken as-is).
- Keep `Instant`, `ZonedDateTime`, `TimeZone` and `Period` pending: they
  need the IANA zone database and DST resolution, which is a separate
  change.
- Update the "Out-of-scope temporal types" requirement: `Date`, `Time` and
  `DateTime` move from "not yet available" to shipped; the zone-aware types
  stay pending.
- Extend `hello.zrk` with a temporal section and update the feature-status
  table (Phase 7 row for the civil types).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-temporal-types`: "Out-of-scope temporal types" is split — the
  civil types `Date`, `Time` and `DateTime` are implemented per the
  existing normative requirements (distinct types, calendar/clock
  distinction, parsing/formatting, controlled errors), while
  `Instant`, `ZonedDateTime`, `TimeZone` and `Period` remain gated to a
  later phase. New requirement covering the implemented civil surface.
- `zirk-ir-lowering`: new requirement for lowering the civil temporal
  types (value representation, constructor validation calls, component
  access, arithmetic, `to_string`).

## Impact

- `crates/zirk-sema/src/types.rs`, `checker.rs`: `Date`/`Time`/`DateTime`
  resolve in `Type::from_name` and leave `pending_type`; constructors,
  members and operators gain checker entries.
- `crates/zirk-ir/src/lower.rs` + `zirk-ir` ops: lowering for the new
  value types (likely `i64`/`struct` representations or runtime calls).
- `crates/zirk-runtime/src/`: calendar algorithms (days-from-civil,
  civil-from-days), validation, ISO formatting/parsing, host-clock reads.
- `crates/zirk-codegen-llvm`: new runtime symbols.
- Tests: `zirk-sema` typing tests, IR lowering tests, CLI valid/invalid
  corpus fixtures (`date_time_basics`, `invalid_date_literal`, ...).
- Docs: handbook `03a-temporal` pages and `ZIRK_FEATURE_STATUS.md` Phase 7
  row move from pending to shipped for the civil subset.
- Website: `../zirk-lang-site` must be re-synced — this changes the public
  feature-status surface.
