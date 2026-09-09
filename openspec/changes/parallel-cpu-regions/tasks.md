## 1. ADRs + spec deltas

- [ ] 1.1 `docs/decisions/ADR-019-parallel-and-multithreaded-gc.md`: two schedulers (cooperative executor stays single-threaded; a separate worker pool); stop-the-world safepoint collector; `parallel` block parallelizes `for` + pipelines lexically; `cores` option; no I/O in a region
- [ ] 1.2 `docs/decisions/ADR-003-memoria.md` addendum + `ADR-017` addendum: the deferred concurrency note is discharged for the `parallel` worker pool
- [ ] 1.3 `specs/parallel-regions/spec.md` — the block, `cores`, no-I/O rule, boundary Transfer/Share, `.parallel` + `Parallel.each` (this change; drafted)
- [ ] 1.4 `specs/parallel-runtime/spec.md` — ADDED: worker pool lifecycle and sizing; `parallel` region submit + join; the stop-the-world safepoint protocol (request flag, back-edge polls, park-all, multi-root walk, release)
- [ ] 1.5 `specs/zirk-structured-concurrency/spec.md`: MODIFIED — "Parallel CPU operations" + "Parallel reductions" pinned to the `parallel` block and `.parallel`; "Safe-code data-race freedom" gains a `parallel`-boundary scenario
- [ ] 1.6 `specs/zirk-grammar/spec.md`: ADDED — `parallel` block + `;`-separated option header; MODIFIED — keyword set gains `parallel`
- [ ] 1.7 `specs/zirk-type-system/spec.md`: ADDED — `parallel` block as expression; `ParallelSeq<T>` API subset; `Parallel.each` typing; associativity check for parallel reduce
- [ ] 1.8 `specs/zirk-ir-lowering/spec.md`: ADDED — `ParallelRegion { core_budget, body }`; parallel `for` lowers to a work-splitting loop; pipeline ops lower to parallel combinators; safepoint poll at loop back-edges in a region
- [ ] 1.9 `specs/zirk-native-codegen/spec.md`: ADDED — worker-pool submit/join emission; safepoint poll emission
- [ ] 1.10 `specs/zirk-object-memory/spec.md` + `specs/zirk-memory-safety/spec.md`: MODIFIED — collector triggering and root enumeration are thread-aware; header unchanged; stop-the-world safepoint is the concurrency-safe collection point
- [ ] 1.11 `specs/async-runtime-core/spec.md`: MODIFIED — add the worker pool and the safepoint protocol as runtime requirements
- [ ] 1.12 `specs/zirk-feature-phasing/spec.md`: MODIFIED — Phase 5 marks `parallel` delivered; `Mutex` / `Atomic` stay with `concurrency-completion`
- [ ] 1.13 `openspec validate parallel-cpu-regions --strict`

## 2. Worker pool (`crates/zirk-runtime/src/pool.rs`)

- [ ] 2.1 New `pool.rs`: N OS worker threads (N = a runtime-config default; forced to 1 initially), a work-stealing deque per worker, `submit(region_tasks)`, `join()`
- [ ] 2.2 A `parallel` region: split work, `submit`, cooperatively block the calling branch (the executor keeps running other I/O branches), `join`, collect
- [ ] 2.3 `cores` resolution: `N` / `-N` / `A..=B` / default; runtime error for `-N` with `N ≥ detected`; nested region clamps to the outer budget
- [ ] 2.4 Pool unit tests: parallel map/reduce correctness, order preservation, `cores` resolution, nested region

## 3. Collector: stop-the-world safepoint (`crates/zirk-runtime/src/collector.rs`)

