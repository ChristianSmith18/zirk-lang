## Why

`concurrent-blocks-and-timers` delivers I/O concurrency on the single-threaded
cooperative executor: overlapping work that mostly *waits*. It does nothing for
work that mostly *computes*. A Zirk program that hashes 10,000 files or reduces a
2-million-row dataset still runs on one core.

This change adds the second half: a `parallel` block that spreads CPU-bound work
across cores, with lexical configuration of the core budget. This is where the
runtime gains a real worker pool and the garbage collector gains a
stop-the-world safepoint — the multi-threading work `ADR-003` and `ADR-017`
deliberately deferred.

Change #3 of five. Depends on #1 (removal) and #2 (`concurrent` scopes, the
capture and transfer/share analysis it establishes).

## What Changes

- **New keyword `parallel`**: `parallel { ... }` and `parallel; <options> { ... }`
  open a CPU-parallel region. Inside the region, `for` loops distribute their
  iterations across worker threads and collection pipelines
  (`.map` / `.filter` / `.reduce` / `.sum` …) run in parallel. The block can be
  an expression and yield a value. Options after `parallel` are separated by `;`
  and terminate at the `{`.
- **`cores` option**: `cores: N` (exactly N, N > 0), `cores: -N` (all available
  minus N), `cores: A..=B` (a range the runtime picks within), absent (all
  available). `cores: 0` and `cores: -N` with N ≥ available are errors.
- **`chunk` option** (advanced, may land later): `chunk: N` sets the work-unit
  granularity for pipeline parallelism.
- **`.parallel` adapter**: `collection.parallel.map(f).filter(g).sum()` runs one
  chain in parallel outside a `parallel` block; the runtime picks the core count.
- **`Parallel.each(collection, fn)`**: a parallel for-each returning `List<R>` in
  input order.
- **Worker pool runtime**: a fixed pool of OS worker threads (default: hardware
  concurrency). `parallel` regions submit tasks to it; the calling branch blocks
  until the region completes.
- **Multi-threaded garbage collection**: the collector gains a stop-the-world
  safepoint. A collection triggered while worker threads are active parks every
  worker at a safepoint, walks all roots (the calling branch's shadow-stack
  chain plus each worker's stack), collects, and releases. Non-moving mark-sweep
  is unchanged; only triggering and root enumeration become thread-aware.
- **Transfer/Share enforcement at the `parallel` boundary**: values and strict
  immutable references may cross into a `parallel` region; a mutable alias the
  parent keeps using is rejected with the standard resolutions.
- **New keyword `parallel`** is contextual only where a block can start; `cores`,
  `chunk` are contextual option names.
- **New ADR** `ADR-019-parallel-and-multithreaded-gc`.

## Capabilities

### New Capabilities

- `parallel-regions`: the `parallel` block, its `cores` / `chunk` options,
  auto-parallel `for` and pipelines inside it, the `.parallel` adapter,
  `Parallel.each`, and the Transfer/Share rules at the region boundary.
- `parallel-runtime`: the worker pool, `parallel` region submission and join, and
  the stop-the-world safepoint collector.

### Modified Capabilities

- `zirk-structured-concurrency`: "Parallel CPU operations" and "Parallel
  reductions" gain scenarios pinning them to the `parallel` block and the
  `.parallel` adapter; "Safe-code data-race freedom" gains a `parallel`-boundary
  scenario.
- `zirk-grammar`: add the `parallel` block + `;`-separated option header.
- `zirk-type-system`: `parallel` block as an expression; `.parallel` adapter type
  (`ParallelSeq<T>` with the parallel-safe subset of the sequence API);
  `Parallel.each` typing; reject a non-associative combiner in a parallel reduce.
- `zirk-ir-lowering`: `ParallelRegion` instruction wrapping a body with a core
  budget; parallel `for` lowers to a work-splitting loop; pipeline ops lower to
  parallel combinators.
- `zirk-native-codegen`: emit worker-pool submit/join calls; safepoint polls at
  loop back-edges inside a `parallel` region.
- `zirk-object-memory` / `zirk-memory-safety`: the collector's triggering and
  root enumeration become thread-aware; the object header is unchanged.
- `async-runtime-core`: add the worker pool and the safepoint protocol.
- `zirk-feature-phasing`: Phase 5 marks `parallel` delivered; `Mutex` / `Atomic`
  remain with `concurrency-completion`.

## Impact

- **Code**: `crates/zirk-lexer` (`parallel` keyword), `crates/zirk-ast`
  (`Stmt::Parallel { options, body }`), `crates/zirk-parser` (the `;` option
  header — contextual parse), `crates/zirk-sema` (`ParallelSeq`, `Parallel.each`,
  associativity check, Transfer/Share at the boundary), `crates/zirk-ir`
  (`ParallelRegion`, parallel `for`, parallel pipeline lowering),
  `crates/zirk-codegen-llvm` (pool submit/join, safepoint polls),
  `crates/zirk-runtime` (new `pool.rs`; `collector.rs` — stop-the-world
  safepoint, multi-root walk; `executor.rs` — a branch entering a `parallel`
  region blocks on the pool). Fixtures + unit tests.
- **Runtime**: first real multi-threading. `collector.rs` is the riskiest file.
- **Normative docs**: `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` (parallel
  sections), `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` (stop-the-world), new
  `ADR-019`, `ADR-003` addendum (the deferred concurrency note is discharged),
  `ADR-017` addendum (the "second thread arrives" note), handbook parallel
  chapter, `examples/parallel_examples.zrk`.
- **Companion `../zirk-lang-site`**: parallel chapter, examples, Phase 5 status,
  memory-model page. Sync with a reviewed `--audit-date`.
