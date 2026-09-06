# Tasks

## 1. Checker: static-member dispatch

- [x] 1.1 Register `LookupError` in `register_native_exception_hierarchy` and `NativeExceptions` (sibling of `ParseError`/`RegexError`).
- [x] 1.2 Dispatch `E.count` (property, `Int32`), `E.keys()` → `List<String>`, `E.values()` → `List<E>`, `E.from_name(name)` → `Result<E, LookupError>` on enum type paths (same dispatch point as `variant_accesses`).
- [x] 1.3 Dispatch `E.from_value(v)` → `Result<E, LookupError>` for traditional enums; reject it for algebraic enums.
- [x] 1.4 Dispatch `Enums.keys(E)` / `Enums.values(E)` / `Enums.count(E)` treating the argument as a type reference.
- [x] 1.5 Tests: accepted calls type-check, `from_value` on an algebraic enum is rejected, `from_name`/`from_value` wrong-arg diagnostics.

## 2. Lowering

- [x] 2.1 `count` lowers to `ConstInt`; `keys()`/`values()` lower to constant `List` construction in declaration order.
- [x] 2.2 `from_name`/`from_value` lower to a lookup producing `Ok`/`Err` (compile-time expansion or dedicated `InstKind`).
- [x] 2.3 `Enums.*` helpers lower identically once the type argument resolves.
- [x] 2.4 `synthesize_native_failure_bodies` covers `LookupError`; `NATIVE_FAILURE_CODES` in `lowering.rs` tests updated.
- [x] 2.5 `zirk-ir` lowering tests.

## 3. Verify / codegen / runtime

- [x] 3.1 Verify any new `InstKind` introduced; runtime helper only if expansion is not viable.
- [x] 3.2 `cargo check` green across the workspace.

## 4. End-to-end

- [x] 4.1 `crates/zirk-cli/tests/corpus/valid/enum_static_members.zrk` + `.out` exercising every member.
- [x] 4.2 Invalid corpus case for `from_value` on an algebraic enum.
- [x] 4.3 `cargo test -p zirk-cli --test end_to_end` green.

## 5. Docs and site

- [x] 5.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` and the enum handbook pages (mark members implemented).
- [ ] 5.2 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` and commit `../zirk-lang-site`.
- [ ] 5.3 Archive the change when the user confirms.
