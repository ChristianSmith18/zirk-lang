## Context

`docs/decisions/ADR-003-memoria.md`'s "Closing the decision" (this session) already fixed the strategy: non-moving mark-sweep, function-granularity shadow stack, cooperative allocation-triggered collection. This document covers the implementation-level decisions that ADR text doesn't (or restates the ones it does, at the granularity code needs).

Two facts about the existing pipeline make the chosen design tractable, both confirmed by reading the code, not assumed:
- Every function's `Slot`s are all `alloca`d together in the entry block (`crates/zirk-codegen-llvm/src/emit.rs`, `FunctionEmitter::emit`) — LLVM `mem2reg`-friendly, and exactly the shape a function-granularity (not per-scope) shadow stack needs.
- `Terminator::Return` is the *only* way a function ends (`crates/zirk-ir/src/ir.rs`); Zirk's own exceptions propagate via an explicit pending-exception check and ordinary early return (`fase-4b`'s D1-D3), never LLVM unwind tables — so there is exactly one exit point shape to instrument, not two.

## Goals / Non-Goals

**Goals:**
- Unreachable managed memory, including cycles, is actually reclaimed — `zirk_rt_alloc` stops being "never frees."
- Object identity and address stability are preserved (non-moving) — nothing that already assumes a stable address (`Pointer.from`, `is`, dispatch by descriptor) needs to change.
- No live object is ever collected — this is the load-bearing correctness property of the whole change; see D4 below for the specific hazard found and closed.
- Collection is fully transparent to a Zirk program: no observable pause-timing guarantee is made (out of scope per ADR-003 criterion 4), but no *incorrect* behavior is introduced either.

