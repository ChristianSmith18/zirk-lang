# ADR-003 — Memory strategy: constraints now, implementation in Phase 4

- **Status:** accepted (constraints and choice of strategy; see "Closing the decision" — the collector's implementation is ongoing Phase 4e work, it does not block the status of this ADR)
- **Date:** August 12, 2026 (constraints) — closed August 24, 2026
- **Phase:** 0 (constraints) → 4e (closing and implementation)

## Context

`ZIRK_ROADMAP.md` calls this *the project's highest-leverage decision* and places it before any code is written. But the Phase 1 subset (`main`, `println`, `Int32`, literals, `if`/`else`) allocates practically nothing: choosing "generational GC" today would be an indefensible decision with no data, made about a language that does not yet exist.

The symmetric risk is real: if nothing is written down, Phase 1's IR is born with tacit assumptions about memory that later cannot be removed.

## Decision

The **constraints are closed** now and the **concrete choice is anchored to Phase 4**.

### Constraints derived from the spec (non-negotiable)

These are not preferences: they are deduced from the normative documents and narrow the design space far more than the open question "GC, RC, regions or hybrid?" suggests.

| Constraint | Source | Implication |
|---|---|---|
| Ownership and RC are **not** public semantics | `SPEC_FINAL` §3 | nothing Rust-style at the language surface |
| Cycles must be freed correctly | `RUNTIME_SPEC` §9 | **pure RC is ruled out**: tracing or a cycle collector is needed |
| Stable identity even if the object physically moves | `RUNTIME_SPEC` §9 | a moving GC requires handles or pinning; this clashes with the C ABI boundary (`LANGUAGE_SPEC` §13) |
| No general-purpose destructors with an observable moment | `RUNTIME_SPEC` §9 | **the GC needs no finalizers** — a major simplification |
| External resources are closed via `Resource<E>` + `match with`, not via memory release | `RUNTIME_SPEC` §10 | the lifetime of files, sockets and locks is independent of the GC |
| Pauses and consumption must be measurable | `RUNTIME_SPEC` §9 | GC is allowed, but with an explicit and observable budget |
| `parallel` and `thread` are real and multi-core | `RUNTIME_SPEC` §6, §7 | whatever is chosen must be thread-safe by design, not adapted afterward |
| Value types can be stored inline | `RUNTIME_SPEC` §9 | value classes and records do not pay for indirection |

### Likely direction (non-binding)

The remaining space points toward **non-moving tracing (or moving with handles) + escape analysis to promote to the stack + inline value types**. This is recorded as a working hypothesis, not a decision: the final choice requires a language with closures and real objects to measure against.

### Decision criteria for Phase 4

The choice will be closed by evaluating, on real Zirk programs:

1. behavior with cycles between objects and with capturing closures;
2. barrier cost (if any) in `parallel for`;
3. interaction with `Resource<E>` and with the C ABI boundary;
4. pauses measured against a declared budget.

## Consequences

- Phase 1's IR must **not** assume a concrete memory model: every allocation goes through an abstract IR operation, resolved by the runtime.
- `zirk-runtime` ([ADR-002](./ADR-002-runtime-staticlib.md)) is the single point where this decision materializes.
- This ADR is reviewed and replaced at the start of Phase 4. It is not considered closed until then.

## Closing the decision (August 24, 2026)

`docs/decisions/ADR-003-investigacion-fase-4.md` gathered evidence from real execution on `develop` across several Phase 4 sessions — not toy programs, the very criterion this ADR set as a condition for closing. Of the four criteria the "Decision criteria for Phase 4" section set:

1. **Behavior with cycles and with capturing closures** — measured (probes 1, 8-11): cycles are the natural result of two classes that reference each other, not a lab-only case; a closure that captures a shared object and escapes its creating frame (once `fase-4d-callables` made the case constructible) survives correctly, with identity and aliasing intact, even under sustained allocation pressure.
2. **Barrier cost in `parallel for`** — not yet measurable: `parallel`/`thread` are Phase 5 and do not exist. This criterion cannot be closed before that phase by construction, not for lack of effort — it is left as post-implementation validation, not as a precondition for choosing the strategy.
3. **Interaction with the C ABI boundary** — partially measurable since `fase-4e-unsafe-pointer-extern` (this session): `unsafe`/`Pointer<T>`/`extern` now exist; it remains as follow-up validation once the collector exists, to confirm that a pinned object exposed across the FFI boundary remains correct during and after the native call.
4. **Pauses measured against a budget** — not measurable without a collector; it becomes an acceptance criterion for the implementation, not for the choice of strategy.

**Final decision: non-moving tracing (mark-sweep), with root enumeration via a shadow stack at function granularity, cooperative triggering at the allocation site itself.**

- **Non-moving.** Preserves identity and stable address with no handles or extra indirection needed — consistent with the fact that `Pointer.from`/`is` already assume, since `fase-4e-unsafe-pointer-extern`, that an object does not change address. This also closes, by elimination, the question the original "likely direction" left open (moving-with-handles vs. non-moving): non-moving is strictly simpler and nothing built so far pays the cost of moving it.
- **Tracing (mark-sweep), not reference counting.** Confirmed by probe 1: cycles are an ordinary design pattern, not an edge case, so pure RC is ruled out exactly as the original constraint already anticipated. A mark-sweep collector needs no separate cycle collector: it frees cycles just like any other garbage.
- **Root enumeration: manual shadow stack at function granularity, not LLVM statepoints nor conservative scanning.** Investigated in this session against the actual LLVM binding (`inkwell` 0.10 exposes a function's `gc` attribute but no wrapper around the statepoint intrinsics — they would have to be emitted by hand, with tight coupling to every optimization pass). The shadow stack is cheaply viable here because every function already `alloca`s all of its `Slot`s at once in the entry block (`emit.rs`, confirmed in this session) and Zirk exceptions do not use LLVM unwind — there is only one kind of function exit (`Terminator::Return`) to instrument, no special exception paths.
- **Critical correctness issue found in this session, before any implementation:** a managed reference-type value that lives only as an SSA result (`ValueId`), never written to a `Slot`, is invisible to a shadow stack that only looks at named slots — a concrete example, `f(SomeClass(a), SomeClass(b))` could collect `SomeClass(a)` while evaluating the second argument, if the second one triggers the collector. The resolution: every managed reference-type value is spilled to its own synthetic slot as soon as it is produced, before taking part in any expression that could allocate — the same technique `lower_throws_check` (`fase-4b`) already uses for an unrelated block-validity reason, applied here for collector soundness.
- **Object header grows from one word to three** (`ADR-012` already reserved this on purpose): dispatch descriptor (unchanged), a new `next` pointer threading the intrusive list of everything allocated for sweeping, and the size of the allocation (so it can be freed correctly with `dealloc`). The mark bit hides in the low bit of the `next` pointer — an entirely new field that only the collector itself reads, so no existing site that already reads the descriptor unmasked (`zirk_rt_contract_table`, `zirk_rt_check_cast`, `zirk_rt_is_instance`, and the codegen sites that dispatch through it) needs to change.
- **Cooperative triggering inside `zirk_rt_alloc`**, without threads: if the configured threshold is exceeded, it collects before serving the allocation. Correct by construction until Phase 5 introduces real concurrency — at that point, triggering and "stop the world" need to be revisited, and this is noted as work for that phase, not for this decision. [ADR-017](./ADR-017-modelo-de-suspension.md) discharges the first half of this for Phase 5 steps 1–3: the executor is single-threaded and cooperative, so allocation-triggered collection stays correct as written, and the only change is that the shadow-stack head becomes one chain per task and `collect()` walks every live task's chain. Real stop-the-world work returns with steps 4–6.

### Discarded alternatives

- **LLVM statepoints (precise roots via LLVM's own intrinsics).** More "correct" in the sense that LLVM already knows how to optimize around them, but with no wrapper in the binding this compiler uses, poorly documented outside JIT compilers such as the JVM's or Julia's, and it interacts non-trivially with inlining and other optimization passes. Discarded for this first real implementation; left as a future improvement if the manual shadow stack turns out to be costly in practice.
- **Conservative stack scanning (Boehm-Demers-Weiser style).** Zero codegen changes to enumerate roots, but it introduces false positives (an integer that happens to look like a valid address retains garbage) exactly where this ADR is already strict about identity and undefined behavior. Discarded for now because the manual shadow stack, given that this compiler already tracks named slots, does not cost substantially more and does not pay that uncertainty.
- **Moving with handles.** Adds a permanent layer of indirection (every access to an object goes through a handle, not its address) that nothing built so far needs — neither `Pointer.from` nor descriptor-based dispatch assume indirection. Discarded until a concrete reason exists (for example, real compaction under fragmentation pressure) that justifies it.

## Consequences of closing

- `crates/zirk-runtime/src/memory.rs`'s "does not free, deliberately" stops being the final state: the collector's implementation (Phase 4e, in progress) replaces `zirk_rt_alloc`'s current body with a threshold-based version, and adds the mark-sweep module, the shadow stack, and the header growth described above.
- `ADR-012` (object layout) ends up exercised exactly as anticipated: the header grows without displacing the indices of the real fields.
- `Weak<T>` and the `Clone` contract (`MEMORY_AND_UNSAFE_SEMANTICS.md` §4 and §6) go from "no strategy to build on" to implementable — they are the natural extension once a real collector distinguishes alive from dead.
- Criterion 2 (barrier in `parallel for`) and criterion 4 (pauses against a budget) remain as post-implementation follow-up validation, not as a closing condition — this ADR documents why requiring them as a precondition would have been circular (they depended on phases later than the one this ADR gates).
- The shadow stack's single process-global head is generalized to one head per task by [ADR-017](./ADR-017-modelo-de-suspension.md) (Phase 5 steps 1–3): a task that suspends at `await` keeps its own root chain, and root enumeration walks every live task's chain. The SSA-spill rule above is what keeps a suspended frame's roots visible. The object header is unchanged.

## Addendum — `parallel-cpu-regions` (Phase 5 steps 4–6, `parallel` surface)

The deferred concurrency note — *"Correct by construction until Phase 5
introduces real concurrency — at that point, triggering and 'stop the world'
need to be revisited"* — is discharged for the `parallel` worker pool.

- **Triggering becomes thread-aware.** When the worker pool has active threads,
  an allocation that crosses the GC threshold (on a pool thread or the executor
  thread) SHALL raise a global "collection requested" flag instead of collecting
  inline. With no active workers, inline cooperative triggering is unchanged.
- **Stop-the-world safepoint.** Every worker thread polls a safepoint at
  `parallel` loop back-edges and parks when the flag is set; the executor thread
  parks at its next scheduling turn. When every thread is parked, one thread
  walks all roots — the executor's current branch chain, every suspended branch
  chain, each parked worker's shadow-stack chain, and `mark_clone_roots` — then
  marks, sweeps the shared intrusive allocation list under a lock, clears the
  flag, and releases every thread.
- **What does not change.** Non-moving mark-sweep, the one shared heap, the
  3-word object header (`ADR-012`), and the per-frame SSA-spill rule. Only
  triggering and root enumeration gain thread-awareness.

See [ADR-019](./ADR-019-parallel-and-multithreaded-gc.md) for the full worker
pool + safepoint design and the per-triple stress-test plan.