- [ ] 3.1 Global "collection requested" flag; `zirk_rt_alloc` sets it at the GC threshold instead of collecting inline when the pool has active workers
- [ ] 3.2 Safepoint: worker threads poll at `parallel` loop back-edges and park; the executor parks at its next scheduling turn
- [ ] 3.3 Coordinator: when every thread is parked, walk all roots (executor current + all suspended branch chains + each worker's shadow-stack chain + `mark_clone_roots`), mark, sweep the shared allocation list under a lock, clear the flag, release
- [ ] 3.4 Stress tests on macOS aarch64, Linux x86_64, Linux aarch64, Windows x86_64: allocation-heavy `parallel` reduce under a low GC threshold; no use-after-free, no leak, deterministic result
- [ ] 3.5 Raise the pool default to hardware concurrency; re-run 3.4

## 4. Lexer + AST + parser

- [ ] 4.1 `crates/zirk-lexer`: `parallel` keyword
- [ ] 4.2 `crates/zirk-ast`: `Stmt::Parallel { options: Vec<(Ident, Expr)>, body: Block }` (also usable as `Expr::Parallel`)
- [ ] 4.3 `crates/zirk-parser`: decide the option-header grammar — `parallel; name: expr; name: expr {` contextual parse (prototype first); fallback `parallel(name: expr, ...) {` if it fights the grammar. Record the decision in `design.md`
- [ ] 4.4 Parser tests: bare `parallel { }`, `parallel; cores: 4 { }`, `parallel; cores: -1; chunk: 1000 { }`, as an expression, `cores` as an ordinary identifier elsewhere

## 5. Semantic analysis

- [ ] 5.1 `crates/zirk-sema`: `parallel` block typing (expression = final expr type); `cores` operand must be `Int` or an inclusive range of `Int`
- [ ] 5.2 `ParallelSeq<T>` type from `.parallel`; expose the parallel-safe API subset; `Parallel.each` signature
- [ ] 5.3 Associativity obligation on a parallel `reduce`; `reduce_ordered` escape
- [ ] 5.4 Reject I/O / suspension / `spawn` / `concurrent` / `Timer.*` inside a `parallel` region (`PARALLEL_REGION_IO`)
- [ ] 5.5 Transfer/Share enforcement at the region boundary (reuse #2's analysis)
- [ ] 5.6 Checker tests: region typing, `cores` operand, `.parallel` chain, associativity rejection, I/O rejection, boundary alias rejection

## 6. IR + codegen

- [ ] 6.1 `crates/zirk-ir`: `InstKind::ParallelRegion { core_budget: Operand, body: BlockId }`; parallel `for` -> work-split loop; parallel pipeline -> combinator lowering; `verify.rs`
- [ ] 6.2 `crates/zirk-codegen-llvm`: emit `zirk_rt_pool_submit` / `zirk_rt_pool_join`; emit `zirk_rt_safepoint_poll` at `parallel` loop back-edges
- [ ] 6.3 IR + codegen golden tests

## 7. Fixtures + example

- [ ] 7.1 `valid/parallel_for.zrk`, `valid/parallel_pipeline_sum.zrk`, `valid/parallel_cores_option.zrk`, `valid/parallel_adapter.zrk`
- [ ] 7.2 `invalid/parallel_io.zrk`, `invalid/parallel_nonassociative_reduce.zrk`, `invalid/parallel_cores_zero.zrk`, `invalid/parallel_boundary_alias.zrk`
- [ ] 7.3 `examples/parallel_examples.zrk`: compress-all, dataset reduce with `cores: -1`, `.parallel` chain; compile-and-run CLI test

## 8. Documentation

- [ ] 8.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: the `parallel` sections
- [ ] 8.2 `docs/MEMORY_AND_UNSAFE_SEMANTICS.md`: the stop-the-world safepoint
- [ ] 8.3 Handbook: a parallel chapter; `.parallel` / `Parallel.each` reference
- [ ] 8.4 `docs/init/ZIRK_ROADMAP.md` + `ZIRK_FEATURE_STATUS.md`: `parallel` delivered; the multi-threaded-GC milestone
- [ ] 8.5 `README.md` concurrency line

## 9. Website + closeout

- [ ] 9.1 `cargo test --workspace` (incl. per-triple stress) green; fmt; clippy
- [ ] 9.2 Commit; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review the status catalog + memory-model page; commit `../zirk-lang-site` separately; record both revisions
- [ ] 9.3 `openspec validate parallel-cpu-regions --strict`
