## Why

Every asynchronous data type the language promises — `Task<T>`, `TaskSettlement<T>`,
`Channel<T>` and its broadcast/watch/one-shot relatives — sits in Phase 5 of
`docs/init/ZIRK_ROADMAP.md`, the next unstarted phase. Today `task` and `await`
are reserved keywords that only emit a "not implemented yet" diagnostic, and
`Channel<T>` / `TaskSettlement<T>` are pending types. There is no executor, no
task control block, and no suspension mechanism anywhere in `zirk-runtime`
(`crates/zirk-runtime/src/lib.rs` still reads "later on ... scheduler").

The roadmap deliberately splits Phase 5 into six sub-steps. Steps 1–3 are a
coherent unit: they deliver the whole cooperative, single-threaded async model —
structured tasks, cancellation, aggregation, selection, channels, and the
compiler analysis that keeps it data-race free — **without** touching the garbage
collector's single-threaded assumption. Steps 4–6 (`Atomic<T>`, `Mutex<T>`,
scoped `thread`, `parallel`) are where the collector must become multi-threaded;
that is a separate change with its own risk budget.

This change delivers roadmap Phase 5, steps 1–3, plus the prerequisite step 0:
the architectural decision for how `await` suspends and how the collector finds
roots inside a suspended task.

## What Changes

### Step 0 — Suspension model (prerequisite decision)

- New ADR (`docs/decisions/ADR-017-*`): **stackful coroutines on a single-threaded
  cooperative executor**. A `task` owns a heap-allocated stack; `await` performs a
  cooperative context switch back to the executor. Plain function calls stay plain
  — there is no `async fn`, no function coloring, and no dual ABI, matching
  `STRUCTURED_CONCURRENCY_SEMANTICS.md` §1 ("There is no `async fn`").
- New ADR or ADR addendum: **per-task GC roots**. The function-granularity shadow
  stack from `ADR-003` (design D4) is generalized from one process-global chain
  to one chain per task, anchored in the task control block. The collector's root
  enumeration walks every live task's chain, not only the running task's. The
  object header is unchanged.

### Step 1 — Structured tasks

- `task expr`, `task { block }`, and `task scope { block }` parse, type-check,
  lower, and run. `task` starts a child in the current structured scope
  immediately and returns `Task<T>`.
- `await expr` suspends the current task (never an OS thread), produces exactly
  `T`, and consumes the `Task<T>` handle once. A second `await` of the same handle
  is a compile-time use-after-consume error.
- Every function body is a structured scope; `task scope { }` is an explicit
  nested supervisor. A scope cannot exit while a child is still running: normal
  exit awaits children; exceptional exit or cancellation requests cancellation,
  awaits child cleanup, then propagates.
- Sibling-failure propagation: the first unhandled child exception becomes the
  primary failure, cancels active siblings, awaits their cleanup, attaches
  additional failures as `suppressed()`, and propagates. A returned `Result.Error`
  is an ordinary fulfilled value, not task rejection.
- Cooperative cancellation: `operation.cancel()` (idempotent, optional typed
  reason defaulting to `CancellationReason.Cancelled`), propagated parent → child,
  observed at defined safe points (`await`, channel operations, `select`, timers,
  explicit checks). New compiler-known `CancelledError`.
- `cancellation shield { }`: a bounded region that defers cancellation delivery
  and delivers any pending cancellation immediately on exit.
- `await operation timeout duration`: cancels the operation at expiry, awaits its
  cleanup, throws the new compiler-known `TimeoutError`. Duration must be
  non-negative.

### Step 2 — Aggregation, selection, channels

- `Task.all(tasks)` (input order preserved, first failure cancels the rest),
  `Task.first(tasks)` (first completion wins, rest cancelled), `Task.settled(tasks)`
  (all finish, input order preserved).
- Compiler-known generic enum `TaskSettlement<T> { Fulfilled(T), Rejected(Throwable),
  Cancelled(CancelledError) }`. A `Task<Result<T,E>>` returning `Error(e)` settles
  as `Fulfilled(Error(e))`; only an unhandled throwable settles as `Rejected`.
- `select { ... }`: waits for the first ready guard (task completion, channel
  send/receive, `after duration`, `cancelled`), runs exactly one branch fairly,
  leaves losing operations alive, and supports a non-suspending `default` branch.
  Channel closure is a ready outcome.
