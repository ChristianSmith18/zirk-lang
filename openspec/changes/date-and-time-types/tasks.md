# Tasks

## 1. Types and checker

- [ ] 1.1 Resolve `Date`, `Time`, `DateTime` in `Type::from_name` (new `Base` variants or runtime-backed value types) and remove them from `pending_type` Phase 7.
- [ ] 1.2 Register constructors `Date(y, m, d)`, `Time(h, m, s?, ns?)`, `DateTime(date, time)` and `Date + Time` composition in the checker, with the `InvalidDate`/`InvalidTime` controlled-error contract.
- [ ] 1.3 Register component properties (`year`, `month`, `day`, `hour`, `minute`, `second`, `nanosecond`), `to_string()`, comparisons, `Date ± Int` / `Date - Date` / `Time ± Duration`, and `Date.today()` / `Time.now_local()` in the member tables.
- [ ] 1.4 `zirk-sema` typing tests: valid construction/access/arithmetic; invalid `Date(2026, 2, 29)` literal-shape rejection path, `Time(25, 0)`, cross-type rejects.

## 2. Runtime

- [ ] 2.1 `zirk-runtime`: days-from-civil / civil-from-days algorithms (Hinnant), `zirk_rt_date_new` (validated), `zirk_rt_time_new`, `zirk_rt_datetime_new`, `zirk_rt_date_today`, `zirk_rt_time_now_local`.
- [ ] 2.2 ISO 8601 formatters `zirk_rt_date_to_string` / `time` / `datetime`; component accessors.
- [ ] 2.3 `InvalidDate` / `InvalidTime` native failure classes wired into the native-exception hierarchy (`checker.rs` `native_exceptions` + `lower.rs` `throw_native_failure`).

## 3. IR and codegen

- [ ] 3.1 `zirk-ir`: representation decision (packed `i64` for `Date`, `i64` ns-since-midnight for `Time`, pair/`struct` for `DateTime`); lower constructor calls, component projection, arithmetic and `to_string`.
- [ ] 3.2 `zirk-codegen-llvm`: emit/runtime-symbol wiring for the new entrypoints.
- [ ] 3.3 IR lowering tests mirroring the delta spec scenarios.

## 4. Corpus and example

- [ ] 4.1 Valid corpus `date_time_basics.zrk` (+ `.out`) covering construction, components, arithmetic, comparisons, `to_string`, `Date.today()`.
- [ ] 4.2 Invalid corpus: `Date(2026, 2, 29)` producing the controlled error output; `Time(25, 0)` likewise; `Instant.now()` still reports Phase 7.
- [ ] 4.3 Add a `temporal()` section to `hello.zrk`.

## 5. Status, docs, site

- [ ] 5.1 `ZIRK_FEATURE_STATUS.md`: move `Date`/`Time`/`DateTime` to shipped; keep `Instant`/`ZonedDateTime`/`TimeZone`/`Period` pending.
- [ ] 5.2 Handbook `03a-temporal` pages: document the civil subset.
- [ ] 5.3 `cargo test --workspace` green, then `./scripts/sync-website-content.sh --audit-date <today>` and a separate `zirk-lang-site` commit.
