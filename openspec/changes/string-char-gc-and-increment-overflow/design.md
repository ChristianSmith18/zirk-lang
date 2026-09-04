# Design: String/Char GC and increment overflow

## Context

`docs/decisions/ADR-003-memoria.md` closed on a non-moving mark-sweep collector reachable through `zirk_rt_alloc`. `docs/decisions/ADR-005-representacion-string.md` made `String` and `Char` opaque handles whose layout belongs to `zirk-runtime`. Those two decisions meet in this change: the collector must reclaim the opaque handles, but the IR and codegen must still not know the handle layout.

Today the runtime string module deliberately leaks every `String`/`Char` via `Box::into_raw` because `zirk_rt_alloc` was not yet a collector. That leak is no longer acceptable now that the collector exists. In parallel, `++`/`--` in expression position are lowered as raw `Binary` operations, so `i++` on `Int32` at `2147483647` silently wraps instead of throwing `ArithmeticOverflowError` like `i + 1` does.

## Goals / Non-Goals

**Goals:**
- Every `String` and `Char` allocated by `zirk-runtime` is reclaimed by the existing mark-sweep collector when unreachable.
- The compiler treats `String`/`Char` as managed references for GC root enumeration and synthetic-slot spilling.
- `++` and `--` in prefix and postfix position overflow-check on every integer width and signedness, consistent with `+`/`-`/`*`.
- ADR-005 is preserved: the IR still sees `String`/`Char` only as opaque pointers.

**Non-Goals:**
- Moving or compacting the heap.
- Small-string optimization, string interning, or cross-ABI string lifetime (e.g. strings passed to `extern "C"` functions).
- Changing the language semantics of `String` equality, identity, or grapheme indexing.
- Introducing reference counting as a public or hidden lifetime model.

## Decisions

### 1. String/Char values become ordinary GC objects

`zirk_rt_alloc` will be the single allocation path for all runtime strings. A string object has the same three-word header as any GC-managed value, followed by a `ZirkString` payload, followed by the UTF-8 bytes when the runtime owns them.

```
[ descriptor | next | size | bytes | len | is_ascii | bytes... ]
  ^ header (3 words)   ^ payload              ^ inline data
```

- **Why:** It reuses the existing non-moving mark-sweep path, keeps `String` opaque, and avoids inventing a second allocator.
- **Literal strings** (`zirk_str_from_utf8`) point `bytes` at the global constant and allocate only header + payload. The collector frees the object; the constant is never freed.
- **Runtime-built strings** (`to_string`, `concat`, `repeat`, `slice`) allocate header + payload + `len` bytes, set `bytes` to the inline region, and copy the result there.
- `ZirkString` keeps its `#[repr(C)]` shape; `borrow()` is updated to skip the GC header.

### 2. A dedicated string descriptor with no traced fields

`collector.rs` exposes a static `zirk_rt_string_descriptor` whose `gc_field_count` is zero, so `mark_object` does not try to follow `ZirkString.bytes` as a reference.

- **Why:** The `bytes` pointer is owned data, not a GC reference, and the collector must not interpret it as one.
- The descriptor layout matches existing class descriptors (method table, ancestor count, contract count, GC field count) so `gc_field_offsets()` works unchanged.

### 3. `IrType::String` and `IrType::Char` are managed references

```rust
match self {
    IrType::Object(_) | IrType::Contract(_) | IrType::Weak(_) |
    IrType::Dependent(_) | IrType::Pin(_) |
    IrType::String | IrType::Char => true,
    ...
}
```

`zirk-codegen-llvm/src/emit.rs` `gc_reference_paths()` also treats `String`/`Char` as leaf reference types.

- **Why:** This is the only compiler-level change needed. Root enumeration and synthetic-slot spilling (`fase-4e-colector-mark-sweep` design D4) then automatically cover every string/char temporary, local, field, and return value.
- This does not leak layout: the IR still carries only an opaque `ptr`.

### 4. `clone.rs` shares immutable string handles

When `clone_recursive()` sees the string descriptor, it returns the source handle unchanged instead of allocating a copy.

- **Why:** `String`/`Char` are immutable; sharing the handle is sound and avoids copying the `bytes` pointer incorrectly. A deep copy of the whole object would also copy the absolute `bytes` address, leaving the clone pointing at the source's inline bytes.
- `is` still compares handles, and `==` still compares content; sharing is observable only through `is`, which is consistent with immutable values that may be interned/shared by the runtime.