- `Channel<T>`: explicit bounded construction (`Channel<T>(capacity: n)`),
  zero-capacity rendezvous, and defensively-limited unbounded construction
  (`Channel.unbounded(limit:)`). Suspendible `send`/`receive` cooperate with the
  executor; `try_send`/`try_receive` never suspend and return typed outcomes that
  distinguish value, temporary fullness/absence, and closure; `close()` is
  idempotent, wakes suspended operations, and lets queued values drain before
  closure is observed; a full bounded channel applies backpressure.
- Standard `broadcast`, latest-value `watch`, and one-shot channel families with
  the same transfer and cancellation rules.
- The channel runtime is a new GC-traced heap object: queued values that are
  references are roots and are traced through a custom trace hook.

### Step 3 — Transfer / Share analysis

- The compiler derives two non-user-forgeable properties, `Transfer` (a value may
  cross to another concurrent context) and `Share` (a referent may be accessed
  concurrently), and enforces them at every `task`, `task scope` return, channel
  send/receive, and `select` branch-value boundary.
- Boundary rules per `STRUCTURED_CONCURRENCY_SEMANTICS.md` §12–13: value types and
  projections copy; a complete `inmut::strict` reference may share; an exclusive
  mutable complete reference may transfer (the sender then cannot use it until it
  returns through a structured result or channel); `clone()` sends an independent
  graph; task handles, channel endpoints, pointers, and dependent views cross only
  where their own contracts permit.
- Task and closure captures reuse the same analysis and the existing Phase 4d
  GC-tracked capture-block machinery: value snapshots, independent projected
  reads, shared strict-immutable references, transferred exclusive-mutable
  references, and a compile-time error for ambiguous mutable aliasing.
- Safe-code data-race guarantee: safe Zirk rejects an unsynchronized concurrent
  access when at least one access mutates shared state, with no guarantee about
  task completion order.

### Keyword and type gating

- `task` and `await` move from "deferred to Phase 5" to implemented (lexer,
  parser, checker). `select`, `timeout`, `scope`, and `shield` become contextual
  keywords in their forms.
- `Task<T>`, `Channel<T>`, `TaskSettlement<T>`, `CancellationReason`,
  `CancelledError`, `TimeoutError` become known.
- `parallel`, `thread`, `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`,
  `Once<T>`, `Atomic<T>`, and `task.blocking` **remain** gated/pending with a
  diagnostic naming their arrival step.

### Explicitly out of scope

- Roadmap Phase 5 steps 4–6: `Atomic<T>`, `Mutex<T>` and the synchronizer
  library, scoped `thread`, `task.blocking`, `parallel` operations and reductions,
  the application root supervisor's public `spawn_service` surface.
- Making the garbage collector multi-threaded. The executor is strictly one OS
  thread for this change.
- An asynchronous I/O reactor. I/O stays blocking; there is no `await` on a
  socket or file yet. `zirk-runtime-io`'s "Task-aware I/O" requirement is not
  delivered here beyond the executor lifecycle in `main`.
- Any change to integer, `Float`, `Boolean`, `Char`, `String`, or temporal
  semantics.

## Capabilities

### New Capabilities

- `async-runtime-core`: the cooperative single-threaded executor and its run
  queue, the task control block, the stackful-coroutine suspend/resume and
  context-switch mechanism, per-task shadow-stack root enumeration, the timer
  wheel that backs `after` / `timeout`, the channel runtime (bounded, unbounded,
  rendezvous, broadcast, watch, one-shot) with its custom GC trace, the
  aggregation runtime for `Task.all` / `first` / `settled`, and the executor
  lifecycle around `main`.

### Modified Capabilities

- `zirk-structured-concurrency`: add a requirement pinning the single-threaded
  cooperative executor as the conformance vehicle for this phase and the
  guarantee that a suspended task's references stay reachable across collection.
- `zirk-feature-phasing`: `task`, `await`, `select`, `cancellation shield`,
  `timeout`, `Task<T>`, `Channel<T>`, and `TaskSettlement<T>` become part of the
  implemented subset delivered by Phase 5 steps 1–3; `parallel`, `thread`,
  `Mutex<T>`, `Atomic<T>`, and the synchronizers stay deferred; the pending-types
  table is updated accordingly.
