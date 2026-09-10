## Context

The runtime from `fase-5-executor-core` (executor loop, task control block,
stackful `context.rs` seam over `corosensei`, timer min-heap, per-task
shadow-stack roots, `zirk_rt_run_main`) is kept by `remove-task-await-model`.
This change puts the new language surface on it. `ADR-017`'s runtime decisions
stand; this change's `ADR-018` records the surface.

## Goals / Non-Goals

**Goals**: `concurrent { }`, `spawn`, `Job<T>`, `Timer.sleep/after/every`,
dataflow branch ordering, binding hoisting, structured failure + cooperative
cancellation for a scope, the implicit `main` scope.

**Non-Goals**: `parallel` (#3), `Channel<T>` (#4), `Concurrent.of/each/detach`,
`Thread.run`, atomics/synchronizers, real multi-threading, the app background
scope for `detach` (#5). No `select`. No timeouts (`within` is #5).

## Decisions

### D1: `concurrent { }` is a statement; its bindings hoist

`concurrent { inmut a = f(); inmut b = g(); }` declares `a` and `b` in the
enclosing scope. They are unreadable until the block closes; reading one before
`}` (outside a dependent branch) is a compile error. This keeps the common case
(`return Dashboard(a, b)` right after) clean and avoids a tuple-return dance.

**Alternative**: the block is an expression returning a tuple. Rejected — forces
`inmut (a, b) = concurrent { ... }` and names every result twice.

### D2: Rule B — dataflow branch ordering

Over the block's top-level bindings the compiler builds a DAG: an edge `b -> a`
when `b`'s initializer names `a`. Bindings in the same antichain run
concurrently; a dependent binding starts when its predecessors have completed.
Closures cannot mutate captures, and branches cannot mutate enclosing variables
(`zirk-structured-concurrency` "Safe concurrent captures"), so the only branch
dependencies are through the named bindings — the DAG is complete and sound.

A cycle (`a = f(b); b = g(a)`) is a compile error. Non-binding statements
(`stdout.println(...)`) between bindings run on the block's own branch, in
source order, and join like any other branch.

**Alternative A (strict)**: referencing a sibling binding is an error; pull it
out. Rejected — forces restructuring for no benefit on small blocks.
**Alternative C (explicit `spawn` per branch)**: rejected — noise; `spawn` stays
for the *dynamic* case.

### D3: `spawn` — the dynamic branch, keyword not method

`spawn expr` / `spawn { block }` / `inmut h = spawn expr`. Only inside a
`concurrent` scope (or `main`'s implicit one); elsewhere it is a compile error
naming `concurrent { }`. The block waits for every `spawn`ed branch, bound or
not. The body follows the ordinary closure capture rules, reusing
`begin_capture_scope` / `finish_capture_scope`.

An infinite `spawn` body (a server loop, `Timer.every`) makes the block infinite
— correct for a server's `main`, and a script simply does not spawn infinite
work or cancels it.

### D4: `Job<T>` — the only handle, must-use, single-consume

`spawn expr` where `expr: T` -> `Job<T>`. `job.wait(): T` consumes it (linear;
second `wait` / any later use is `SECOND_WAIT`). `job.cancel()` requests
cancellation. `job.done: Boolean`. A `Job<T>` that leaves its `concurrent` scope
un-waited and un-cancelled is a must-use diagnostic dischargeable with `_ = job`
(the scope still joins it). `Job<T>` lowers to the runtime's one-word branch id.

### D5: `Timer` — static members, ambient `after` / `every`

`Timer.sleep(d): Void` — suspends the current branch via a timer wait; a
cancellation safe point. `Timer.after(d, thunk): Job<T>` — one-shot; equivalent
to `spawn { Timer.sleep(d); return thunk() }`. `Timer.every(d, thunk):
Job<Void>` — fixed-delay loop until cancelled; `.wait()` blocks until cancelled.

`after` / `every` bind to the **nearest enclosing lexical `concurrent` block**
(or `main`'s implicit scope). They are **ambient**: on that block's close (normal
or exceptional) they are cancelled, not awaited — so a live `Timer.every` does
not deadlock the block. `spawn`ed branches, by contrast, are awaited. This is the
one asymmetry in the model and it is the right default (a periodic task is
support work; a spawned task is the work).

`Timer.every` may not be wrapped so it cannot be cancelled (there is no `final`
marker in this model; that concern is a `concurrency-completion` non-cancellable
region).

**Alternative**: `after` / `every` are `spawn`ed and awaited like everything
else. Rejected — every server `main` would then need an explicit
`ticker.cancel()` before shutdown, and the block would hang if you forgot.

### D6: No coloring — stackful, one ABI

Reaffirmed from `ADR-017`. A function that calls `Timer.sleep` or a blocking
channel op has the same signature and ABI as one that does not. The checker has
no async/effect system. `concurrent` / `spawn` mark where a branch is *born*,
which is exactly where the scope boundary matters.

### D7: Runtime — scope join + cancellation sweep

`task.rs` control block gains `parent: Option<ScopeId>` and the executor gains a
scope table. `ScopeExit` runs: wait for every registered branch's terminal
state; on the first unhandled branch exception, `request_cancel` the others,
wait for `cleanup_state == Done`, resolve with the primary + suppressed list.
`request_cancel` is idempotent and wakes a branch suspended at a cancellable
`WaitReason`. The safe-point check (`Timer.sleep`, blocking channel op, explicit
`Concurrent.check_cancelled()`) throws `CancelledError` when
`cancel_requested && shield_depth == 0`. `shield_depth` stays 0 in this change
(#5 adds a non-cancellable region).

The collector's per-task root walk is unchanged — it already walks every live
control block's shadow-stack chain, and a branch is a control block.

## Risks / Trade-offs

- **Rule B DAG is a new analysis** → keep it dead simple: only over a block's
  direct `inmut`/`mut` bindings, edges only through name references, cycle = hard
  error. No inter-procedural anything.
- **The `spawn` vs `Timer.*` await/cancel asymmetry** is a thing to learn →
  document it prominently; the alternative (explicit `.cancel()` everywhere) is
  worse.
- **C-ABI rename churn** across `runtime.rs` + `emit.rs` → mechanical, one
  commit, covered by codegen golden tests.
- **`main` implicit scope + infinite `Timer.every`** → `main` returning while a
  `Timer.every` runs: the implicit scope cancels ambient timers on normal exit
  (same rule as any `concurrent` block).

## Migration Plan

1. `ADR-018`; spec deltas + the two new-capability specs; `openspec validate`.
2. Runtime: scope table + join + cancellation sweep + timer re-arm; unit tests.
3. C-ABI rename + new entry points; codegen golden tests adjusted.
4. Lexer/AST/parser: `concurrent` block, `spawn`, binding list.
5. Sema: `Base::Job`, `Timer` members, hoisting, Rule B DAG, single-consume.
6. IR/codegen: `ScopeEnter/Exit`, `BranchStart`, `JobWait`, per-branch thunk.
7. CLI fixtures + `examples/concurrent_examples.zrk`.
8. Docs: semantics doc sections, handbook chapter rewrite, `Timer` page,
   roadmap, feature status.
9. `cargo test --workspace` + fmt + clippy; commit; website sync with reviewed
   `--audit-date`; commit site separately.

## Open Questions

- `Timer.after` that has not fired when its scope closes: cancelled (D5) — but a
  "send a final ping" use wants it to run. Accept the loss; `#5`'s `detach`
  covers deliberate outlive-me work.
- Non-binding statements inside `concurrent { }` sharing the block's own branch
  vs each being its own branch: proposed — one "block body" branch in source
  order, simplest.