**Non-Goals:** (see proposal's "Explicitly out of scope" — not repeated here.)

## Decisions

### D1: Object header grows to three words; the mark bit lives in the new `next` field, not the existing descriptor word

```
today:      [ descriptor ]
this change: [ descriptor | next (low bit = mark) | size ]
```

`descriptor` is untouched — every existing reader (`zirk_rt_contract_table`, `zirk_rt_check_cast`, `zirk_rt_is_instance`, and every codegen dispatch/cast site) keeps reading it exactly as today, unmasked. `next` is new and read/written only by the collector itself (the intrusive all-allocations list sweep walks), so hiding the mark bit in its low bit (pointers are aligned; the low bit is free) costs nothing anyone else has to account for. `size` is the byte size `zirk_rt_alloc` already receives as a parameter at every call site — stored so sweep can `dealloc` correctly without recomputing it from the descriptor.

Alternative considered: steal a bit from the existing descriptor word instead of adding a `next` field. Rejected — every one of the three runtime functions above, plus every codegen site that dispatches through the descriptor, would need to mask the bit out; touching all of them to save one word per object is a bad trade, and `ADR-012` explicitly reserved header growth for exactly this situation ("whatever Phase 4 needs... gets added to the header").

### D2: Root enumeration is a function-granularity shadow stack, computed statically per function

At compile time (`zirk-ir` lowering), every function gets a static root descriptor: the list of slot offsets (within that function's alloca'd frame) that hold a managed reference. At runtime, `zirk_rt_push_frame(descriptor, frame_base)` runs once at function entry (after every `alloca`, after D4's zero-init below), and `zirk_rt_pop_frame()` runs once before every `Terminator::Return` lowers to LLVM `ret`. The collector's mark phase walks the linked stack of pushed frames; for each, walks its static descriptor to find every live root's actual address (`frame_base + offset`).

No per-block/per-scope push/pop is needed — see Context above for why function granularity is sufficient here (all slots already share one allocation lifetime, the function's).

Alternative considered: LLVM statepoints (`llvm.experimental.gc.statepoint`). Rejected for this pass — `inkwell` 0.10 (this project's LLVM binding) exposes `FunctionValue::set_gc()` (the `gc` function attribute) but no wrapper for the statepoint intrinsics themselves; they would need to be declared and emitted by hand, with fine-grained interaction with every potential-allocation call site and real risk around how LLVM's own optimization passes handle them (documented mostly through JIT compilers like the JVM's or Julia's, not general-purpose ahead-of-time compilers like this one). The shadow stack is markedly simpler given D2's own precondition already holds here for free.

Alternative considered: conservative stack scanning (Boehm-Demers-Weiser style). Rejected — zero codegen changes to enumerate roots, but introduces false-positive retention (an integer that happens to look like a valid heap address) exactly where ADR-003 is strict about identity and defined behavior, for no real savings here since the shadow stack is already cheap given this compiler's existing slot representation.

### D3: Collection trigger is a threshold check inside `zirk_rt_alloc`, single-threaded, stop-the-world

`zirk_rt_alloc` tracks live-byte count (incremented on allocation, decremented on sweep-reclaim); when a configured threshold is crossed, it runs a full collection before serving the new allocation. No concurrency exists yet (`parallel`/`thread` are Phase 5), so "stop the world" requires nothing special — there is nothing else running to stop. Revisiting trigger/pause behavior under real concurrency is explicitly Phase 5's own work (per ADR-003's closure), not this change's.

### D4: Every managed-reference-typed SSA value is spilled to a synthetic slot immediately on production (the correctness-critical finding)

A shadow stack that only tracks named `Slot`s cannot see a managed-reference value that lives purely as an LLVM SSA register between its creation and its first (if any) store to a named local — e.g., evaluating `f(SomeClass(a), SomeClass(b))` left to right, `SomeClass(a)`'s result has nowhere to be found by the collector if evaluating `SomeClass(b)` triggers D3's threshold check. `zirk-ir` lowering closes this by giving every instruction whose result type is a managed reference (`Object`, a `Closure` that captures one, a `Value`/`Enum` that contains one) its own synthetic slot, written immediately after the instruction that produces it, before that value participates in any further expression. This slot is included in D2's root descriptor exactly like a named local.

This reuses a technique already present in this codebase for an unrelated reason: `lower_throws_check` (`fase-4b`, and the ordering fix documented in `native-runtime-errors-catcheable`'s own design D11) already spills a value to a slot to keep it valid across a block boundary its own lowering introduces. Here the same move closes a GC-soundness hazard instead of a block-validity one — same mechanism, different justification.

Alternative considered: only spill values that are *provably* live across a call that might allocate (a liveness analysis). Rejected for this pass — real dataflow analysis this change does not need to build yet; spilling unconditionally costs an extra store per managed-reference-typed instruction result, accepted as the safe, simple starting point (the same kind of trade-off `fase-4e-unsafe-pointer-extern`'s D4 already made for `Pointer<T>` escape checking).

### D5: Reference-typed slots are zero-initialized immediately after their `alloca`

An `alloca`'d slot's initial bytes are whatever the stack happened to hold — not null. Between a slot's declaration and its first real write, if a collection runs, the shadow stack would read garbage as a candidate pointer. Every slot whose type is a managed reference gets an explicit store of the null/zero representation immediately after its `alloca`, before any other codegen for that function runs — mirrors `zirk_rt_alloc`'s own existing reason for zeroing a fresh object ("a partially built object [is] readable rather than a window onto whatever the allocator last held there" — same justification, applied to stack slots instead of heap objects).

### D6: Mark walks live objects via each object's own header-derived layout; sweep is an intrusive singly-linked list over all allocations

Mark: for each root (D2), read the pointer; if non-null and unmarked, set its mark bit (D1) and recurse into every managed-reference field its layout (already known from the object's own descriptor/class layout — the same information `zirk_rt_check_cast` already walks) names. Sweep: walk the `next`-linked list of every allocation ever made (D1); for each, if unmarked, `dealloc` it (using its stored `size`, D1) and unlink it from the list; if marked, clear the mark bit for the next cycle and leave it linked.

## Risks / Trade-offs

- **[Risk] D4's unconditional spill adds a store per managed-reference-typed intermediate value, even in hot paths that never actually cross an allocation.** → Accepted: correctness first: this is the same class of trade-off the project has already made repeatedly this session (e.g. `fase-4e-inmut-strict-proyeccion`'s conservative escape rule); a liveness-based refinement is legitimate future work once this is verified correct.
- **[Risk] A missed root (a managed-reference value the compiler fails to spill or include in a descriptor) is a silent use-after-free — the worst failure mode this project has, worse than a compile error.** → Mitigation: D4 and D5 together are the two places this could go wrong; both get dedicated test coverage (allocation pressure during multi-argument evaluation, mirroring `docs/decisions/ADR-003-investigacion-fase-4.md`'s own probe 5/10 style — construct real allocation pressure and confirm nothing that should still be reachable gets collected).
- **[Risk] `zirk-runtime`'s collector is the first genuinely stateful, cross-cutting piece of runtime this project has built (existing runtime functions are each independently correct; mark/sweep/allocate interact).** → Mitigation: unit-test each phase (mark, sweep, threshold trigger) independently against a synthetic object graph before relying on end-to-end `.zrk` program tests to catch interaction bugs.
- **[Trade-off] No incremental/generational collection — every trigger does a full stop-the-world mark-sweep.** → Accepted per ADR-003's own closure: pause-budget tuning is validation *of* this implementation, not a precondition; simplest-correct-thing-first.

## Migration Plan

Additive over a pipeline that already compiles and runs end to end. `zirk_rt_alloc`'s signature is unchanged from the IR/codegen's perspective (still takes size/align); its internal behavior changes from "never frees" to "frees when unreachable." No program that compiles today changes what it prints; long-running allocation-heavy programs change from unbounded RSS growth to bounded. Rollback: revert the merge — `zirk-runtime`'s change is isolated to its own new module plus `memory.rs`'s existing function bodies, `zirk-ir`/`zirk-codegen-llvm` changes are additive (new fields, new instrumentation) so a revert restores the exact "never frees" behavior.