### 5. `lower_increment` uses the same checked arithmetic as `+`/`-`

For an `Increment` expression:

1. `check_increment()` in `zirk-sema` accepts any `Base::Int(_)` width, not only `Int32`.
2. `lower_increment()` emits `ConstInt(1)` at the variable's own width using `const_int_at(1, ty, span)`.
3. It then calls `emit_checked_binary()` with `BinaryOp::Add` for `++` and `BinaryOp::Sub` for `--`, which emits `CheckedArithmetic` and throws `ArithmeticOverflowError` on overflow.

- **Why:** This reuses the existing `fase-4b` overflow guard for `+`/`-`/`*` and makes increment/decrement consistent with the language's checked-arithmetic promise.
- Statement-position `i++;` is already desugared to `i = i + 1` and goes through `lower_binary`; this change only repairs expression-position `i++`/`++i`/`i--`/`--i`.

### 6. GC trigger threshold stays, but becomes meaningful for strings

The default `ZIRK_GC_THRESHOLD` (1 MiB) remains. Because string allocations now call `zirk_rt_alloc`, `maybe_collect()` counts them and collections fire on string-heavy programs.

- **Why:** The threshold is a tuning knob (`ZIRK_GC_THRESHOLD`); the immediate fix is making strings visible to the collector at all. Corpus tests will measure peak RSS and can drive a later threshold change if needed.

## Risks / Trade-offs

- **[Risk]** Synthetic-slot spilling of every `String`/`Char` result may increase stack pressure and slow very tight string loops.
  - **Mitigation:** This is the same cost already paid for object references. A future optimization can keep transient string results in registers when no allocation call intervenes, but correctness requires spilling first.
- **[Risk]** `zirk_str_from_utf8` allocates a fresh GC object for every literal occurrence in a hot loop.
  - **Mitigation:** Correctness first. Literal interning or module-level string constants are a follow-up optimization and do not change the ABI.
- **[Risk]** `clone.rs` sharing string handles changes the observable `is` result for cloned objects' string fields.
  - **Mitigation:** Document that the runtime may share immutable string handles; `is` tests handle identity, `==` tests content, and mutation of strings is impossible by language semantics.
- **[Risk]** Changing `is_managed_reference` for `String`/`Char` could affect `Nullable<String?`/`Char?` root paths.
  - **Mitigation:** `gc_reference_paths` already handles `Nullable` by recursing into its inner type; adding `String`/`Char` as leaf references gives the correct `{present, payload}` path.
- **[Risk]** String objects have no finalizer, so no resource side effects run on collection.
  - **Mitigation:** This matches ADR-003's "no general-purpose finalizers" rule. `String` owns no external resources.

## Migration Plan

No source migration is required. The change is purely runtime and compiler-internal.

Deployment steps:
1. Implement runtime string allocation through `zirk_rt_alloc`.
2. Update `ir.rs`, `lower.rs`/`lower_increment`, and `emit.rs` `gc_reference_paths`.
3. Update `clone.rs` to share string handles.
4. Add CLI corpus and unit tests for string-collection RSS and increment overflow.
5. Run `cargo test --workspace`, `cargo clippy --workspace`, and `openspec validate --all --strict`.
6. Update `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/handbook/13-appendices/07-current-limitations.md`, and `docs/decisions/proximos-pasos-fase-4.md` section 6.1.
7. Run `./scripts/sync-website-content.sh` and review `../zirk-lang-site`.

Rollback is a single revert: the runtime string functions can temporarily be restored to `Box` allocation, but that reintroduces the leak.

## Open Questions

- Should `++`/`--` be allowed on `Float`? The current checker rejects non-`Int32`. This design generalizes only to integers because `zirk-scalars` "Checked integer arithmetic" does not cover floats. If floats are desired, a separate change is needed.
- Should `String`/`Char` handles be pinned when passed to `extern "C"` functions? The current `Pin<T>` inference only looks at `Pointer.from` and object field/method use. Strings are opaque pointers; an `extern "C"` signature that takes `*const c_void` and keeps it beyond the call is already unsafe and out of scope for this change.
