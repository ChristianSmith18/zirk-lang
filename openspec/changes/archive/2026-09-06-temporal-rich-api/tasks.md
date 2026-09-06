# Tasks

## 1. Runtime (`zirk-runtime/src/temporal.rs`)

- [x] 1.1 Calendrical helpers: `zirk_rt_date_day_of_week` (ISO 1–7), `zirk_rt_date_day_of_year`, `zirk_rt_date_week_of_year` (ISO), `zirk_rt_date_quarter` (or IR-derivable), `zirk_rt_date_days_in_month`, `zirk_rt_date_days_in_year`, `zirk_rt_date_is_leap`; DateTime delegates via its date half.
- [x] 1.2 `zirk_rt_date_start_of` / `zirk_rt_date_end_of` / `zirk_rt_time_start_of` / `zirk_rt_time_end_of` / `zirk_rt_datetime_start_of` / `zirk_rt_datetime_end_of` taking `(value, unit: ZirkStr)` and returning a pair `(ok: bool, value)`; units: `year`/`month`/`week` (ISO Monday-start)/`day`, plus `hour`/`minute`/`second` for Time/DateTime.
- [x] 1.3 ISO parsers: `zirk_rt_date_parse_ok/_value`, `zirk_rt_time_parse_ok/_value`, `zirk_rt_datetime_parse_ok/_value` over `ZirkStr` (strict `YYYY-MM-DD`, `HH:MM[:SS[.frac]]`, `YYYY-MM-DD[T ]HH:MM[:SS[.frac]]`).
- [x] 1.4 Pattern formatter: `zirk_rt_date_format`/`zirk_rt_time_format`/`zirk_rt_datetime_format` over `(value, pattern: ZirkStr) -> ZirkStr`; tokens `YYYY MM DD HH mm ss SSS`, unknown text literal.

## 2. Semantic checker (`zirk-sema`)

- [x] 2.1 `member_type` arms: calendrical properties on Date/DateTime, `millisecond`/`microsecond` on Time/DateTime, `is_*`/`is_between` queries, `with_*` replacements, `start_of`/`end_of` → `Result<T, ParseError>`, `format` → `String`.
- [x] 2.2 `check_call`: register `Date.parse`/`Time.parse`/`DateTime.parse` static calls → `Result<_, ParseError>` (reuse the `scalar_static_accesses`/`native_static` path).
- [x] 2.3 `native_arithmetic` + `expect_comparable`: `DateTime ± Duration` → `DATETIME`, `Date ± Duration` → `DATETIME`, `Duration + Date/DateTime` symmetric, `DateTime - DateTime` → `DURATION`.
- [x] 2.4 Typing tests: valid + invalid per spec scenario (unknown unit Result, rejected `with_*`, parse error, wrong-member rejects).

## 3. IR lowering (`zirk-ir`)

- [x] 3.1 Extern declarations for all new `zirk_rt_*` entrypoints.
- [x] 3.2 `lower_temporal_property`: new property arms (DateTime delegates to its halves); `millisecond`/`microsecond` as pure IR arithmetic.
- [x] 3.3 Temporal method lowering: `is_*`/`is_between` → pure comparisons; `with_*` → project/replace/revalidate via the constructor path; `start_of`/`end_of`/`parse` → `build_result_from_flag`; `format` → `ZirkStr` call.
- [x] 3.4 `lower_temporal_binary`: `DateTime ± Duration` i128 shift, `Date ± Duration` promotion (days→ns widen + add), `DateTime - DateTime` → `Duration`.
- [x] 3.5 Lowering tests mirroring the delta spec scenarios (pure-IR query, parse Result shape, format call).

## 4. Corpus and example

- [x] 4.1 Valid corpus `temporal_members.zrk` (+ `.out`) covering properties, queries, `with_*`, `start_of`/`end_of`, `parse` Ok/Error, `format`, and the Duration interop.
- [x] 4.2 Invalid corpus entries: `d.with_month("x")` type mismatch, `Date.parse` on non-String, `Date + Duration` result-type misuse if relevant.
- [x] 4.3 Extend `hello.zrk`'s `temporal()` section with the new members.

## 5. Status, docs, site

- [x] 5.1 `ZIRK_FEATURE_STATUS.md`: extend the `Date`/`Time`/`DateTime` row with the new members.
- [x] 5.2 Handbook `03a-temporal/README.md` status note + the `02-date`/`03-time`/`04-date-time` "Status" markers for shipped members; callout for `Date + Duration` → `DateTime`.
- [x] 5.3 `cargo test --workspace` green, then `sync-website-content.sh --audit-date <hoy>` + commit en `zirk-lang-site`.
