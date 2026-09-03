## Context

Phase 4e is the last remaining piece of the single-threaded foundation before concurrency. The pieces already delivered are `Result<T,E>` (4a), explicit and typed exceptions (4b), single-resource `match with` (4c), escaping single-capturing closures (4d), and the core unsafe/memory toolkit (4e partial). What remains in 4e is the unsafe/memory closeout that lets the compiler support Phase 5's `Task`, `Channel`, `Mutex`, and `Atomic`:

- `Dependent<T>` references that keep their base object alive through the GC;
- automatic bounded native pinning (`Pin<T>`);
- completed `inmut::strict` reachable-alias analysis;
- native view provenance for bounded slice construction.

This change treats those items as one coordinated closeout. Where possible it reuses the existing machinery: the mark-sweep runtime, the `unsafe` journal, the object header, the shadow stack, and the existing alias-analysis pass. New work is added only where those surfaces are genuinely incomplete.

## Goals / Non-Goals

**Goals:**

1. Implement `Dependent<T>` references as a two-word `{object pointer, base pointer}` value with GC reachability through the base.
2. Implement automatic bounded native pinning (`Pin<T>`) that is inferred when an interior address is exposed and released on every `unsafe`/`commit` exit.
3. Complete `inmut::strict` reachable-alias analysis for field declarations and mutating method calls.
4. Track native view provenance for `Pointer.from(place).as_slice(n)` and reject overlong slices.
5. Update `ZIRK_ROADMAP.md`, `ZIRK_FEATURE_STATUS.md`, and `docs/handbook/13-appendices/07-current-limitations.md` to mark Phase 4e complete.

**Non-Goals:**

- Phase 5 concurrency primitives (`Task`, `Channel`, `Atomic`, `Mutex`, etc.).
- `Array<T>`/`List<T>`/`Map`/`Set` collections (Phase 7).
- Generators, pipe, and functional combinators (Phase 7b).
- Self-hosting or package management (Phases 8–12).
- New syntax unrelated to the listed features; existing keywords already cover `Dependent`, `Pin`, `Pointer`, `inmut::strict`, etc.

## Decisions

### 1. `Dependent<T>` references

**Decision:**
- A `Dependent<T>` is a two-word reference `{object pointer, base pointer}` where the base pointer is the GC-managed object the dependent value lives inside.
- The GC treats `Dependent<T>` as a strong edge to the base object, and the base object keeps the dependent object alive.
- A dependent reference is invalidated when the base object is mutated in a way that could move or deallocate the dependent storage.

**Rationale:** This matches the spec's intent without requiring a full region or lifetime system; it reuses the existing GC and object-layout machinery.

**Alternatives considered:**
- Pure compile-time lifetime tracking. Rejected because it requires a borrow checker the project has not adopted.

### 2. Automatic bounded native pinning (`Pin<T>`)

**Decision:**
- `Pin<T>` wraps a `T` and, for the duration of the pin, prevents the GC from moving the underlying object and prevents any operation that could invalidate interior pointers.
- For native slices and `Pointer<T>`, `Pin<T>` is created automatically by the checker when an interior address is exposed, and is bounded by the lifetime of the enclosing `unsafe`/`commit` block.
- The runtime records pinned objects in a per-thread list and removes them at the corresponding unwind/commit/rollback point.

**Rationale:** Pinning is required for safe native interop; integrating it with `unsafe` journals and `commit` gives us bounded, deterministic unpinning for free.

**Alternatives considered:**
- Manual `pin()`/`unpin()` only. Rejected because the language's safety goal is automatic bounded pinning.

### 3. `inmut::strict` completion

**Decision:**
- Extend the existing strict-alias analysis from local/parameter rebindings and field projections to field declarations themselves and to mutating method calls reached through a strict reference.
- A `mut` method call on an `inmut::strict` receiver is rejected; an `inmut::strict` field declaration makes the field unwritable through any projection.

**Rationale:** These are the two remaining gaps noted in `ZIRK_ROADMAP.md` for the `inmut::strict` feature.

### 4. Native view provenance

**Decision:**
- Track the statically known extent of the source `place` when a `NativeSlice<T>` or `NativeSliceMut<T>` is constructed with `Pointer.from(place).as_slice(n)`.
- Reject the construction with an extent diagnostic when the compiler can prove `n` exceeds the available elements.

**Rationale:** Provenance checking prevents out-of-bounds native views before lowering; it reuses the existing constant-folding and array-length facts in the semantic checker.

## Risks / Trade-offs

- `[Risk]` `Dependent<T>` and `Pin<T>` interact with the GC shadow stack and `unsafe` journals in subtle ways. `Mitigation`: add dedicated runtime tests for "dependent base collected during pin", "pin released on journal rollback", and "dependent invalidated on base mutation".
- `[Risk]` `Pin<T>` and `Dependent<T>` introduce new two-word runtime values that must be handled by the GC and every backend. `Mitigation`: enumerate them explicitly in the shadow-stack and root categories, then run the full runtime test suite.
- `[Risk]` `inmut::strict` completion may reject programs that previously compiled when the compiler missed strict violations. `Mitigation`: document the tightened checks in the change and handbook; fix any tests that rely on the bug.
- `[Risk]` Automatic pinning may be inserted more eagerly than users expect, pinning longer-lived objects. `Mitigation`: keep the pin bounded to the smallest `unsafe`/`commit` block and emit `UnpinObject` on all exits.
