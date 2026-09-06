## Why

Zirk already specifies `Map<K, V>` and `Set<T>` as part of the standard collection family in the handbook and the `zirk-collections` spec, but the compiler still rejects them as Phase 7 pending types. This forces example code and early adopter tests to stay unwritable even though the language design is complete. Implementing the core type system, IR, runtime, and a focused API surface for maps and sets unblocks the documented collection family.

## What Changes

- Introduce `Map<K, V>` and `Set<T>` as first-class type-system base variants, with interned `<K, V>` and `<T>` tables.
- Resolve the type references `Map<K, V>` and `Set<T>` in the semantic checker and remove `Map`/`Set` from the `pending_type` list.
- Add `Map`/`Set` `IrType` variants and the minimum IR instructions needed for construction and member access (`MapNew`, `SetNew`, `MapSet`, `MapGet`, `MapContainsKey`, `SetAdd`, `SetContains`).
- Implement GC-managed `Map` and `Set` runtime objects with insertion-ordered, hash-backed storage; require coherent `Hash` + `Equal` for keys and set elements.
- Add type-checker dispatch for `Map.set`, `Map.get`, `Map.get_or_null`, `Map.contains_key`, `Set.add`, `Set.remove`, `Set.contains`, plus shared `length` and `is_empty` properties.
- Add `zirk-sema`/`zirk-ir`/`zirk-codegen-llvm` tests and `zirk-cli` end-to-end fixtures for map and set construction and basic usage.
- Update `docs/init/ZIRK_FEATURE_STATUS.md`, the handbook collection status, and `../zirk-lang-site` after the implementation is committed.

No existing valid program becomes invalid; `Map` and `Set` were not previously resolvable.

## Capabilities

### New Capabilities

- `map-collections`: `Map<K, V>` type resolution, construction, insertion, lookup, membership, and basic property access.
- `set-collections`: `Set<T>` type resolution, construction, add/remove, membership, and set relation queries.

### Modified Capabilities

*None. The impacted existing areas (`zirk-collections`, `zirk-type-system`, `zirk-ir-lowering`, `zirk-runtime`) are driven by the implementation tasks below; no existing normative requirement changes.

## Impact

- `crates/zirk-sema/src/types.rs`: new `Base` variants and interning tables.
- `crates/zirk-sema/src/checker.rs`: type reference resolution, method dispatch, `pending_type` update.
- `crates/zirk-ir/src/ir.rs`: `IrType::Map`/`IrType::Set`, new instructions.
- `crates/zirk-ir/src/lower.rs`: lowering for constructors and member calls.
- `crates/zirk-codegen-llvm/src/runtime.rs` and `emit.rs`: runtime symbol declarations and instruction emission.
- `crates/zirk-runtime/src/map.rs` and `set.rs`: new runtime modules (or equivalent additions to a shared collections module).
- `crates/zirk-cli/tests/` and `crates/zirk-cli/tests/corpus/`: new end-to-end fixtures.
- `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/handbook/02-handbook/12-collections/README.md`, and `../zirk-lang-site` after commit.
