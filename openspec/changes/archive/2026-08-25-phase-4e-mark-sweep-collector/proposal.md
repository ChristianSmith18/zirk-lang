## Why

`docs/decisions/ADR-003-memoria.md` closed today (August 24, 2026): non-moving mark-sweep, function-granularity shadow stack for root enumeration, cooperative collection triggered at allocation. `crates/zirk-runtime/src/memory.rs` has, since Phase 3, deliberately never freed anything — "a program of this phase terminates and the operating system reclaims everything... that is the bound, and it is written down rather than assumed." That bound stops being acceptable once a program runs long enough to matter (already measured: probe 5 of `docs/decisions/ADR-003-investigacion-fase-4.md` put a real number on it — ~32 bytes/object, unbounded growth). This change builds the real collector ADR-003 now specifies: not a placeholder, not a partial slice deferred like `fase-4e-unsafe-pointer-extern`'s own journal — the whole mark-sweep-and-sweep-back-into-the-allocator cycle, correct end to end.

This is the piece the rest of Phase 4e's remaining bullets (`Weak<T>`, deep `clone()`, dependent references) sit on top of: none of them have a live/dead distinction to build against until this exists.

## What Changes

- **Object header grows from one word to three** (dispatch descriptor unchanged; a new intrusive `next`-allocation pointer threading every live object for sweep, with the mark bit hidden in its low bit; the allocation's own size, needed to `dealloc` correctly): `crates/zirk-ir`'s object layout and `crates/zirk-codegen-llvm`'s header emission both grow accordingly. No existing header-reading site (`zirk_rt_contract_table`, `zirk_rt_check_cast`, `zirk_rt_is_instance`, or any codegen dispatch/cast read of the descriptor word) changes — the descriptor word itself is untouched; only new fields are added after it.
- **Every managed-reference-typed SSA value gets spilled to a synthetic slot immediately on production**, before it can participate in any expression that might allocate — closing the "value only lives in an LLVM register" hazard found while designing this change (a shadow stack that only sees named `Slot`s cannot protect a value that never became one). Reuses the same technique `lower_throws_check` (`fase-4b`) already applies for an unrelated cross-block-validity reason.
- **A per-function static root descriptor**, computed once at compile time from the function's own slot table (real named locals plus the new synthetic slots above): which slot offsets hold a managed reference. Every managed-reference-typed slot is additionally zero-initialized immediately after its `alloca`, so the shadow stack never reads uninitialized garbage as a pointer.
- **`zirk_rt_push_frame`/`zirk_rt_pop_frame`**: push the function's root descriptor and its stack-frame base address on entry (after allocas, after zero-initializing reference slots); pop before every `Terminator::Return` — the only function-exit terminator this IR has, so no exception-unwind path needs separate instrumentation (Zirk's own exceptions already propagate through an explicit pending-exception check and ordinary early return, not LLVM unwind tables).
- **A real mark-sweep collector in `crates/zirk-runtime`**: mark walks the pushed-frame stack, then each object's own header-derived layout to trace reachability transitively; sweep walks the intrusive all-allocations list, `dealloc`s anything unmarked (using the header's stored size), and clears the mark on survivors for the next cycle.
- **`zirk_rt_alloc` gains a threshold check**: when the live-byte count crosses a configured threshold, a collection runs before the new allocation is served. Single-threaded only (matches the runtime's current state — no `parallel`/`thread` exist yet; Phase 5 revisits triggering under real concurrency).

### Explicitly out of scope

- **`Weak<T>` and the `Clone` contract** — natural next uses of a live/dead distinction that now exists, but each is its own surface (checker + runtime API), not part of building the collector itself. Tracked as Phase 4e follow-up work, not this change.
- **Moving/compacting collection** — ADR-003 closed on non-moving; nothing here builds a mover.
- **Multi-threaded collection / concurrent marking** — Phase 5 (`parallel`/`thread`) does not exist yet; "stop the world" is trivial today because nothing else is running. Revisiting the trigger and pause model under real concurrency is that phase's own work.
- **Generational collection, incremental/concurrent marking, or any pause-budget tuning** — ADR-003's own criterion 4 (pauses against a budget) is validation *of* this implementation, not a precondition for building it; a simple stop-the-world mark-sweep is the correct first cut to validate against, not a premature optimization target.
- **Verifying criterion 3 (the ABI/`unsafe` boundary) against a real collector** — now possible once this change lands (a `Pointer.from`-pinned object surviving a collection cycle during an `extern` call is exactly the probe ADR-003 asked for); left as a follow-up probe once this merges, not part of building the collector itself.

## Capabilities

### New Capabilities
(none — extends `zirk-object-memory` and `zirk-memory-safety`)

### Modified Capabilities
- `zirk-object-memory`: its existing "Object header" requirement already mandates a header preceding fields; a new requirement is added for the collector-bookkeeping fields the header now also carries (next-allocation link, mark, size) — genuinely new, no prior requirement named this.
- `zirk-memory-safety`: its existing "Strategy-neutral automatic memory" and "Deterministic cleanup belongs to resources" requirements are reaffirmed verbatim, delivered by this change (both already describe exactly this behavior: reclaim unreachable memory including cycles, without exposing GC/RC/moves as source semantics, no observable destructor timing — this change is the first to actually make the first sentence true; the second was already vacuously true since `Resource<E>` never depended on memory reclamation).

## Impact

- Affected code: `crates/zirk-ir` (object/value layout gains the new header fields; lowering gains the synthetic-slot-spill pass and per-function root descriptor), `crates/zirk-codegen-llvm` (header emission, zero-init of reference slots, `push_frame`/`pop_frame` calls around entry/`Return`), `crates/zirk-runtime` (new collector module: mark, sweep, the grown `zirk_rt_alloc`, `zirk_rt_push_frame`/`zirk_rt_pop_frame`).
- `docs/decisions/ADR-003-memoria.md` (closed this session, "Closing the decision") is the strategy decision this change implements; no further ADR needed for the collector's internal shape (already spelled out there).
- Public documentation: `docs/init/ZIRK_ROADMAP.md` Phase 4e gets its "Deliver the strategy chosen by the Phase 0 memory ADR" bullet marked delivered (the collector itself; `Weak<T>`/references/clone remain their own bullets); `docs/handbook/13-appendices/07-current-limitations.md` and `12-feature-status.md` get their memory rows updated. `../zirk-lang-site` sync required per project convention once these land.
- No breaking changes: every program that compiles and runs today keeps doing so — this changes *when* memory is reclaimed (now: when unreachable), never program-observable behavior the language guarantees (identity, equality, mutation rules, all explicitly preserved by ADR-003's own constraints).
