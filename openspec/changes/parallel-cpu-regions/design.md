## Context

The executor from #2 is single-threaded and cooperative — right for I/O, useless
for CPU throughput. `ADR-003` (collector) and `ADR-017` (executor) both
explicitly deferred multi-threading: *"the full stop-the-world question returns
with steps 4–6"*. This change is those steps for the `parallel` surface.

## Goals / Non-Goals

**Goals**: `parallel { }` with `cores` options, auto-parallel `for` and pipelines
inside it, `.parallel` adapter, `Parallel.each`, a worker pool, a stop-the-world
safepoint collector, Transfer/Share enforcement at the `parallel` boundary.

**Non-Goals**: `Mutex` / `RwLock` / `Semaphore` / `Barrier` / `Once` / `Atomic`
(#5), `Thread.run` (#5), making the *cooperative executor* multi-threaded (it
stays one thread; only the worker pool is multi-threaded), moving GC, changing
the object header.

## Decisions

### D1: Two schedulers, not one

The cooperative executor stays single-threaded (I/O branches). A separate
**worker pool** (`pool.rs`, N OS threads) runs `parallel` work. A branch that
enters a `parallel` region submits the region's tasks to the pool and **blocks**
(cooperatively — other I/O branches still run) until the region joins. Pool
threads run only pure-CPU Zirk code with no safe points and no I/O.

**Alternative**: make the executor itself M:N multi-threaded (Go model).
Rejected for this change — it drags every I/O branch into shared-memory
concurrency and a far larger collector change. Two schedulers keep the blast
radius at "CPU regions only".

### D2: `parallel { }` parallelizes `for` and pipelines lexically

Inside the region: a `for x in coll { ... }` splits `coll` into chunks across
pool threads; `coll.map(f).filter(g).reduce(...)` runs as parallel combinators.
Outside a region these are sequential unless the `.parallel` adapter is used.
Ordered operations preserve input order regardless of completion order; a reduce
requires an associative combiner (checked; a `parallel.reduce_ordered` variant is
the deterministic escape).

**Alternative**: require `.parallel` everywhere, no block magic. Kept as the
adapter for single chains; the block earns its place for a *region* of several
parallel operations sharing one core budget, matching OpenMP `parallel`.

### D3: `cores` option grammar and semantics

`parallel; cores: N; chunk: M { ... }` — `parallel`, then `;`-separated
`name: expr` options, then `{`. Contextual: `parallel` as a leading token in a
block position switches the parser to option-header mode until `{`. `cores`:
`N>0` exact; `-N` = `available - N` (error if `≤ 0`); `A..=B` range (Zirk range
syntax), runtime picks; absent = all. Evaluated once at region entry.

**Alternative**: `parallel(cores: N) { }` (function-call shape). Rejected by the
language author; the `;` header is the chosen form. Fallback if the contextual
parse proves unworkable: comma-separated in parens.

### D4: Stop-the-world safepoint collector

`collector.rs`: allocation on a pool thread (or the executor thread) that hits
the GC threshold raises a global "collection requested" flag. Every pool thread
polls a safepoint at loop back-edges inside a `parallel` region and parks when
the flag is set; the executor thread parks at its next scheduling turn. When all
threads are parked, one thread walks all roots — the executor's current branch
chain, every suspended branch chain, and each parked pool thread's shadow-stack
chain — marks, sweeps the shared intrusive allocation list under a lock, clears
the flag, releases. Non-moving mark-sweep and the 3-word header (`ADR-012`) are
unchanged.

**Alternative**: per-thread heaps + no stop-the-world. Rejected — `ADR-003`
already chose one shared non-moving heap and a shadow stack; keeping one
root-enumeration strategy is worth the safepoint machinery.

### D5: Transfer/Share at the `parallel` boundary

Reuses #2's analysis. Values and projections copy in; strict immutable complete
references share; a mutable alias the parent keeps using is rejected (transfer,
strict-share, clone, or channel). A `parallel` region SHALL NOT perform I/O, a
blocking channel op, `Timer.sleep`, or `spawn` — the checker rejects them (pool
threads have no safe points).

