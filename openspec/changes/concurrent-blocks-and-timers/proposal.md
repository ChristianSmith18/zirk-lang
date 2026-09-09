## Why

`remove-task-await-model` strips the old `task` / `await` surface but keeps the
runtime — the single-threaded cooperative executor, stackful-coroutine
suspension, the timer service, and per-task garbage-collection roots. This change
lands the **core of the new surface**: the everyday way a Zirk program does
concurrent I/O.

The model (`docs/concurrency-model-draft.zrk`): a `concurrent { }` block whose
branches run together and which does not close until every branch has finished;
a `spawn` keyword for dynamic branches; and a `Timer` type for delays and
periodic work. No `Task<T>`, no `await`, no `async`, no function coloring — any
function can suspend without changing its signature, because suspension is a
stackful context switch, not a compiler transform.

This is change #2 of five. `parallel` (#3), `Channel<T>` (#4), and the method
API + atomics/synchronizers (#5) build on it.

## What Changes

- **New keyword `concurrent`**: `concurrent { ... }` is a statement that opens a
  structured scope. `inmut` / `mut` bindings declared directly inside it hoist to
  the enclosing scope and are filled when the block closes. The block closes only
  when every branch has finished. A branch that throws cancels its siblings,
  awaits their cleanup, and propagates.
- **Dataflow branch ordering (Rule B)**: each top-level binding in a `concurrent`
  block is a branch; branches with no dependency on a sibling binding run
  concurrently; a branch that reads a sibling binding waits for exactly that
  sibling. The compiler builds the dependency DAG over the block's bindings
  (closures cannot mutate captures, so the DAG is complete).
- **New keyword `spawn`**: valid only inside a `concurrent` block (or the
  implicit root scope of `main`). `spawn expr` adds a branch the block will wait
  for; `inmut h = spawn expr` yields a `Job<T>` handle.
- **New type `Job<T>`**: `job.wait() -> T` (a second `wait()` is a compile-time
  use-after-consume error), `job.cancel()`, `job.done -> Boolean`. `Job<T>` is
  must-use: dropping one without `wait` or `cancel` is a diagnostic.
- **New type `Timer`** with static members: `Timer.sleep(Duration): Void`
  suspends the current branch and is a cancellation safe point;
  `Timer.after(Duration, (): T): Job<T>` runs a thunk once after the delay;
  `Timer.every(Duration, (): Void): Job<Void>` runs a thunk repeatedly at a fixed
  delay until cancelled. `after` / `every` attach to the nearest lexical
  `concurrent` block (or the root scope) and are **ambient** — cancelled when
  that block closes unless `wait`ed. A negative duration is a controlled error.
- **`main` body is an implicit `concurrent` scope**: top-level `spawn` /
  `Timer.after` / `Timer.every` are owned by it and cleaned up before the process
  exits.
- **Cooperative cancellation** is implemented for `concurrent` scopes:
  `CancelledError` is thrown at safe points (`Timer.sleep`, blocking channel ops
  in #4, explicit checks), propagated parent-to-child, idempotent.
- **Runtime C-ABI rename**: `zirk_rt_task_spawn` -> `zirk_rt_spawn`,
  `zirk_rt_task_await` -> `zirk_rt_job_wait`, plus new entry points for
  scope-enter / scope-exit / branch-register / cancel / timer-arm.
- **New ADR** `ADR-018-concurrency-surface`: `concurrent` / `spawn` as the
  structural surface; no coloring on a stackful runtime; binding hoisting;
  Rule B; ambient timers.

## Capabilities

### New Capabilities

- `concurrent-scopes`: the `concurrent { }` block, `spawn`, `Job<T>`, dataflow
  branch ordering, binding hoisting, structured failure propagation and
  cooperative cancellation of a scope, the implicit `main` scope.
- `timer-operations`: `Timer.sleep` / `Timer.after` / `Timer.every`, ambient
  ownership by the nearest lexical `concurrent` scope, safe-point semantics of
  `Timer.sleep`.

### Modified Capabilities

- `zirk-structured-concurrency`: the model-neutral survivor requirements from
  change #1 gain scenarios pinning them to `concurrent { }` / `spawn` / `Job<T>`.
- `zirk-grammar`: add the `concurrent` block and `spawn` productions; `concurrent`
  / `spawn` become keywords; remove them from any removed-construct list.
- `zirk-type-system`: `Job<T>` is a known one-parameter generic; `spawn expr` is
  `Job<T>`; `job.wait()` is `T` and single-consume; `Timer` static-member typing;
  a `concurrent` block's bindings hoist with their branch types.
- `zirk-ir-lowering`: new instructions `ScopeEnter` / `ScopeExit` (reusing the
  cleanup-edge mechanism), `BranchStart`, `JobWait`; `concurrent` block lowering
  builds the branch DAG.
- `zirk-native-codegen`: emit the renamed C-ABI calls; a per-branch thunk
  (reusing the Phase-4d capture machinery); `Job<T>` -> one-word LLVM `i64`.
- `async-runtime-core`: the aggregation-runtime requirement is replaced by a
  scope-join requirement; the timer-service requirement adds `Timer.sleep` /
  `after` / `every`; the executor requirement's safe-point list drops `await` /
  `select`.
- `zirk-errors`: `CancelledError` is thrown at the new safe points; add a "a
  concurrent branch's failure is distinct from `Result.Error`" requirement.
- `zirk-feature-phasing`: Phase 5 marks `concurrent` / `spawn` / `Timer`
  delivered.
- `zirk-lexical-syntax`: `concurrent` / `spawn` are keywords.

## Impact

- **Code**: `crates/zirk-lexer` (2 keywords), `crates/zirk-ast` (`Expr` /
  `Stmt` nodes for the block, `spawn`, `Job` type ref), `crates/zirk-parser`
  (block + header, `spawn`, dataflow-visible binding list), `crates/zirk-sema`
  (`Base::Job`, `Timer` members, hoisting, branch DAG, single-consume on
  `Job::wait`, capture analysis reuse, cancellation-visible flow),
  `crates/zirk-ir` (`ir.rs` new instructions, `lower.rs` DAG lowering,
  `verify.rs`), `crates/zirk-codegen-llvm` (`emit.rs`, `runtime.rs` symbol
  rename + new symbols), `crates/zirk-runtime` (`executor.rs` scope join +
  cancellation sweep + branch registry; `task.rs` -> control block gains
  `scope` / `parent`; `context.rs` unchanged; `timer.rs` gains the
  `after` / `every` re-arm loop; `collector.rs` root walk unchanged), plus CLI
  fixtures and unit tests.
- **Runtime C-ABI**: renamed + extended (provisional, internal).
- **Normative docs**: `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` (the
  `concurrent` / `spawn` / `Timer` sections), `docs/init/ZIRK_ROADMAP.md`,
  `docs/init/ZIRK_FEATURE_STATUS.md`, new `ADR-018`, handbook concurrency
  chapter (rewrite around `concurrent { }`), a new `Timer` reference page, a new
  `examples/concurrent_examples.zrk`.
- **Companion repository `../zirk-lang-site`**: concurrency chapter, examples,
  Phase 5 status. `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`
  after the commits; review the status catalog.
