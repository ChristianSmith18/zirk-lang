## Context

`Map<K, V>` and `Set<T>` are already documented in `docs/handbook/02-handbook/12-collections/` and the `std.collections` reference. The compiler currently recognizes the names only through `pending_type` as Phase 7 pending work, and the `Base` enum has no variants for them. `Array<T>` and `List<T>` provide the model: built-in generic reference types with `Base` variants, interned type tables in the checker, dedicated `IrType` variants, IR instructions for allocation and mutation, and matching runtime headers.

## Goals / Non-Goals

**Goals:**

- Make `Map<K, V>` and `Set<T>` resolvable, typeable, and lowerable in the compiler.
- Implement the minimum viable API surface: construction, insertion, lookup/membership, and `length`/`is_empty`.
- Reuse the existing `Hash` + `Equal` contracts for key and element hashing.
- Keep the end-to-end test suite green throughout the work.

**Non-Goals:**

- Full iterator and set-algebra methods (`union`, `intersection`, etc.) are not required for the initial delivery; they can be added in a follow-up change.
- Lazy iterators, read-only views, and `inmut::strict` freezing rules are deferred to the `zirk-collections` follow-up that already covers `Iterable<T>`/`Iterator<T>`.
- The `Hash` contract derivation for records and enums is assumed to exist or to be stubbed; this change does not add derivation itself.

## Decisions

- **D1: Model `Map` and `Set` as built-in collection types, not library classes.**
  Like `Array` and `List`, they are `Base`/`IrType` variants with interned element tables. This keeps generic constraints explicit and avoids requiring a full class system for built-in collections.

- **D2: Hash table with insertion-ordered iteration.**
  The handbook promises insertion order and expected constant-time lookup. A single flat hash table with an array of entries and a probe or chaining strategy gives us both. The runtime uses a per-process defensive seed (already described in `std.collections`).

- **D3: Reuse `Hash` and `Equal` contracts for keys and set elements.**
  The `Hash` + `Equal` contract must be coherent: equal values hash equally. For now we support the primitive and string types that already have known hashing; user-defined types with derived `Hash` will follow later.

- **D4: One runtime module per collection (`map.rs` and `set.rs`) sharing a common entry/iterator header shape.**
  This mirrors `array.rs`/`list.rs` and keeps the runtime surface small and separately testable.

- **D5: No breaking changes to `Array`/`List`.**
  `Map`/`Set` are additive. Existing code paths for `Array`/`List` remain untouched except where they share `Base` match exhaustiveness.

## Risks / Trade-offs

- **[Risk] Adding `Base` variants touches many exhaustive `match` expressions.**
  → Mitigation: run `cargo check` after each file and add `Base::Map`/`Base::Set` arms immediately; use `todo!()` where behavior is not yet implemented so the compiler stays helpful.

- **[Risk] Hash table choice may affect GC layout assumptions.**
  → Mitigation: keep the object header GC-compatible by storing the entry buffer as a single managed allocation and never moving it in place; follow `array.rs`'s use of `zirk_alloc`/`zirk_realloc`.

- **[Risk] Method dispatch overlaps with existing `.get` and `.remove` on `List`.**
  → Mitigation: dispatch by receiver base (`Base::Map` or `Base::Set`) before looking at the method name, so `List` behavior is not accidentally changed.

## Open Questions

- Should `Map`/`Set` accept nullable key/element types? The spec disallows nullable or composite non-hashable keys; this will be enforced in the type checker.
- Should the first delivery include `Map<K,V>(entries)` and `Set(e0, e1, …)` variadic constructor syntax, or only `Map()` and `Set()`? This change targets empty construction; variadic forms are listed as follow-up tasks.
