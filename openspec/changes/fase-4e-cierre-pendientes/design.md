## Context

Phase 4e has already delivered the non-moving mark-sweep GC, `Weak<T>`, deep `clone`, `Pointer<T>`, `NativeSlice<T>`/`NativeSliceMut<T>`, `unsafe {}`/`commit {}` journals, and `expr[index]` over native slices. Three deliberate limitations remain in the unsafe/ffi/indexing surface and are small enough to close in a single follow-up change without reopening the larger memory-strategy questions.

## Goals / Non-Goals

**Goals:**

1. Make `return`, `break`, and `continue` inside `unsafe { ... }` correctly roll back the active journal before exiting the block, matching the behavior already implemented for exceptions.
2. Allow `Pointer.from(place)` where `place` is an lvalue field or slot of a `record` or `value class` whose underlying storage is addressable, with the same FFI-safety rules as `class` fields.
3. Add read-only `String[index]` indexing that returns a `Char` grapheme, using the existing grapheme runtime helpers.

**Non-Goals:**

- Implementing dependent references, automatic bounded native pinning, or `Pin<T>`. These require a dedicated design spike and are left to a future change.
- Adding write-assignment to `String[index]` (`s[i] = c`). Grapheme replacement changes string length and needs copy-on-write semantics that belong to the `String`/`Char` lifetime change.
- General `Array<T>`/`List<T>` indexing, which is Phase 7 work.

## Decisions

### 1. Unsafe journal cleanup for early exits

**Decision:** Extend `LoopTargets` with `unsafe_depth` and introduce a single `run_exit_cleanup` helper that interleaves `try` and `unsafe` frames by their lexical `pushed_at` order, calling `finally` or `JournalRollback` as appropriate.

**Rationale:** This reuses the exact interleaving pattern already proven by `lower_pending_exception_dispatch`, avoids duplicating the cleanup logic, and guarantees that `try { unsafe { ... } }` rolls back before `finally`, while `unsafe { try { ... } }` runs `finally` before rollback.

**Alternatives considered:**
- Adding a separate `run_unsafe_rollback_through` and calling it after `run_finally_through`. Rejected because it does not preserve lexical interleaving of mixed `try`/`unsafe` frames.
- Handling rollback in `end_unsafe_frame` for the normal-fallthrough path only. Rejected because it cannot run after the `Jump`/`Return` terminator has already been emitted.

### 2. `Pointer.from` over `record` and `value class` fields

**Decision:** Lower `Pointer.from` to a chain of *lvalue addresses* (`PointerFromSlot`, then one or more `PointerFromField`) rather than values. The `PointerFromField` instruction receives a pointer to the container (`Object` or `Pointer<Value>`) and returns a `Pointer<T>` to the selected field.

**Rationale:** Records and value classes are inline structs; their fields have no stable address unless the storage itself is addressed. Building a pointer chain keeps the semantics explicit and lets the existing GEP logic in `emit.rs` generalize to `ValueLayout`.

**Alternatives considered:**
- Auto-boxing the record before taking a pointer. Rejected because it would silently copy data and make the pointer point to a temporary.
- Allowing `Pointer.from` only on `value class` but not `record`. Rejected because both share the same inline storage model; the only difference is whether `unsafe` mutation is expected, and `Pointer.write` already requires `unsafe`.

### 3. `String[index]` read-only grapheme access

**Decision:** Add one new runtime helper `zirk_str_grapheme_offset` that walks the requested number of graphemes and returns the byte offset or `-1`. The IR instruction `StringGraphemeOffset` wraps this, and lowering reuses `GraphemeLenAt` + `GraphemeSlice` once the offset is known.

**Rationale:** This mirrors the `for ... in` over `String` implementation and reuses the existing Unicode grapheme helpers. Returning `Char` by copy is safe today because `Char` is owned, not GC-managed.

**Alternatives considered:**
- Exposing grapheme iteration directly in the IR. Rejected because it complicates the IR for a single read and does not improve performance over a dedicated runtime helper.

## Risks / Trade-offs

- `[Risk]` `run_exit_cleanup` may interact poorly with `finally` blocks that themselves contain `return`/`break`/`continue`. `Mitigation`: reuse the `std::mem::take` + restore pattern from `lower_pending_exception_dispatch` so re-entrant frames are not processed twice.
- `[Risk]` `Pointer.from` on value types can create a pointer that outlives the copied value if the root is a temporary. `Mitigation`: reject non-lvalue roots (`Foo().x`, `makePoint().x`) in the checker; `reject_pointer_escape` already prevents the resulting `Pointer<T>` from escaping the frame.
- `[Risk]` `StringGraphemeOffset` is `O(n)` over preceding graphemes. `Mitigation`: document it as a first cut and accept the cost; Phase 7 can introduce cached offset tables if needed.
- `[Risk]` Combining three unrelated changes in one OpenSpec change may create a large diff. `Mitigation`: keep each feature independent in the task list and commit separately; tests and fixtures are per feature.

## Open Questions

1. Should `Pointer.from` on a `record` field reject `Pointer.write` statically, or is `unsafe` permission enough to override record immutability? Answer needed before sema implementation.
2. Should `String[index]` return `Char?` on out-of-bounds, or throw `IndexOutOfBoundsError`? The existing `for ... in` and `NativeSlice` bounds pattern favors throwing; this change follows that convention.