## Risks / Trade-offs

- **The collector change is the highest-risk work in the whole five-change set.**
  → Mitigation: land `pool.rs` + safepoint behind a feature flag with the pool
  size forced to 1 first (behaviourally identical to today), verify the whole
  suite, then raise the pool size. Stress test: allocation-heavy `parallel`
  reduce under a low GC threshold on all four target triples.
- **`parallel` region doing accidental I/O** → static rejection (D5) with a clear
  message; runtime assert as a backstop.
- **`cores: -N` on a 1-core CI machine** → runtime error at region entry with the
  detected core count in the message.
- **Contextual `;` header parsing** → if it fights the grammar, D3's paren
  fallback; decide in task 3.x before committing the parser.
- **Nested `parallel` regions** → the inner region shares the outer's pool (no
  new threads); `cores` on the inner is clamped to the outer budget.

## Migration Plan

1. `ADR-019` + `ADR-003`/`ADR-017` addenda; spec deltas; validate.
2. `pool.rs` with size forced to 1; safepoint scaffolding; full suite green.
3. `collector.rs` stop-the-world + multi-root walk; stress tests per triple.
4. Raise pool default to hardware concurrency; re-run stress tests.
5. Lexer/AST/parser: `parallel` keyword + `;` option header (decide grammar).
6. Sema: `ParallelSeq`, `Parallel.each`, associativity check, boundary rules,
   I/O-in-region rejection.
7. IR/codegen: `ParallelRegion`, parallel `for` + pipelines, safepoint polls.
8. Fixtures + `examples/parallel_examples.zrk`.
9. Docs; `cargo test` + fmt + clippy; commit; website sync with reviewed date.

## Implementation addenda (sema, task 5)

Recorded once implementation reached the checker and found the design's
assumptions did not all hold against the current codebase.

**D2 addendum — `.parallel` is a typing no-op.** Spec `zirk-type-system`'s own
scenario says `rows.parallel.map(cost).sum()` "is typed exactly as the
sequential `rows.map(cost).sum()`". Taken literally: `.parallel` does not
introduce a `ParallelSeq<T>` type at all — `check_field` returns the receiver's
own type unchanged and records the field-expression span in
`parallel_adapter_accesses` for lowering to consume later. This also sidesteps
a real gap found while implementing it: **`map`/`filter`/`reduce`/`sum`/
`count`/`collect`/`for_each` do not exist on `List<T>`/`Array<T>` yet, not even
sequentially** — so "expose the parallel-safe subset" has no subset to define
today. `Parallel.each(coll, fn)` needed none of those methods (it is a
self-contained static call, `check_parallel_each_call`, typed `List<R>` off the
callback's own return type) and is implemented.

**D3 addendum — reduce associativity is future work, not implementable yet.**
The same gap blocks 5.3: there is no `.reduce` call site in the checker to
attach an associativity check to. Recorded for whoever adds `.reduce`: reject
a combiner lambda whose body's top-level operator is not one of
`+ * & | ^ && ||` (conservative — real associative operations only), requiring
`reduce_ordered` otherwise. This directly rejects the spec's own example,
`(a, b) => a - b`.

**D5 addendum — boundary reuse via the capture/barrier mechanism.** #2
(`concurrent-blocks-and-timers`) turned out not to have a dedicated
boundary-alias check to reuse — only the general `mut`/`inmut::strict` matrix
(`check_strict_alias`, `STRICT_ALIAS_VIOLATION`) exists. Implemented by reusing
*that* mechanism at the region boundary: `check_parallel_block`/
`check_parallel_expr` open a capture-tracking barrier scope (the same
`begin_capture_scope`/`finish_capture_scope` pair `spawn` and lambdas use, which
also correctly isolates `loop_depth` — `break`/`continue` cannot cross into a
`parallel` region from outside or escape one), then `check_parallel_boundary`
rejects a captured reference-type binding whose declared mutability is `mut`
with `STRICT_ALIAS_VIOLATION`. A `return` inside a `parallel` region is left
targeting the enclosing function, same as `unsafe {}`/`commit {}` — its
runtime semantics across worker threads are an IR/codegen question, not
addressed here.

## Implementation addenda (IR/codegen, task 6)

**D1/D4 addendum — a real, pre-existing GC-rooting bug, found and fixed.**
Verifying `try_lower_parallel_for`'s work-splitting end to end (compiling and
running real programs under `ZIRK_GC_THRESHOLD` pressure, not just unit-testing
each piece in isolation) turned up a crash. Isolating it further — by
reproducing the *same* crash with a plain sequential `for x in list { }` and
no `parallel` anywhere — proved it was not something this change introduced:
`crates/zirk-codegen-llvm/src/emit.rs`'s `gc_reference_paths` was missing
match arms for `IrType::Array(_)`/`List(_)`/`Range`/`Map(_)`/`Set(_)`. Those
five types are correctly flagged by `IrType::is_managed_reference` (so a slot
holding one gets zero-initialized at function entry, and is *counted* in
`Function::gc_roots`), but the separate table that turns a gc-root slot into
an actual shadow-stack root *address* fell through to `_ => {}` for all five —
producing zero root addresses. A local `List<T>` (or `Array<T>`/`Range`/
`Map<K,V>`/`Set<T>`) variable was therefore invisible to every collection: the
collector could — and, under a small enough threshold, did — reclaim it (or
the buffer/elements it alone referenced) while the program was still using it.
`List<T>` exposed this easily, since each `.add()` can trigger its own
buffer-growth allocation, i.e. its own chance for a collection to land while
nothing else roots the list; `Array<T>` (built in one allocation, filled with
no further allocation) happened not to expose it under ordinary use.

