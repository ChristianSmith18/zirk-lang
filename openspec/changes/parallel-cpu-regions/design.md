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

## Open Questions

- `chunk` in this change or deferred? Proposed: grammar reserved now,
  implementation deferred to a follow-up.
- Does `.parallel` outside a block get a `cores` argument
  (`coll.parallel(cores: -1)`) or always runtime-picked? Proposed: runtime-picked
  here; a `cores:` arg is a later addition.
- Ordered vs unordered pipeline default: proposed ordered; `.parallel.unordered`
  opts out.
