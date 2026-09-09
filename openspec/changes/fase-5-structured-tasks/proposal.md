## Why

`fase-5-task-await` delivered the bare `task expression`, `task { block }`, and
`await expression` forms end to end, but a program still cannot supervise a group
of tasks: there is no failure propagation between siblings, no cooperative
cancellation surfaced to source, and no way to protect cleanup work that must
finish once cancellation has begun. Those behaviors are roadmap Phase 5
steps 1 to 3 and they are the smallest coherent unit that turns `task` from a
single spawn primitive into structured concurrency.

While specifying that unit, two constructs the current normative specs promise —
the `task scope` keyword and the `cancellation shield` statement — turn out to be
avoidable. Every executing function is already a structured scope, so a nested
`task scope` keyword only duplicates that boundary; the ergonomic cases it exists
for are covered by the implicit function scope plus a `Task.combine` /
`Task.all` join. And `cancellation shield` and any notion of an "uncancelable
spawn" collapse into one primitive — `final task` — once awaiting a
`Task.Final<T>` is defined to be a shielded await. Removing both keywords keeps
the language surface smaller without losing any capability.

## What Changes

- **Sibling-failure propagation**: within a structured scope (a function body or
  the block of a `task`), the first unhandled child exception becomes the primary
  failure, requests cancellation of active siblings, awaits their cleanup,
  attaches additional failures as suppressed, and propagates. A returned
  `Result.Error` stays an ordinary fulfilled value.
- **Cooperative cancellation**: `handle.cancel()` on a `Task<T>` is idempotent,
  accepts an optional typed reason defaulting to `CancellationReason.Cancelled`,
  and is observed only at defined safe points (`await`, and — in later slices —
  channel operations, timers, explicit checks). Cancellation propagates parent to
  child and throws the compiler-known `CancelledError` at the safe point. A
  cancelled task still runs ordinary cleanup; it is never stopped at an arbitrary
  instruction.
- **`final task` / `final task { block }`**: a call-site modifier (like `task`
  itself, never a declared return type) that produces a `Task.Final<T>`. A
  `Task.Final<T>` task is excluded from every sibling-cancellation and
  parent-cancellation sweep and runs to completion. `handle.cancel()` on a
  `Task.Final<T>` is a compile-time error.
- **Shielded await rule**: `await` on a `Task.Final<T>` produces exactly `T` and
  additionally defers delivery of the current task's pending cancellation until
  that await completes; the pending `CancelledError` is then delivered at the
  next safe point. This is the mechanism that replaces `cancellation shield`.
- **`Task.combine`**: a heterogeneous, fixed-arity fail-fast join —
  `await Task.combine(a, b, c)` returns `(A, B, C)`; the first unhandled failure
  cancels the remaining members (unless a member is a `Task.Final<T>`), awaits
  cleanup, and propagates.
- **Timer surface on `Task`**: `Task.sleep(Duration): Task<Void>` is a
  cooperative delay of the awaiting task; `Task.after(Duration, (): T): Task<T>`
  runs a callable once after a delay; `Task.every(Duration, (): Void): Task<Void>`
  runs a callable repeatedly until cancelled. All three return `Task<T>` and are
  awaitable through the ordinary mechanism, are backed by the existing timer
  service, and — unlike JS `setTimeout` / `setInterval` — are scope-owned:
  cancellable and never orphaned. `Task.sleep` is a cancellation safe point.
- **`Task.all` / `Task.settled` / `TaskSettlement<T>`**: `Task.all` is the
  homogeneous fail-fast join over a list, preserving input order; `Task.settled`
  lets every member finish, preserves order, and returns
  `List<TaskSettlement<T>>` with `Fulfilled(T)` / `Rejected(Throwable)` /
  `Cancelled(CancelledError)` variants. `TaskSettlement<T>` becomes an ordinary
  known enum.
- **BREAKING (pre-1.0 spec surface)**: the `task scope` keyword is removed. The
  implicit "a scope MUST NOT abandon unfinished children" behavior stays, now
  anchored only on function bodies and `task` blocks. Programs that would have
  written `task scope { ... }` use the surrounding function scope plus
  `Task.combine` / `Task.all`.
- **BREAKING (pre-1.0 spec surface)**: the `cancellation shield` statement is
  removed and replaced by `final task` and the shielded await rule.
- **Deferred, unchanged**: `select`, `await ... timeout`, the `Channel<T>`
  family, `Task.first`, the lower-level fixed-rate ticker resource in the
  temporal family, `parallel`, `thread`, `task.blocking`, `Mutex<T>`,
  `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`, and `Atomic<T>` keep emitting
  their phase diagnostics. The executor stays single-threaded and the garbage
  collector stays single-threaded.
- **New example**: `examples/structured_tasks_examples.zrk`, a runnable
  compile-and-run validation covering sibling failure, cancellation, `final
  task` cleanup that survives cancellation, the shielded await, `Task.combine` /
  `Task.all` / `Task.settled`, and `Task.sleep` / `Task.after` / `Task.every`.

