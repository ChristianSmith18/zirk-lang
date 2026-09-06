## 1. Type system: add `Map` and `Set` base variants

- [ ] 1.1 Add `Base::Map(u32)` and `Base::Set(u32)` variants to `crates/zirk-sema/src/types.rs`.
- [ ] 1.2 Add `map_types` and `set_types` interning tables to `Checker`/`CheckedProgram`/`TypeNames`.
- [ ] 1.3 Update `Type::from_name` to not resolve `Map`/`Set` without arguments, and remove `Map`/`Set` from `pending_type`.
- [ ] 1.4 Update `describe` and the `TypeNames` trait to format `Map<K, V>` and `Set<T>`.
- [ ] 1.5 Update `Base` sorting in `sort_bases` and any other exhaustive `match` on `Base` to include the new variants.
- [ ] 1.6 Run `cargo check -p zirk-sema` and fix compile errors; keep tests passing.

## 2. Type system: resolve `Map<K, V>` and `Set<T>` references

- [ ] 2.1 Add `resolve_map_type_ref` to `crates/zirk-sema/src/checker.rs` accepting exactly two type arguments and interning `(K, V)`.
- [ ] 2.2 Add `resolve_set_type_ref` accepting exactly one type argument and interning `T`.
- [ ] 2.3 Wire `Map` and `Set` into `resolve_type_atom`.
- [ ] 2.4 Add a temporary key-hashability check that accepts primitives and `String`, rejects `List`/`Map`/`Set`/`Array`.
- [ ] 2.5 Add `zirk-sema` tests that `Map<String, Int32>` and `Set<String>` resolve and `Set<List<Int32>>` is rejected.

## 3. IR: add `Map` and `Set` types and construction instructions

- [ ] 3.1 Add `IrType::Map(u32)` and `IrType::Set(u32)` to `crates/zirk-ir/src/ir.rs` with matching `Nullable` and string conversions.
- [ ] 3.2 Add `map_types` and `set_types` tables to `IrModule`.
- [ ] 3.3 Add `MapNew { key_id, value_id }` and `SetNew { element_id }` instructions.
- [ ] 3.4 Update `is_managed_reference` and any other exhaustive `match` on `IrType`.
- [ ] 3.5 Add `zirk-ir` verification tests that `MapNew`/`SetNew` produce the expected `IrType`.

## 4. IR lowering: lower `Map()` and `Set()` constructors

- [ ] 4.1 Add `lower_map_new` and `lower_set_new` helpers in `crates/zirk-ir/src/lower.rs`.
- [ ] 4.2 Extend the constructor-call dispatch in `lower_expr` to recognize `Map()` and `Set()`.
- [ ] 4.3 Add `zirk-ir` lowering tests for empty `Map<String, Int32>` and `Set<String>` construction.

## 5. Runtime: add `Map` and `Set` native objects

- [ ] 5.1 Create `crates/zirk-runtime/src/map.rs` and `crates/zirk-runtime/src/set.rs` (or a single `crates/zirk-runtime/src/collections.rs`) with empty allocation functions.
- [ ] 5.2 Register the new runtime symbols in `crates/zirk-codegen-llvm/src/runtime.rs`.
- [ ] 5.3 Link the runtime functions in `crates/zirk-codegen-llvm/src/emit.rs` for `MapNew` and `SetNew`.
- [ ] 5.4 Run `cargo test -p zirk-cli --test end_to_end` to confirm no regressions.

## 6. Type checker: dispatch `Map` and `Set` methods

- [ ] 6.1 Add `Map` and `Set` method dispatch in `crates/zirk-sema/src/checker.rs` for `.length`, `.is_empty`, `.set`, `.get_or_null`, `.contains_key`, `.add`, `.remove`, and `.contains`.
- [ ] 6.2 Check method arguments against the interned `K`/`V`/`T` types.
- [ ] 6.3 Add `zirk-sema` tests for accepted and rejected map/set method calls.

## 7. IR lowering and codegen: lower map/set method calls

- [ ] 7.1 Add IR instructions `MapSet`, `MapGet`, `MapContainsKey`, `SetAdd`, `SetRemove`, and `SetContains`.
- [ ] 7.2 Add lowerer helpers and runtime implementations for the new instructions.
- [ ] 7.3 Emit LLVM calls in `crates/zirk-codegen-llvm/src/emit.rs`.
- [ ] 7.4 Add `zirk-ir`/`zirk-codegen-llvm` tests for the method calls.

## 8. End-to-end tests and fixtures

- [ ] 8.1 Add `map_basic.zrk` and `set_basic.zrk` fixtures to `crates/zirk-cli/tests/corpus/`.
- [ ] 8.2 Add `zirk-cli` end-to-end assertions for empty construction, set/add, and membership.
- [ ] 8.3 Run `cargo test -p zirk-cli --test end_to_end` and fix failures.

## 9. Documentation and status updates

- [ ] 9.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` to mark `Map` and `Set` as partially delivered.
- [ ] 9.2 Update `docs/handbook/02-handbook/12-collections/README.md` to reflect delivered `Map` and `Set`.
- [ ] 9.3 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`, review `../zirk-lang-site` diff, and commit the website changes.
  - Left pending; requires `zirk-lang-site` and explicit audit date.
