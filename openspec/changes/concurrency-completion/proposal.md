## Why

Changes #2–#4 deliver the everyday concurrency surface: `concurrent { }` /
`spawn` / `Job<T>` / `Timer`, `parallel` regions, and `Channel<T>`. Four things
are still missing before Phase 5 is complete:

1. **Policy combinators** — race, settle-all, and timeout over a set of
   operations. In the old model these were `Task.first` / `Task.settled` and
   `await ... timeout`; here they are pipe-style methods.
2. **Deliberate background work** — a `detach` that outlives the local function
   (analytics, logs, cache warming) without becoming an unsupervised orphan.
3. **Raw OS threads** — `Thread.run` for blocking C libraries and native
   affinity, an escape hatch distinct from `parallel`.
4. **Shared-memory synchronization** — `Atomic<T>`, `Mutex<T>`, and the
   `RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>` library, plus the full
   `Transfer` / `Share` data-race analysis now that real threads exist.

This change closes Phase 5. Depends on #1–#4 (especially #3's worker pool and
multi-threaded collector).

## What Changes

- **`Concurrent.of(t1, t2, ...)`**: builds a pipeline over thunks.
  `.first()` runs all, returns the first to finish, cancels the rest.
  `.settled()` runs all, returns `List<Outcome<T>>` with `Fulfilled` /
  `Rejected` / `Cancelled`. `.within(d)` caps the pipeline at a `Duration`,
  raising `TimeoutError` and cancelling on expiry.
- **`Concurrent.each(coll, fn) -> List<R>`** (wait-all over a collection, in
  order) and **`Concurrent.each_settled(coll, fn) -> List<Outcome<R>>`**.
- **`Outcome<T>`**: a known enum `Fulfilled(T)` / `Rejected(Throwable)` /
  `Cancelled(CancelledError)`. A branch returning `Result.Error` settles as
  `Fulfilled(Error(...))`.
- **App background scope**: `main`'s implicit `concurrent` scope owns a dedicated
  background sub-scope. `Concurrent.detach(fn)` spawns into it; the work is
  cancelled at shutdown after a bounded drain; unhandled errors go to a settable
  `App.on_background_error` handler (default: log to stderr).
- **`Thread.run(fn) -> T`**: runs `fn` on a fresh OS thread, joined at the call
  site (the calling branch blocks cooperatively). For blocking APIs and native
  affinity, not ordinary I/O. `fn` may not `spawn`, open a `concurrent` block, or
  touch a channel from the OS thread.
- **`Atomic<T>`**: `load` / `store` / `exchange` / `compare_exchange` and
  documented numeric updates for supported boolean, integer, and low-level
  pointer forms. Sequentially consistent by default; weaker ordering requires
  `unsafe`.
- **`Mutex<T>`**: `mutex.with(fn)` scoped access; a guard cannot escape; a mutex
  guard may not be held across a safe point. `RwLock<T>`, `Semaphore`,
  `Barrier`, `Once<T>` as library types, not syntax.
- **Non-cancellable region**: `Concurrent.protect(fn)` runs `fn` with the current
  branch's cancellation delivery deferred until it returns — the structured
  replacement for `cancellation shield`, needed for cleanup that performs
  channel ops or `Timer.sleep`.
- **Full data-race analysis**: `Transfer` / `Share` are enforced at every branch
  boundary, channel op, `parallel` boundary, and `Thread.run` boundary; a
  safe-code data race is a compile error.
- **Long-lived services**: `application.spawn_service(fn)` transfers work to the
  app root supervisor, which owns startup failure, ordered shutdown, and final
  diagnostics.
- **New ADR** `ADR-020-synchronization-and-data-race-analysis`.

## Capabilities

### New Capabilities

- `concurrency-combinators`: `Concurrent.of(...).first()` / `.settled()` /
  `.within(d)`, `Concurrent.each` / `Concurrent.each_settled`, `Outcome<T>`.
- `background-and-services`: `Concurrent.detach` + the app background scope,
  `App.on_background_error`, `application.spawn_service`.
- `raw-threads`: `Thread.run` and its boundary rules.
- `synchronization`: `Atomic<T>`, `Mutex<T>`, `RwLock<T>`, `Semaphore`,
  `Barrier`, `Once<T>`, `Concurrent.protect`.

### Modified Capabilities

- `zirk-structured-concurrency`: "Supervised long-lived services", "Scoped
  threads and blocking adapter", "Structured synchronization", "Safe atomics",
  "Safe-code data-race freedom" — restated for the new surface and marked
  delivered; add a non-cancellable-region requirement.
- `zirk-type-system`: `Atomic<T>` / `Mutex<T>` / `RwLock<T>` / `Semaphore` /
  `Barrier` / `Once<T>` / `Outcome<T>` known types; `Concurrent.*` and
  `Thread.run` signatures; the full derived `Transfer` / `Share` rules; a mutex
  guard across a safe point is rejected.
- `zirk-ir-lowering` / `zirk-native-codegen`: `Thread.run` (OS thread spawn +
  join), atomic intrinsics, mutex lock/unlock with guard scoping,
  `Concurrent.protect` (shield-depth increment/decrement on cleanup edges).
- `async-runtime-core`: the app background scope and its drain; `Thread.run`
  join; the atomic and mutex runtime.
- `zirk-memory-safety`: weaker atomic ordering is an `unsafe` operation with an
  explicit proof obligation.
- `zirk-feature-phasing`: Phase 5 is complete.

## Impact

- **Code**: `crates/zirk-sema` (many known types, `Concurrent.*` / `Thread.run`
  resolution, guard-across-safe-point rejection, full Transfer/Share),
  `crates/zirk-ir` + `crates/zirk-codegen-llvm` (OS thread spawn/join, atomic
  intrinsics, mutex ops, `protect` shield edges), `crates/zirk-runtime`
  (`thread.rs`, `sync.rs`, `atomic.rs`, the app background scope + drain in
  `executor.rs`, `application` supervisor). Fixtures + unit tests.
- **Runtime**: OS-thread interaction with the collector safepoint (a `Thread.run`
  thread must also park); the app background scope drain protocol.
- **Normative docs**: `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` (sections
  10–20), `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` (atomics, `unsafe` ordering),
  new `ADR-020`, handbook synchronization + services chapters,
  `examples/synchronization_examples.zrk`.
- **Companion `../zirk-lang-site`**: the remaining concurrency chapters,
  examples, "Phase 5 complete" status. Sync with a reviewed `--audit-date`.
