## Context

Phase 5's surface is delivered by #2–#4 except: policy combinators, deliberate
background work, raw threads, and shared-memory synchronization. The old model's
`Task.first` / `Task.settled` / `await ... timeout` / `cancellation shield` /
`application.spawn_service` / `Atomic` / `Mutex` requirements all live in
`zirk-structured-concurrency` and are restated here for the new surface.

## Goals / Non-Goals

**Goals**: `Concurrent.of(...).first/settled/within`, `Concurrent.each` /
`each_settled`, `Outcome<T>`, `Concurrent.detach` + app background scope,
`Thread.run`, `Atomic<T>`, `Mutex<T>` + the synchronizer library,
`Concurrent.protect`, full data-race analysis, `application.spawn_service`.

**Non-Goals**: broadcast/watch/oneshot channels (a `typed-channels` follow-up),
async `Thread.run` pools beyond one-shot join, lock-free data structures in the
stdlib, changing the `parallel` model.

## Decisions

### D1: `Concurrent.of` is a lazy pipeline over thunks

`Concurrent.of(t1, t2, ...)` where each `ti` is `(): Ti` builds a
`ConcurrentPipeline`. Nothing runs until a terminal: `.first()`, `.settled()`,
`.within(d)`. `.within(d)` is chainable
(`Concurrent.of(...).within(5s).first()`). Members must be a small fixed arity
(2..8) for a heterogeneous `.first()` returning the common type; a homogeneous
collection uses `Concurrent.each*`.

**Alternative**: eager `Concurrent.first(t1, t2)` flat calls. Rejected — the
pipeline composes `.within` cleanly and reads left-to-right.

### D2: `Outcome<T>` unifies settle results

`enum Outcome<T> { Fulfilled(T), Rejected(Throwable), Cancelled(CancelledError) }`.
A branch returning `Result.Error(e)` settles `Fulfilled(Error(e))` — only an
unhandled throwable is `Rejected`. Same rule the old `TaskSettlement` had.

### D3: App background scope

`main`'s implicit `concurrent` scope creates one child sub-scope,
`App.background`, before running the body. `Concurrent.detach(fn)` = a `spawn`
into `App.background` that returns nothing. On `main` completion (normal or
signal): `App.background` is cancelled, then drained with a bounded grace period
(default 5s, `App.shutdown_grace`), then the process exits. An unhandled
exception in a detached branch is delivered to `App.on_background_error` (a
settable `(Throwable): Void`; default logs to stderr with the branch's
diagnostic).

**Alternative**: a truly ownerless `detach`. Rejected — that is the JS
unhandled-rejection hole; the background scope keeps it structured while still
not blocking the local function.

### D4: `Thread.run` blocks the calling branch cooperatively

`Thread.run(fn): T` spawns a real OS thread, and the calling branch suspends
(the executor keeps running other branches) until the thread joins. The OS
thread must park at the collector safepoint (#3) — `fn`'s compiled body gets
safepoint polls at its own loop back-edges, same as pool threads. `fn` may not
`spawn`, open `concurrent { }`, or use a channel (no safe points on an OS
thread); the checker rejects it. Values crossing in/out follow Transfer/Share.

### D5: `Mutex<T>` — scoped, no guard across a safe point

`Mutex<T>` wraps its `T`; the only access is `mutex.with((value) => { ... })`
which locks, runs the closure with a writable view, and unlocks on every exit
edge. The view may not escape the closure (checked). The closure may not contain
a safe point (`Timer.sleep`, a channel op, `spawn`, `Thread.run`) — holding a
mutex across suspension can deadlock the scheduler; the checker rejects it with a
pointer to `RwLock` or a redesign. `RwLock<T>` / `Semaphore` / `Barrier` /
`Once<T>` are ordinary library types.

### D6: `Atomic<T>` — SC default, weak ordering is `unsafe`

`Atomic<Boolean>` / `Atomic<IntN>` / `Atomic<Pointer<T>>` with `load` / `store` /
`exchange` / `compare_exchange` and documented numeric updates
(`fetch_add`, …). Default `AtomicOrder.seq_cst`. `AtomicOrder.relaxed` /
`.acquire` / `.release` are only accepted inside `unsafe` and the programmer
carries the proof obligation (`zirk-memory-safety`).

### D7: `Concurrent.protect(fn)` — the non-cancellable region

`Concurrent.protect((): T => { ... }): T` runs `fn` with the current branch's
`shield_depth` raised for the duration; a pending `CancelledError` is held and
delivered at the first safe point after `protect` returns. This is the
structured replacement for `cancellation shield`, needed when cleanup must
perform a channel op or `Timer.sleep`. The body must be bounded (a best-effort
lint; the unresolvable-wait detector is the backstop).

### D8: Full Transfer/Share enforcement

The `Transfer` / `Share` analysis (introduced structurally in #2, enforced at the
`parallel` boundary in #3) is now enforced everywhere real concurrency can
observe a race: branch captures, `concurrent` results, channel send/receive,
`parallel` boundary, `Thread.run` boundary. A safe-code data race is a compile
error naming transfer, strict-immutable sharing, cloning, a channel, or a
synchronizer.

## Risks / Trade-offs

- **`Thread.run` + collector safepoint** → the OS thread must be registered with
  the collector and park like a pool thread; a `Thread.run` body that loops
  without a back-edge safepoint could stall a collection. → Mitigation: insert a
  safepoint poll at every loop back-edge in a `Thread.run` body, same as `parallel`.
- **App background drain grace period** → too short kills legit in-flight
  analytics; too long hangs shutdown. → Mitigation: 5s default, configurable,
  documented; detached work should be short.
- **`Mutex` guard-across-safe-point rejection** may be surprising → the message
  names `RwLock` and the "don't hold a lock across I/O" rule explicitly.
- **`Concurrent.of` fixed arity** → 8 covers realistic heterogeneous races;
  beyond that, `Concurrent.each*`.
- **Weak atomics behind `unsafe`** → matches `zirk-memory-safety`; the unsafe
  transaction rule does not make relaxed ordering correct on its own.

## Migration Plan

1. `ADR-020`; spec deltas; validate.
2. Runtime: `Outcome`, `Concurrent.of` pipeline + terminals, `each*`;
   `App.background` scope + drain; unit tests.
3. Runtime: `thread.rs` (OS thread + join + safepoint registration), `sync.rs`
   (`Mutex` / `RwLock` / `Semaphore` / `Barrier` / `Once`), `atomic.rs`;
   collector interaction tests per triple.
4. `Concurrent.protect` — `shield_depth` on cleanup edges.
5. Sema: all known types, `Concurrent.*` / `Thread.run` resolution,
   guard-across-safe-point rejection, full Transfer/Share, weak-ordering-in-unsafe.
6. IR/codegen: OS thread spawn/join, atomic intrinsics, mutex ops, `protect`.
7. Fixtures + `examples/synchronization_examples.zrk`.
8. Docs: semantics §10–§20, memory-and-unsafe atomics, handbook chapters,
   roadmap "Phase 5 complete", feature status.
9. `cargo test` (incl. per-triple) + fmt + clippy; commit; website sync with
   reviewed date; commit site separately.

## Open Questions

- `application` — is it an ambient global, or passed to `main`
  (`fn main(app: Application)`)? Proposed: ambient `App` / `application` for now.
- `Thread.run` returning after `main` starts shutting down — join or abandon?
  Proposed: shutdown cancels the calling branch; the OS thread is detached and
  the process waits up to the grace period.
- `Concurrent.protect` nesting and a second forceful cancel — proposed: honor the
  shield absolutely in this change; revisit if a hard-deadline source appears.
