## 1. ADR + spec deltas

- [ ] 1.1 `docs/decisions/ADR-020-synchronization-and-data-race-analysis.md`: `Mutex.with` scoped access + no guard across a safe point; `Atomic` SC default, weak ordering in `unsafe`; `Concurrent.protect` as the shield replacement; full Transfer/Share enforcement points; the app background scope
- [ ] 1.2 `specs/concurrency-combinators/spec.md` — `Concurrent.of(...).first/settled/within`, `each` / `each_settled`, `Outcome<T>` (this change; drafted)
- [ ] 1.3 `specs/synchronization/spec.md` — `Mutex`, `Atomic`, `Concurrent.protect`, full data-race freedom (this change; drafted)
- [ ] 1.4 `specs/background-and-services/spec.md` — app background scope, `Concurrent.detach`, `application.spawn_service` (this change; drafted)
- [ ] 1.5 `specs/raw-threads/spec.md` — `Thread.run` (this change; drafted)
- [ ] 1.6 `specs/zirk-structured-concurrency/spec.md`: MODIFIED — "Supervised long-lived services", "Scoped threads and blocking adapter", "Structured synchronization", "Safe atomics", "Safe-code data-race freedom" restated for the new surface and marked delivered
- [ ] 1.7 `specs/zirk-type-system/spec.md`: ADDED — `Atomic<T>` / `Mutex<T>` / `RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>` / `Outcome<T>` / `ConcurrentPipeline` known types; `Concurrent.*` and `Thread.run` signatures; full derived `Transfer` / `Share`; mutex guard across a safe point rejected
- [ ] 1.8 `specs/zirk-ir-lowering/spec.md` + `specs/zirk-native-codegen/spec.md`: ADDED — OS thread spawn/join; atomic intrinsics; mutex lock/unlock with guard scoping; `Concurrent.protect` shield-depth on cleanup edges
- [ ] 1.9 `specs/async-runtime-core/spec.md`: MODIFIED — app background scope + drain; `Thread.run` join; atomic + mutex runtime
- [ ] 1.10 `specs/zirk-memory-safety/spec.md`: MODIFIED — weaker atomic ordering is an `unsafe` operation with an explicit proof obligation
- [ ] 1.11 `specs/zirk-feature-phasing/spec.md`: MODIFIED — Phase 5 complete
- [ ] 1.12 `openspec validate concurrency-completion --strict`

## 2. Combinator runtime (`crates/zirk-runtime`)

- [ ] 2.1 `Outcome<T>` enum in the runtime + sema
- [ ] 2.2 `Concurrent.of` pipeline: register N branches on a shared waiter; `.first()` (resume on first completion, cancel rest); `.settled()` (wait all, collect outcomes in order); `.within(d)` (arm a timer, on expiry cancel + `TimeoutError`), chainable
- [ ] 2.3 `Concurrent.each` / `each_settled` over a collection
- [ ] 2.4 Unit tests: first + cancel-rest, settled ordering + mixed outcomes, within expiry + cleanup, each fail-fast + order, `Result.Error` settles `Fulfilled`

## 3. App background scope + services (`crates/zirk-runtime/src/executor.rs`, `application.rs`)

- [ ] 3.1 `main` implicit scope creates `App.background` child sub-scope before the body
- [ ] 3.2 `Concurrent.detach(fn)` = `spawn` into `App.background`, returns nothing
- [ ] 3.3 Shutdown: cancel `App.background`, drain within `App.shutdown_grace` (default 5s, settable), then exit
- [ ] 3.4 `App.on_background_error: (Throwable): Void` settable; default logs to stderr
- [ ] 3.5 `application.spawn_service(fn)`: transfer to the root supervisor; startup-failure, ordered shutdown, final diagnostics
- [ ] 3.6 Unit tests: detach does not block caller, drain at shutdown, error handler invoked, service ordered shutdown

## 4. Raw threads (`crates/zirk-runtime/src/thread.rs`)

- [ ] 4.1 `Thread.run(fn)`: spawn OS thread, register with the collector, calling branch suspends cooperatively, resume on join
- [ ] 4.2 Safepoint polls at loop back-edges in a `Thread.run` body (codegen); the OS thread parks on request
- [ ] 4.3 Sema: reject `spawn` / `concurrent` / channel op / `Timer.sleep` in a `Thread.run` body (`THREAD_RUN_SUSPEND`)
- [ ] 4.4 Tests per triple: executor keeps working during `Thread.run`; collection while the OS thread is active is safe

