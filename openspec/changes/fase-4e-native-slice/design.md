## Context

`fase-4e-unsafe-pointer-extern` (merged) built `Pointer<T>` with a conservative static escape rule (any occurrence as a return value, field-write source, or closure capture is rejected, regardless of whether the specific case is hypothetically safe) and its own design explicitly says this change should give `NativeSlice<T>` "the same treatment." The collector this compiler has (`fase-4e-colector-mark-sweep`) is non-moving, so nothing about pinning is load-bearing yet — a native view's safety story is entirely about *bounds* and *escape*, not about the referent moving underneath it.

## Goals / Non-Goals

**Goals:**
- `NativeSlice<T>`/`NativeSliceMut<T>` are constructed only through validated construction (`pointer.as_slice(length)`/`pointer.as_slice_mut(length)`) that checks nullability, alignment, extent, provenance where available, and mutation rights, returning `Result<..., NativeError>`.
- Indexing and iteration through an already-constructed view are bounds-checked and available in ordinary safe code — no `unsafe` needed to *use* a view, only to construct one.
- A view cannot escape past its validated owner's scope: cannot be returned, stored past the owner's lifetime, or captured by a closure — reusing `Pointer<T>`'s own conservative static rule.
- The view never owns or frees the storage it observes.

**Non-Goals:** (see proposal's "Explicitly out of scope" — pinning, volatile access through a view, unions, a linking manifest.)

## Decisions

### D1: `NativeSlice<T>`/`NativeSliceMut<T>` are dedicated `Base`/`IrType` variants, parallel to `Base::Pointer`/`Base::Weak` — compiler built-in, not routed through user-generic machinery

`Base::NativeSlice(u32)`/`Base::NativeSliceMut(u32)` (ids into an interning table, matching `Base::Pointer(u32)`'s actual shape — the precedent `fase-4e-weak`'s own task 1.1 confirmed by checking, not assuming, `Pointer`'s real representation). Element type `T` is restricted to the same ABI-safe subset `Pointer<T>` already accepts (`fase-4e-unsafe-pointer-extern`'s own D-series established this list: `Void`/`Boolean`/fixed-width `Int`/`UInt`/`Float32`/`Float64`/nested `Pointer<T>`) — a view over anything else has no well-defined native layout to bounds-check against.

### D2: Runtime representation is a `(pointer, length)` pair — no separate heap allocation, no header

A `NativeSlice<T>`/`NativeSliceMut<T>` value is a two-word stack/register value (base address + element count), not a collector-tracked allocation — it is not itself an "object" with identity; it is a bounded view over memory the program already owns (validated from an existing `Pointer<T>`). This matches how the existing `Pointer<T>` is represented (one word) and keeps the collector completely unaware of views, same as it's unaware of raw `Pointer<T>` today.

Alternative considered: give a view its own small heap allocation (like `Weak<T>`'s WeakCell) so it participates in the collector's own tracked-object bookkeeping. Rejected — a view has no identity to preserve (`is` on two views over the same range should compare structurally, not by allocation address) and adding an allocation per view construction is unnecessary indirection for a value that is conceptually just a checked pointer+length pair, exactly the "movable-with-handles... unnecessary indirection" reasoning ADR-003's closure already rejected for the collector's own object strategy.

### D3: Bounds validation at construction computes and stores nothing beyond what the two-word representation already holds; every index operation re-derives its check from `(pointer, length)` directly

`pointer.as_slice(length)` validates once at construction (non-null, `pointer`'s own alignment already established by `Pointer<T>`'s existing invariants, `length` is representable, and — where the underlying allocation's own size is knowable, i.e. it came from a `zirk_rt_alloc`'d buffer rather than an opaque foreign pointer — that `length` doesn't exceed it) and returns `Result<NativeSlice<T>, NativeError>`. Every subsequent `view[i]` checks `i < view.length` at the point of use — ordinary bounds checking, the same shape this compiler already emits for `Array<T>`/`List<T>` indexing, reused rather than inventing a second bounds-check code path.

Alternative considered: validate once and mark the view as trusted thereafter, skipping per-index checks. Rejected — directly contradicts `MEMORY_AND_UNSAFE_SEMANTICS.md`'s own explicit text: "Bounds checks remain active in safe operations over the resulting view."

### D4: Escape checking reuses `Pointer<T>`'s exact static rule and its existing checker pass, extended to the two new types

`fase-4e-unsafe-pointer-extern`'s escape check (any expression of type `Pointer<T>` rejected as a return value, field-write source, or closure-captured value, regardless of hypothetical safety) is generalized to match on "is this a dependent-reference type" (`Pointer<T>` OR `NativeSlice<T>` OR `NativeSliceMut<T>`) rather than duplicating the pass. New diagnostic reuses the existing pattern (parallel wording, not a new code family) — `E0444`-range (checker) continues from wherever `fase-4e-unsafe-pointer-extern` left its own pointer-escape codes, checked against the actual assigned codes at implementation time rather than assumed here.

## Risks / Trade-offs

- **[Risk] The conservative escape rule (D4) rejects some hypothetically-safe cases** (e.g., a view whose owner the caller already independently keeps alive) — same accepted trade-off `Pointer<T>` already made; this change does not attempt to be less conservative for views than for raw pointers, despite views being the "safer" surface. → Accepted: consistency between `Pointer<T>` and `NativeSlice<T>`'s escape rules is more valuable than marginally more permissive views, and a real borrow-tracking system is explicitly not being built in Zirk 1.x.
- **[Risk] D3's "size known only when the underlying allocation is `zirk_rt_alloc`'d" means a view over foreign/opaque native memory (e.g., a buffer returned by an `extern` call) can only validate what the caller-supplied `length` claims, not cross-check it against a real allocation size.** → Accepted and documented: this is inherent to unsafe interop with native code with no visibility into its own allocator; `MEMORY_AND_UNSAFE_SEMANTICS.md` frames construction as validating "provenance where available" — explicitly conditional, not an unconditional guarantee.
- **[Trade-off] No separate allocation (D2) means two views over the same underlying range are structurally equal/interchangeable by value, not identity-comparable in any meaningful sense** — considered acceptable since nothing in the spec gives views identity semantics; `Pointer<T>` already behaves the same way (value-comparable, not identity-tracked).