Fixed by adding those five types to the same arm `Object`/`Contract`/`Weak`/
`Dependent`/`Pin`/`String`/`Char` already use (each is "one managed pointer at
the slot itself," with the object's own descriptor — not this table — telling
`mark_object` how to trace through it). Verified: the original 10-element
`List<Int32>` repro (`ZIRK_GC_THRESHOLD=200`) passes 20/20 runs; a 200-element,
allocation-heavy, 4-pool-core stress case that previously hung/crashed passes
15/15; a dedicated regression test was added
(`crates/zirk-cli/tests/end_to_end.rs`,
`a_list_survives_repeated_buffer_growth_under_a_small_threshold`) and confirmed
to fail without the fix. Once this was root-caused and fixed, the
work-splitting lowering itself needed no further changes — it was correct all
along; it was only ever exposed *by* this bug, not the cause of a separate one.
`cargo test --workspace` (1277 tests), `cargo fmt --check`, and
`cargo clippy --workspace --all-targets` are all clean with the fix in place.

## Formal dependency: `collection-sequence-pipeline` (filed 2026-09-12)

Tasks 5.2 (`.parallel`'s real per-method adapter behavior, `ParallelSeq<T>`)
and 5.3 (the associativity check on a parallel `reduce`) are blocked on a
gap this change found but does not own fixing: `List<T>`/`Array<T>` have no
`map`/`filter`/`reduce`/`sum`/`count`/`collect`/`for_each` at all, not even
sequentially. `openspec/changes/collection-sequence-pipeline` is the formal
proposal that adds this surface (validated, not yet implemented as of this
writing) across `List<T>`/`Array<T>`/`Range<T>`/`Tuple`. Its own design
explicitly defers the associativity check to task 5.3 here rather than
implementing it there — `reduce`'s call site is what 5.3 needs, not the
check itself. Resume 5.2/5.3 once that change lands.

## Open Questions

- `chunk` in this change or deferred? Proposed: grammar reserved now,
  implementation deferred to a follow-up.
- Does `.parallel` outside a block get a `cores` argument
  (`coll.parallel(cores: -1)`) or always runtime-picked? Proposed: runtime-picked
  here; a `cores:` arg is a later addition.
- Ordered vs unordered pipeline default: proposed ordered; `.parallel.unordered`
  opts out.
