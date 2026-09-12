# ADR-019: Parallel CPU regions and the multi-threaded collector

## Status

Accepted

## Context

Phase 5's `concurrent-blocks-and-timers` delivered I/O concurrency on a
single-threaded cooperative executor: work that mostly *waits* overlaps well,
work that mostly *computes* still runs on one core. `ADR-003` (collector) and
`ADR-017` (executor) both explicitly deferred real multi-threading —
*"the full stop-the-world question returns with steps 4–6"*. This ADR is those
steps, scoped to the `parallel` surface only.

## Decision

### Two schedulers, not one

The cooperative executor stays single-threaded and keeps owning every I/O
branch. A separate **worker pool** (`crates/zirk-runtime/src/pool.rs`, N OS
threads) runs `parallel` work. A branch that enters a `parallel` region splits
the region's work, submits it to the pool, and blocks **cooperatively** — the
executor keeps servicing other I/O branches — until the region joins. Pool
threads run only pure-CPU Zirk code: no I/O, no channel operations, no
suspension, no `spawn`.

Rejected alternative: an M:N multi-threaded executor (Go model). It drags every
I/O branch into shared-memory concurrency and forces a far larger collector
change. Two schedulers keep the blast radius at "CPU regions only".

### `parallel { }` parallelizes `for` and pipelines lexically

Inside a region, `for x in coll { ... }` splits `coll` into chunks across pool
threads and `coll.map(f).filter(g).reduce(...)` runs as parallel combinators.
Outside a region these stay sequential unless the `.parallel` adapter is used.
Ordered operations preserve input order regardless of completion order; a
parallel `reduce` requires an associative combiner (checked), with
`reduce_ordered` as the deterministic escape. The block MAY be used as an
expression and yields the value of its final expression.

### The `cores` option

`parallel; cores: N; chunk: M { ... }` — the keyword, then `;`-separated
`name: expr` options, then `{`. `cores`: `N > 0` exact; `-N` = `available - N`
(runtime error if `≤ 0`); `A..=B` an inclusive range the runtime picks within;
absent = all available. `cores: 0` is a compile-time error. Evaluated once at
region entry. A nested region shares the outer pool and clamps its budget to the
outer one. `chunk` is reserved in the grammar now; its implementation is
deferred to a follow-up.

### Stop-the-world safepoint collector

An allocation on a pool thread or the executor thread that crosses the GC
threshold while workers are active raises a global "collection requested" flag
instead of collecting inline. Every pool thread polls a safepoint at `parallel`
loop back-edges and parks when the flag is set; the executor thread parks at its
next scheduling turn. When every thread is parked, one thread walks all roots —
the executor's current branch chain, every suspended branch chain, each parked
worker's shadow-stack chain, and `mark_clone_roots` — marks, sweeps the shared
intrusive allocation list under a lock, clears the flag, and releases every
thread. The collector stays non-moving mark-sweep and the 3-word object header
(`ADR-012`) is unchanged; only triggering and root enumeration become
thread-aware.

Rejected alternative: per-thread heaps with no stop-the-world. `ADR-003` already
chose one shared non-moving heap and a shadow stack; keeping a single
root-enumeration strategy is worth the safepoint machinery.

### Transfer/Share at the `parallel` boundary

Reuses the `concurrent-blocks-and-timers` analysis. Values and projections copy
in; a strict immutable complete reference may be shared; a mutable alias the
enclosing scope keeps using is rejected with the transfer / strict-share / clone
/ channel resolutions. A `parallel` region SHALL NOT perform I/O, a blocking
channel operation, `Timer.*`, `spawn`, or `concurrent { }` — the checker rejects
them (`PARALLEL_REGION_IO`), with a runtime assert as a backstop.

## Consequences

- First real multi-threading in the runtime. `collector.rs` is the riskiest
  file in the whole five-change concurrency set.
- Mitigation: land `pool.rs` + the safepoint behind a pool size forced to 1
  (behaviourally identical to today), verify the whole suite, then raise the
  pool default to hardware concurrency and re-run the per-triple stress tests
  (allocation-heavy `parallel` reduce under a low GC threshold on macOS
  aarch64, Linux x86_64, Linux aarch64, Windows x86_64).
- `ADR-003` and `ADR-017` gain addenda discharging their deferred concurrency
  notes for the `parallel` worker pool.
- `Mutex` / `RwLock` / `Semaphore` / `Barrier` / `Once` / `Atomic` and
  `Thread.run` remain with `concurrency-completion` (#5); `Channel<T>` with
  `typed-channels` (#4).

## Related

- [ADR-003](./ADR-003-memoria.md) — the shadow stack and cooperative collection
  this ADR makes thread-aware.
- [ADR-017](./ADR-017-modelo-de-suspension.md) — the single-threaded cooperative
  executor this ADR leaves single-threaded while adding a separate worker pool.
- [ADR-012](./ADR-012-layout-de-objetos.md) — the object header this ADR leaves
  unchanged.
- [ADR-018](./ADR-018-concurrency-surface.md) — the I/O concurrency surface this
  ADR complements with CPU parallelism.
- `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` — the normative `parallel`
  behavior.
- `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` — the stop-the-world safepoint.
