## Why

The `async-runtime-core` capability (spec of record since `fase-5-async-core` was
archived) describes a cooperative single-threaded executor, task control blocks,
stackful-coroutine suspension, a timer service, per-task garbage-collection
roots, and an executor lifecycle around `main`. None of it is built: `zirk-runtime`
has no `executor.rs`, no `task.rs`, no `timer.rs`, and `collector.rs` still keeps
one process-global shadow-stack chain.

`ADR-017` (the suspension model) and `crates/zirk-runtime/src/context.rs` (the
`TaskContext` / `Suspender` seam over `corosensei`, with a thread-backed fallback)
are already delivered on this branch. This change builds the executor on top of
that seam.

It is deliberately scoped to **runtime infrastructure with no language surface**.
`task` / `await` / `task scope` / `select` parsing, checking, lowering, and
codegen are the next change (`fase-5-structured-tasks`); channels, aggregation,
and the `Transfer` / `Share` analysis come after that. Splitting Phase 5 this way
matches the roadmap's own instruction to build it "in sub-steps, not in one go",
and keeps each change independently reviewable and testable.

## What Changes

- New `crates/zirk-runtime/src/task.rs`: the task control block (`state`, owned
  `TaskContext`, result slot + `result_consumed`, `waiter`, wait reason,
  `cancel_requested` / `cancel_reason`, `shield_depth`, per-task shadow-stack
  head, `cleanup_state`) and a task registry (generational slab so a stale
  `TaskId` is detectably dead).
- New `crates/zirk-runtime/src/executor.rs`: one single-threaded loop; a
  first-in-first-out ready queue; a `CURRENT_TASK` runtime global set on every
  resume; `spawn`, internal `suspend_current(reason)`, wake-on-completion,
  `await`-by-id, and the unresolvable-wait detector (empty ready queue + no armed
  timer + a still-suspended task ⇒ abort with a diagnostic).
- New `crates/zirk-runtime/src/timer.rs`: a monotonic min-heap of deadlines
  (`arm(deadline) -> TimerId`, `disarm`, `poll_expired(now)`) built on the same
  clock the `Duration` / temporal `now_*` helpers already use; a negative
  duration is a controlled error before any waiting begins. Consulted once per
  scheduling turn; an idle executor sleeps until the nearest deadline.
- `crates/zirk-runtime/src/context.rs`: add `suspend_current()` — a
  module-level primitive that suspends the running task through a thread-local
  yielder pointer (re-armed on every resume), so runtime functions that have no
  `&Suspender` in hand (a future `zirk_rt_task_await`, channel operations) can
  suspend. `TaskContext` gains a `usize` result channel so a body's return value
  reaches the control block.
- `crates/zirk-runtime/src/collector.rs`: move the shadow-stack head from the
  process-global into the running task's control block; `zirk_rt_push_frame` /
  `zirk_rt_pop_frame` operate on `CURRENT_TASK`; `collect()` root enumeration
  walks every live task's chain (ready, running, suspended, cleaning), not only
  the running task's. Object header unchanged (`ADR-012`). The `main` task is
  task 0 and its chain behaves exactly as today's global chain.
- `crates/zirk-runtime/src/lib.rs`: the entry wrapper runs `main`'s body as the
  executor's root task and returns the exit status only after that task and every
  descendant has completed or been cleaned; an exception escaping the root task
  still causes a nonzero exit.
- Reclamation: a task control block and its stack are freed only after
  `cleanup_state` says structured cleanup finished and no root chain still
  references it.
- Tests: runtime unit tests for cooperative scheduling (two tasks interleave at a
  suspension point), first-in-first-out fairness, `await` producing the awaited
  value exactly once, the idle-executor timer wake, the unresolvable-wait abort,
  and — the highest-severity case — a reference held only by a suspended task (in
  a named local and in a compiler-spilled temporary) surviving a forced
  collection, plus a finished+consumed task ceasing to root its result.

## Capabilities

### New Capabilities

_None._ This change implements requirements that already exist in the
`async-runtime-core`, `zirk-memory-safety`, and `zirk-native-codegen` main specs
(synced from the archived `fase-5-async-core`).

### Modified Capabilities

- `zirk-feature-phasing`: clarify that Phase 5 is delivered in sub-steps — the
  executor / task / timer / GC-root **runtime infrastructure** lands before the
  `task` / `await` / `select` **language surface**, and a program using that
  syntax still receives the phase diagnostic until `fase-5-structured-tasks`.

## Impact

- **Affected crates**: `zirk-runtime` only (`task.rs`, `executor.rs`, `timer.rs`
  new; `context.rs`, `collector.rs`, `lib.rs` modified). No compiler crate
  changes — there is no language surface in this change.
- **Runtime symbols**: new internal ones (`zirk_rt_task_*`, timer helpers). The
  `extern "C"` surface generated code will call (`zirk_rt_task_spawn`,
  `zirk_rt_task_await`, …) is defined here but exercised only by Rust tests until
  `fase-5-structured-tasks` wires codegen — its exact ABI may still be adjusted
  there.
- **Garbage collector**: `collector.rs` gains a per-task root chain and a
  multi-chain walk. This must be verified against the existing
  allocation-pressure and cycle tests plus the new suspended-task-root tests. The
  collector stays non-moving, single-threaded, and cooperatively triggered
  (`ADR-003`, `ADR-017`).
- **Affected public documentation**: `docs/init/ZIRK_FEATURE_STATUS.md` gains a
  Phase 5 row for the executor infrastructure (delivered) distinct from
  `task`/`await` (still pending); `docs/ZIRK_RUNTIME_SPEC.md` §3 already carries
  the executor note from `fase-5-async-core` group 2 and gets the timer and
  lifecycle detail filled in.
- **Companion repository `../zirk-lang-site`**: impacted only through
  `ZIRK_FEATURE_STATUS.md`. After the crates land and are committed, run
  `./scripts/sync-website-content.sh` and pass `--audit-date YYYY-MM-DD` for the
  changed Phase 5 status.