## 5. Synchronization (`crates/zirk-runtime/src/{sync.rs, atomic.rs}`)

- [ ] 5.1 `atomic.rs`: `Atomic<Boolean>` / `Atomic<IntN>` / `Atomic<Pointer<T>>`; `load` / `store` / `exchange` / `compare_exchange` / `fetch_add` …; SC default; `AtomicOrder` enum
- [ ] 5.2 `sync.rs`: `Mutex<T>` (`with` closure, lock/unlock on every exit edge), `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`
- [ ] 5.3 Sema: guard cannot escape; guard across a safe point rejected (`MUTEX_GUARD_ACROSS_SAFE_POINT`); `AtomicOrder.relaxed`/`.acquire`/`.release` only in `unsafe`
- [ ] 5.4 `Concurrent.protect(fn)`: `shield_depth` increment on entry, decrement + pending-cancel check on every exit edge (IR cleanup edges); best-effort unbounded lint
- [ ] 5.5 Unit tests: mutex mutual exclusion, atomic CAS loop, `Once` runs once, barrier release, `protect` holds then delivers

## 6. Full Transfer/Share analysis (`crates/zirk-sema`)

- [ ] 6.1 Enforce `Transfer` / `Share` at: branch captures, `concurrent` results, channel send/receive, `parallel` boundary, `Thread.run` boundary
- [ ] 6.2 A safe-code data race is a compile error naming transfer / strict-share / clone / channel / synchronizer
- [ ] 6.3 Checker tests: two branches mutate a list -> error; transferred exclusive ref unusable by sender; strict-immutable share allowed; synchronized share allowed

## 7. IR + codegen

- [ ] 7.1 `crates/zirk-ir`: OS thread spawn/join instructions; atomic intrinsics; mutex lock/unlock; `Concurrent.protect` shield edges; `verify.rs`
- [ ] 7.2 `crates/zirk-codegen-llvm`: LLVM atomic ops; `zirk_rt_thread_run` / join; `zirk_rt_mutex_*`; safepoint polls in `Thread.run` bodies
- [ ] 7.3 IR + codegen golden tests

## 8. Fixtures + example

- [ ] 8.1 `valid/concurrent_first.zrk`, `valid/concurrent_settled.zrk`, `valid/concurrent_within.zrk`, `valid/concurrent_each.zrk`
- [ ] 8.2 `valid/detach_analytics.zrk`, `valid/thread_run_blocking.zrk`, `valid/mutex_counter.zrk`, `valid/atomic_flag.zrk`, `valid/protect_cleanup.zrk`
- [ ] 8.3 `invalid/mutex_across_sleep.zrk`, `invalid/thread_run_channel.zrk`, `invalid/relaxed_outside_unsafe.zrk`, `invalid/data_race_shared_list.zrk`
- [ ] 8.4 `examples/synchronization_examples.zrk`: shared counter behind a mutex, an atomic flag, `Concurrent.of(...).first()` race, `Concurrent.detach` analytics, `Thread.run` for a blocking lib; compile-and-run CLI test

## 9. Documentation

- [ ] 9.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: sections 10–20 (combinators, detach, threads, synchronization, atomics, data-race guarantee, protect)
- [ ] 9.2 `docs/MEMORY_AND_UNSAFE_SEMANTICS.md`: atomics and `unsafe` ordering; `Thread.run` + safepoint
- [ ] 9.3 Handbook: synchronization chapter, services chapter, `Atomic` / `Mutex` / `Thread` reference pages
- [ ] 9.4 `docs/init/ZIRK_ROADMAP.md` + `ZIRK_FEATURE_STATUS.md`: **Phase 5 complete**
- [ ] 9.5 `README.md` concurrency line -> delivered

## 10. Website + closeout

- [ ] 10.1 `cargo test --workspace` (incl. per-triple) green; fmt; clippy
- [ ] 10.2 Commit; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review status catalog ("Phase 5 complete" is a reviewed claim); commit `../zirk-lang-site` separately; record both revisions
- [ ] 10.3 `openspec validate concurrency-completion --strict`
