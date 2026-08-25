## Why

`docs/decisions/ADR-003-memoria.md` closed on 24 de agosto de 2026 (non-moving mark-sweep, `fase-4e-colector-mark-sweep` merged) — the first point since this project began that a live/dead distinction actually exists at runtime. `Weak<T>` (`MEMORY_AND_UNSAFE_SEMANTICS.md` §4, `openspec/specs/zirk-memory-safety/spec.md`'s own "Weak references" requirement) has had zero trace anywhere in the compiler until now, for the honest reason that there was nothing for it to observe — a reference that "does not keep its referent alive" is meaningless when nothing was ever reclaimed. That blocker is gone.

**Spec correction found while scoping this** (same class of gap as `Byte`/`extern` earlier this session): the existing "Weak references" requirement's own text says `upgrade(): Option<T>` and that it "returns `None`". `Option<T>`/`Some`/`None` do not exist anywhere in Zirk as real types or constructors — the language's actual nullable idiom, used everywhere else in this codebase (`match` narrowing on `null`, `??`, `T?` annotations), is `T?` with `null`. This proposal corrects that wording as part of delivering the requirement, not as an unrelated drive-by change — the corrected signature (`upgrade(): T?`) is what this change actually implements, so the spec needs to say that to remain accurate once this ships.

## What Changes

- New type `Weak<T>`, `T` restricted to a reference type (a class or contract instance — reuses the checker's existing `is_reference_type`, the same predicate `inmut::strict`'s alias matrix already uses) — `Weak<Int32>` or `Weak<SomeRecord>` are rejected, since a value type has no identity to observe weakly.
- `Weak.from(value: T): Weak<T>` — a static constructor, following the same "compiler-recognized static call, not routed through user generic-class construction" treatment `Pointer.from` already established.
- `.upgrade(): T?` — returns the live referent if it is still reachable through some strong reference, `null` otherwise. Corrects the existing spec's `Option<T>`/`None` wording to the language's real nullable idiom (see "Why").
- `.is_alive: Boolean` — an observational read of the same underlying state `.upgrade()` checks; documented (already, in the existing requirement) as a hint only, since a concurrent context could invalidate it immediately after — true in spirit today even though nothing concurrent exists yet (Phase 5), and becomes load-bearing once it does.
- The collector (`crates/zirk-runtime/src/collector.rs`) gains weak-reference support: a `Weak<T>` value does not keep its referent alive (it is never a mark root, and marking a `Weak<T>` handle itself does not trace into what it points at), and every live `Weak<T>` whose referent turns out to be unreachable after mark gets its target pointer cleared *before* sweep frees that referent — so `.upgrade()`/`.is_alive` never observe freed memory, by construction (not by a race against `dealloc`).

### Explicitly out of scope

- **A weak-referenced *collection element* or any bulk/indexed weak structure** — `Weak<T>` here is a single scalar handle to one object, matching the spec's own scope; collections are Phase 7.
- **`Weak<T>` across a concurrency boundary** (Transfer/Share derivation involving a weak handle) — `task`/`thread` are Phase 5; nothing to derive against yet.
- **Interning or deduplicating multiple `Weak.from(value)` calls on the same object** — each call allocates its own independent handle; both observe the same underlying liveness correctly, there is just no sharing of the handle's own storage. Simpler, and nothing in the spec requires identity between two `Weak<T>` values pointing at the same referent.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-memory-safety`: its existing "Weak references" requirement is corrected (`Option<T>`/`None` to `T?`/`null`, matching the language's real nullable idiom — no behavior change, a wording fix) and delivered by this change.
- `zirk-type-system`: `Weak<T>`'s element-type restriction (reference types only) and its operation set have no prior requirement naming them specifically — added as their own requirement, following the same pattern `fase-4e-unsafe-pointer-extern` used for `Pointer<T>`.
- `zirk-object-memory`: the collector's weak-clearing pass (a referent's weak handles are nulled before it is freed, never after) has no prior requirement — added, extending the same capability `fase-4e-colector-mark-sweep` added its header-bookkeeping requirement to.

## Impact

- Affected code: `crates/zirk-sema` (`Base::Weak` type, `is_reference_type` reuse for the element-type restriction, `Weak.from`/`.upgrade()`/`.is_alive` typing — following `Pointer<T>`'s own precedent for a compiler-recognized static call), `crates/zirk-ir` (a `WeakCell` allocation shape and its own instructions), `crates/zirk-codegen-llvm` (lowering those instructions), `crates/zirk-runtime/src/collector.rs` (a weak-clearing pass between mark and sweep; a WeakCell allocation is otherwise an ordinary collector-tracked object).
- Public documentation: `docs/init/ZIRK_ROADMAP.md` Phase 4e's "Implement safe/weak/dependent references, deep clone graph semantics and automatic bounded native pinning" bullet gets a partial-delivery note (`Weak<T>` only — safe/dependent references, `Clone`, and pinning remain their own work); `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md` memory rows updated. `../zirk-lang-site` sync required per project convention once these land.
- No breaking changes: `Weak<T>` is a new type; nothing that compiles today changes behavior.
