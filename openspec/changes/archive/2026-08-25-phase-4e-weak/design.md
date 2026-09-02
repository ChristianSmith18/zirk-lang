## Context

`fase-4e-colector-mark-sweep` (merged) gave the runtime a real live/dead distinction: mark walks the shadow-stack roots and traces reachable objects; sweep frees whatever is left unmarked. `Weak<T>` needs to sit *outside* that trace — a `Weak<T>` handle must itself be an ordinary GC-tracked allocation (so it survives correctly while reachable, like anything else), but the pointer it holds to its referent must never be followed as a strong edge during mark, and must be nulled the instant mark determines that referent is unreachable, strictly before sweep frees it.

## Goals / Non-Goals

**Goals:**
- `Weak<T>` never keeps its referent alive.
- `.upgrade()`/`.is_alive` never observe freed memory — by construction, not by a timing accident.
- `Weak.from`/`.upgrade()` compose correctly with the shadow stack (design D4 of `fase-4e-colector-mark-sweep`): the `T?` `.upgrade()` returns is itself a managed reference and must be spilled to a root slot exactly like any other, with no special-casing needed.

**Non-Goals:** (see proposal's "Explicitly out of scope".)

## Decisions

### D1: `Weak<T>` is its own `Base` variant; its runtime representation is a small heap-allocated indirection cell ("WeakCell"), not a raw pointer to the referent

`Base::Weak(Box<Type>)`, parallel to `Base::Pointer` (`fase-4e-unsafe-pointer-extern`'s own D1) — compiler-built-in, closed operation set, no reason to route through user-generic machinery.

A `Weak<T>` *value* is a pointer to a WeakCell: an ordinary collector-tracked allocation (real header, real `next`/mark/size bookkeeping — task 1 of `fase-4e-colector-mark-sweep` already built this for every object) whose only payload is one field, the target pointer. `Weak.from(value)` allocates a fresh WeakCell and stores `value`'s address in it; the `Weak<T>` value the program holds is a pointer to that cell, not to the target directly.

Alternative considered: `Weak<T>` holds the target's address directly, with the collector maintaining a side-table (address → list of weak handles pointing at it) to know what to null on collection. Rejected — a side-table needs its own dynamic data structure (a hash map keyed by address) the collector does not otherwise need anywhere; the indirection-cell design needs nothing new beyond "one more kind of ordinary tracked allocation", reusing 100% of the existing allocation/header/mark/sweep machinery.

### D2: The WeakCell's target field is marked "weak" via a reserved sentinel descriptor, not a new header field

A WeakCell's header (word 0, the dispatch descriptor) is a fixed sentinel constant (a static symbol in `zirk-runtime`, e.g. `zirk_rt_weak_cell_descriptor`) instead of a real class descriptor — recognizable and distinct from every user class's own descriptor (those are compiler-emitted per-class tables; this is one fixed runtime symbol). Mark's generic object-tracing walk (which normally recurses into every field a class's layout names as a managed reference) special-cases this sentinel: a WeakCell is marked as reachable exactly like any other object when something roots it, but its own single field is *not* traced as a strong edge — mark stops at the cell itself.

Alternative considered: add a per-class-layout "this field is weak" flag to the general field-layout metadata, so any user class could in principle declare a weak field. Rejected — nothing in the spec exposes weak fields as a general class feature (`Weak<T>` is the only weak-reference surface the language defines); building general per-field weak-marking machinery for a feature that has exactly one caller (this change's own compiler-synthesized WeakCell) is speculative generality this change does not need.

### D3: Weak-clearing runs as its own pass between mark and sweep, over the same intrusive allocation list

`fase-4e-colector-mark-sweep`'s existing collection cycle is mark → sweep (free unmarked, clear survivors' mark bits) in one pass over the intrusive `next`-linked allocation list. This change inserts a pass between them: walk the same list once; for every allocation whose descriptor is the WeakCell sentinel (D2) and is itself marked (i.e., some `Weak<T>` handle still reaches it), read its target field — if the target's own mark bit is unset (unreachable, about to be freed), null the WeakCell's target field. This must run as a fully separate pass *before* sweep's own free pass, because it needs to read the target's mark bit, which sweep's free pass would otherwise have already destroyed by freeing that memory.

Collection cycle becomes: mark → clear dead weak cells (this change) → sweep (unchanged from `fase-4e-colector-mark-sweep`).

### D4: `.upgrade()` and `.is_alive` are ordinary field reads/checks on the WeakCell, gated by nothing collector-specific at the language level

`.upgrade()` reads the WeakCell's target field: null → the checker's own nullable-result convention (`T?` returning `null`, task-level; no new IR concept needed, this is the same shape any nullable-returning operation already has); non-null → returns the target pointer as an ordinary `T` value, which — like every other managed-reference-typed result in this compiler since `fase-4e-colector-mark-sweep`'s own D4 — gets spilled to a synthetic root slot immediately, with no special handling needed here: that mechanism is already unconditional for every managed-reference-typed instruction result. `.is_alive` is the same null-check without producing a new strong reference.

## Risks / Trade-offs

- **[Risk] A WeakCell allocated once per `Weak.from()` call is one more small allocation per weak handle**, unlike a design that reuses one cell per target. → Accepted: matches proposal's own "explicitly out of scope" on interning; simpler, and the collector already reclaims a WeakCell itself once nothing references it, same as anything else.
- **[Risk] The weak-clearing pass (D3) adds a full extra O(n) walk of the allocation list to every collection cycle**, even when no `Weak<T>` exists in the running program. → Mitigation: track whether any WeakCell has been allocated at all (a simple counter, incremented in `Weak.from`'s own allocation path); skip the whole pass when it is zero — a program that never uses `Weak<T>` pays nothing beyond one counter check per collection.
- **[Trade-off] The sentinel-descriptor approach (D2) means a WeakCell is recognizable by *identity* of its descriptor pointer, not by any tag stored elsewhere** — correct and cheap, but means anything that inspects an object's descriptor for other reasons (dispatch, cast) must never be handed a WeakCell's descriptor by mistake; this is naturally true today (WeakCell is never a user-visible class, never appears in a checked cast or method dispatch), documented here so a future reader knows why this constraint matters if that ever changes.