- `zirk-grammar`: `task` / `await` / `task scope` / `select` / `cancellation
  shield` / `await ... timeout` productions are added and removed from
  "Constructs outside the subset" for those forms; `parallel` and `thread` stay
  outside the subset.
- `zirk-type-system`: `Task<T>`, `Channel<T>`, `TaskSettlement<T>` are known
  types; `await` typing produces exactly `T` with single-consume enforcement;
  derived `Transfer` / `Share` properties and their boundary checks; task-capture
  typing.
- `zirk-ir-lowering`: add lowering of `task`, `await`, `task scope`, `select`,
  `cancellation shield`, and `await ... timeout` to suspension-point IR with a
  cleanup edge on every scope exit, reusing the Phase 4b/4c cleanup-edge
  machinery; channel operations lower to runtime calls that may suspend.
- `zirk-native-codegen`: add codegen for suspension points and the context-switch
  shim for the four supported target triples, per-task shadow-stack frame
  registration, and the executor entry that wraps `main`.
- `zirk-memory-safety`: root enumeration spans every live task's shadow-stack
  chain; `Transfer` / `Share` enforcement at concurrency boundaries; rejection of
  a mutable alias that would remain usable by the parent while a child can mutate
  it.
- `zirk-errors`: `CancelledError` and `TimeoutError` are compiler-known,
  catchable `RuntimeError` subclasses; task rejection is distinct from
  `Result.Error`; a `CancelledError` that escapes a shield is delivered after the
  shield, not swallowed.

## Impact

- **Affected crates**: `zirk-lexer` (contextual keywords), `zirk-ast` (task /
  await / scope / select / shield nodes), `zirk-parser`, `zirk-sema` (typing,
  single-consume linear check, Transfer/Share analysis, capture analysis),
  `zirk-ir` + `zirk-ir` lowering (suspension-point IR, scope cleanup edges),
  `zirk-codegen-llvm` (`emit.rs`, `runtime.rs` — context-switch shim, per-task
  frame registration, executor entry), `zirk-runtime` (new `executor.rs`,
  `task.rs`, `channel.rs`, `timer.rs`; `collector.rs` — per-task shadow-stack
  head, multi-chain root walk; `lib.rs` — executor lifecycle), `zirk-diagnostics`
  / `zirk-cli` (new diagnostics and help text).
- **New runtime module surface**: an OS-specific register-save/restore + stack
  switch shim. Candidate approaches (decided in design): a small hand-written
  assembly file per triple, or the `corosensei` / `context`-style crate. No new
  dependency if hand-rolled.
- **Object model**: unchanged header; the collector gains a per-task root chain
  and a channel trace hook. This must be verified against the existing
  allocation-pressure and cycle tests plus new suspended-task-root tests.
- **Affected public documentation**: yes — `STRUCTURED_CONCURRENCY_SEMANTICS.md`
  (implementation checklist status), `docs/ZIRK_RUNTIME_SPEC.md` (executor,
  scheduler, channels), `docs/ZIRK_LANGUAGE_SPEC.md` / `docs/CORE_LANGUAGE_SEMANTICS.md`
  (task / await surface now implemented), `docs/init/ZIRK_ROADMAP.md` and
  `docs/init/ZIRK_FEATURE_STATUS.md` (Phase 5 rows for `task`/`await`/`Channel<T>`
  move to implemented; `parallel`/`thread`/`Atomic<T>` stay pending), the handbook
  `02-handbook/18-concurrency/` chapter, `11-reference` type pages for `Task<T>`
  / `Channel<T>` / `TaskSettlement<T>`, and new `.zrk` examples.
- **Companion repository `../zirk-lang-site`**: impacted. After the zirk-lang
  changes land and are committed, run `./scripts/sync-website-content.sh`, review
  the site-owned status catalog for the changed Phase 5 status and the reduced
  concurrency limitations, and pass `--audit-date YYYY-MM-DD`. The website must
  not keep listing `task`/`await`/`Channel<T>` as unimplemented.
- **No migration**: `task` / `await` currently do not compile, so no existing
  program depends on their absence beyond the phase diagnostic.