## Capabilities

### New Capabilities

_None._ This change refines requirements that already exist in the
`zirk-structured-concurrency`, `async-runtime-core`, `zirk-grammar`,
`zirk-type-system`, `zirk-ir-lowering`, `zirk-native-codegen`, `zirk-errors`, and
`zirk-feature-phasing` main specs (synced from the archived `fase-5-async-core`).

### Modified Capabilities

- `zirk-structured-concurrency`: replace the `cancellation shield` requirement
  with a `final task` / `Task.Final<T>` / shielded-await requirement; drop the
  `task scope` keyword from the typed-structured-tasks and data-race requirements
  while keeping the implicit-scope guarantee; add `Task.combine` to the
  aggregation requirement; add a timer-driven-tasks requirement for
  `Task.sleep` / `Task.after` / `Task.every`.
- `zirk-feature-phasing`: move sibling-failure propagation, cooperative
  cancellation, `final task`, `Task.all` / `Task.combine` / `Task.settled`, and
  `TaskSettlement<T>` from deferred to delivered; remove `task scope` and
  `cancellation shield` from the deferred list because they no longer exist;
  `select`, `await ... timeout`, `Channel<T>`, and `Task.first` stay deferred.
- `zirk-grammar`: add the `final task expression` and `final task { block }`
  prefix forms; remove the `task scope` production and its targeted diagnostic;
  remove the `cancellation shield` targeted diagnostic.
- `zirk-type-system`: `Task.Final<T>` is a known single-parameter generic type;
  `await` on it is typed exactly `T`; `.cancel()` on a `Task.Final<T>` is a
  type-level error; `TaskSettlement<T>` resolves as an ordinary enum;
  `Task.sleep` / `Task.after` / `Task.every` are typed static members of `Task`
  returning `Task<Void>` / `Task<T>` / `Task<Void>`.
- `zirk-ir-lowering`: `TaskStart` carries an `uncancelable` flag (or a paired
  `FinalTaskStart`); `Await` carries a `shielded` flag; both are covered in
  `verify.rs` and every exhaustive `IrType` match.
- `zirk-native-codegen`: emit the uncancelable-spawn and shielded-await runtime
  ABI calls; `Task.Final<T>` lowers to the same one-word handle as `Task<T>`.
- `async-runtime-core`: the task control block gains an `uncancelable` marker;
  the executor's cancellation sweeps skip uncancelable tasks; a shielded await
  holds pending cancellation on the current task (reusing the existing
  `shield_depth` field) until the awaited task completes; sibling-failure
  cancellation, cleanup ordering, and the aggregation policies are runtime
  behavior; `Task.sleep` / `Task.after` / `Task.every` are implemented through
  the existing timer service.
- `zirk-errors`: `CancelledError` is thrown at safe points; new diagnostic codes
  for "cancel on a final task" and "final task with an unbounded wait".

## Impact

- **Code**: `crates/zirk-lexer` (keyword table for `final` in the `final task`
  position), `crates/zirk-ast`, `crates/zirk-parser`, `crates/zirk-sema`
  (checker, types, capture analysis, cancellation-linearity tracking),
  `crates/zirk-ir` (`ir.rs`, `lower.rs`, `verify.rs`), `crates/zirk-codegen-llvm`
  (`runtime.rs`, `emit.rs`), `crates/zirk-runtime` (`executor.rs`, `task.rs`,
  `context.rs`, `collector.rs`, a new `aggregate.rs`), plus
  `crates/zirk-cli/tests` fixtures and `crates/zirk-sema/tests` /
  `crates/zirk-ir/tests` unit tests.
- **Runtime ABI**: new provisional `extern "C"` entry points for uncancelable
  spawn, shielded await, `cancel`, and the aggregation joins, extending the
  surface `fase-5-executor-core` introduced.
- **Normative docs**: `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` (rewrite the
  scope, cancellation, shield, and aggregation sections), `docs/init/
  ZIRK_ROADMAP.md` (Phase 5 step 1 status and the `task scope` /
  `cancellation shield` removal), `docs/init/ZIRK_FEATURE_STATUS.md` (Phase 5
  rows), and a new `docs/decisions/ADR-018-*` for the uncancelable-task and
  shielded-await model.
- **Handbook**: `docs/handbook` concurrency chapter and the `Task<T>` reference
  page gain the cancellation, `final task`, and aggregation sections.
- **Companion repository `../zirk-lang-site`**: public documentation, the
  concurrency examples, normative-semantics wording, and the Phase 5 status
  evidence change. After the zirk-lang commits land, run
  `./scripts/sync-website-content.sh` with an explicit `--audit-date YYYY-MM-DD`,
  and review the site-owned status catalog. This change is not complete while the
  website still describes `task scope` / `cancellation shield` or an older
  Phase 5 status.
