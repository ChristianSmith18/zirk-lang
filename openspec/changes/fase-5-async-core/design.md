## Context

Zirk compiles `.zrk` through `zirk-lexer` → `zirk-parser` → `zirk-ast` →
`zirk-sema` → `zirk-ir` (+ `lower.rs`) → `zirk-codegen-llvm` → LLVM 20 → native
binary, linked against the `zirk-runtime` staticlib (`ADR-002`).

State relevant to this change:

- **Keywords already reserved, nothing behind them.** `Keyword::Task`,
  `Keyword::Await`, `Keyword::Parallel`, `Keyword::Thread`, `Keyword::Sync` lex
  today and are gated to Phase 5 by `zirk-feature-phasing`. The parser emits the
  "construct outside the subset" diagnostic for `task` / `parallel` / `thread`
  (`zirk-grammar`). `Channel<T>`, `Atomic<T>`, `TaskSettlement<T>` are
  `pending_type` entries.
- **No runtime scheduler.** `crates/zirk-runtime/src/lib.rs` says "provides the
  application lifecycle and, later on, memory, scheduler, ...". There is no
  executor, no context switch, no task type.
- **The collector assumes one thread everywhere.** `crates/zirk-runtime/src/collector.rs`
  is a non-moving mark-sweep collector (`ADR-003`). Roots are enumerated through a
  **function-granularity shadow stack** (`ADR-003` design D2/D4): codegen emits
  `zirk_rt_push_frame(roots, count)` on entry and `zirk_rt_pop_frame()` before
  every `return`, threading an intrusive linked list of frames through the
  runtime. `collector.rs:100`: "sound under the single-threaded execution model
  this collector already assumes everywhere else (no `parallel`/`thread` ...)".
  Every managed-reference SSA value that lives only transiently between two call
  arguments is spilled to a synthetic root slot the instant it is produced
  (design D4), because a naive shadow stack cannot see it.
- **Cleanup edges already exist.** Phase 4b (`try`/`catch`/`finally`), 4c
  (`match ... with` resources, left-to-right acquire / right-to-left close), and
  4e (`unsafe` journal rollback on every exit including `return`/`break`/`continue`)
  all lower to IR that runs cleanup handlers on normal, exceptional, and
  early-jump exit edges. `Throwable.suppressed()` (Phase 4b) already aggregates
  secondary failures during unwinding.
- **Escaping closures are already GC-tracked heap objects.** Phase 4d lowers a
  captured closure to `{ function pointer, capture-block pointer }` where the
  capture block is a heap-allocated, descriptor-described, collector-traced
  object (`MakeCallable` / `CallCallable`). `.clone()` deep-copies a capture block
  when every capture is `Clone`.
- **`Duration` is exact `i64` nanoseconds** with suffix literals `ns`..`w`
  (`crates/zirk-runtime/src/duration.rs`), already usable in annotations and
  arithmetic. `after 5s` and `timeout 5s` reuse it directly.
- **Normative source of truth** for this whole area is
  `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` and the `zirk-structured-concurrency`
  capability spec (already written in full, describing the *mature* language).
  This change *implements* steps 1–3 of that spec; it does not redefine it.

`docs/init/ZIRK_ROADMAP.md` Phase 5 is explicitly six sub-steps. This change is
step 0 (a decision) + steps 1–3. Steps 4–6 (`Atomic<T>`, `Mutex<T>` +
synchronizers, scoped `thread`, `task.blocking`, `parallel`) are a separate
change because they force the collector to become multi-threaded.

## Goals / Non-Goals

**Goals:**

- Deliver a working cooperative async model end to end: `task` / `await` /
  `task scope`, sibling-failure propagation, cancellation, `cancellation shield`,
  `await ... timeout`, `Task.all` / `first` / `settled`, `select`, and
  `Channel<T>` (bounded / unbounded / rendezvous / broadcast / watch / one-shot).
- Zero function coloring. A plain function call has the same ABI and the same
  source form whether or not the callee awaits internally. There is no `async fn`.
- Keep the collector single-threaded and non-moving. The only collector change is
  generalizing one root chain into one-per-task and adding a channel trace hook.
- A suspended task's references are never collected while the task is alive.
- Build the `Transfer` / `Share` compile-time analysis now, at full strength for
  the boundaries this change delivers, so steps 4–6 inherit a finished analysis.
- Layered, runnable tests: unit tests in `zirk-runtime` for the executor and
  channels, IR/codegen golden tests for suspension points, and `.zrk` CLI
  fixtures for every scenario in the specs.

**Non-Goals:**

- Multi-threaded execution of any kind. The executor is one OS thread. `parallel`,
  `thread`, and `task.blocking` stay gated.
- `Atomic<T>`, `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`. Gated /
  pending.
- A public application-root-supervisor API (`application.spawn_service`). The
  executor owns `main`'s root scope internally; there is no user surface for
  detaching long-lived work in this change, and general `detach()` stays absent.
- An asynchronous I/O reactor. I/O calls stay blocking; there is no `await` on a
  file descriptor. `stdout.println` etc. are unchanged. (`task.blocking`, the
  sanctioned way to run a blocking call off the scheduler, is step 5.)
- A moving / generational / concurrent collector. `ADR-003` stays as decided.
- Stack-overflow-proof task stacks via OS guard pages on every platform. This
  change uses a fixed default stack with a cheap software checkpoint; a
  guard-page implementation is a follow-up.
- Changing any scalar, text, or temporal semantics.

## Decisions

### D1: Stackful coroutines on a single-threaded cooperative executor

Each `task` owns a **heap-allocated contiguous stack** (default 128 KiB, tunable
later). Starting a task allocates a stack and a **task control block (TCB)** and
enqueues the task. `await`, a suspending channel operation, `select`, and a timer
wait perform a **cooperative context switch**: save callee-saved registers + SP +
PC into the current TCB, restore them from the executor's own context, and return
into the executor loop. Resuming a task is the reverse.

A plain function call is an ordinary native `call`. A function that internally
does `task x(); await x;` runs on whatever task's stack is current; its frame is
part of that stack and is suspended and resumed wholesale with it. **No IR
transformation of function bodies. No dual ABI. No `async`/non-`async` split.**

_Alternatives considered:_

- **Stackless state-machine transformation (LLVM coroutine intrinsics or a
  hand-rolled IR pass).** Every function that can reach an `await` — transitively,
  because a callee's suspension must suspend the caller's frame — would be
  rewritten into a resumable state machine with a heap frame and a second entry
  ABI. That is exactly the "what color is your function" problem, and it directly
  contradicts `STRUCTURED_CONCURRENCY_SEMANTICS.md` §1 ("There is no `async fn`.
  A function describes its logical input/output contract"). It also complicates
  every existing pipeline stage (the checker would need an effect system, the IR
  a frame type, codegen two ABIs). Rejected. The one thing stackless buys —
  precise, cheap frames with no reserved stack — is not worth reworking the whole
  compiler for a single-threaded executor.
- **Segmented / growable stacks (Go early-model).** Avoids over-reserving memory
  but introduces "hot split" costs and complicates the context-switch shim and FFI
  (`extern "C"` calls into C need a contiguous stack). Deferred; the default-size
  contiguous stack is simpler and adequate for step 1–3 workloads.
- **One OS thread per task (Loom-style virtual threads need a scheduler anyway;
  1:1 threads).** Defeats the entire purpose — Zirk `task` is explicitly *not*
  `thread` (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §1, §16) — and would drag the
  collector into multi-threading immediately. Rejected.

### D2: GC roots across suspension — one shadow-stack chain per task

Today `collector.rs` holds a single process-global shadow-stack head. This change
moves the head into the **TCB**: `tcb.shadow_stack_head`. The executor sets a
`CURRENT_TASK` runtime global on every resume; `zirk_rt_push_frame` /
`zirk_rt_pop_frame` operate on `CURRENT_TASK.shadow_stack_head`. The `main` task
is task 0 and behaves exactly as the current global chain does.

Root enumeration in `collect()` changes from "walk the one chain" to "for every
task the executor still owns (ready, running, suspended, or in cleanup), walk its
chain". The executor exposes an iterator over live TCBs to the collector. The
`crate::clone::mark_clone_roots` extra root set is unchanged.

Because execution is single-threaded and collection is cooperative (only ever
triggered from inside `zirk_rt_alloc`, `collector.rs:7`), a collection can only
begin while exactly one task is running and every other task is at a well-defined
suspension point with a consistent, fully-pushed shadow-stack frame. No
stop-the-world machinery is needed. The design D4 "spill every transient managed
SSA value immediately" rule from `ADR-003` already guarantees each suspended
frame's roots are all visible.

Object header: **unchanged** (three words per `ADR-012`). No per-object task
tagging.

_Alternatives considered:_

- **Conservative stack scanning of suspended task stacks.** Would remove the need
  for the shadow stack entirely, but `ADR-003` already rejected conservative
  scanning for the main stack (precision, and it interacts badly with a future
  moving collector). Keeping one root-enumeration strategy is worth the small
  generalization. Rejected.
- **Copy each task's roots into a side table at suspension.** Extra work on the
  hot suspend path and a second source of truth. The intrusive per-TCB chain is
  already the natural representation. Rejected.

### D3: The executor

A single-threaded loop in `crates/zirk-runtime/src/executor.rs`:

- **Ready queue**: FIFO `VecDeque<TcbPtr>`. A task is enqueued when created, when
  unblocked (its awaited task completed, a channel operation became possible, a
  timer fired, a `select` guard became ready), or when it voluntarily yields.
- **Timer wheel / heap**: a binary min-heap keyed by deadline (`Instant` built on
  the monotonic clock already used by `Duration` / temporal `now_*`). Checked once
  per loop turn; expired timers unblock their waiter or fire their `select` guard.
- **Turn**: pop a ready task, set `CURRENT_TASK`, switch to it, run until it
  suspends or completes, switch back, service timers, repeat. Empty ready queue
  with a non-empty timer heap → sleep until the nearest deadline. Empty ready
  queue and empty timer heap with children still alive → deadlock; the executor
  reports it and aborts (a real bug in the program, like a `select` with no
  reachable guard).
- **Fairness**: strict FIFO for the ready queue; `select` with multiple ready
  guards picks via a rotating start offset so no guard starves. No priorities
  (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §3).
- **Lifecycle**: `zirk_rt_main` (already the entry wrapper) creates task 0 for
  `main`'s body, runs the loop until task 0 and all its descendants finish, then
  returns `main`'s exit code. An uncaught exception out of task 0 propagates as
  today.

_Alternative considered:_ an off-the-shelf async runtime (embed a cut-down
`tokio`/`smol`). Rejected: those are multi-threaded, `Future`-based, and would
impose a `Future` ABI on generated code — the opposite of D1. Zirk's executor is
tiny (a queue, a heap, a switch) and must match Zirk's exact structured
semantics.

### D4: `Task<T>` and the task control block

`Task<T>` at the language surface is a **GC-tracked handle object**: one word
pointing at the TCB (plus the standard object header). The TCB (runtime-private,
not a Zirk object) holds:

| Field | Purpose |
|---|---|
| `state` | `Ready` / `Running` / `Suspended` / `Completed` / `Failed` / `Cancelled` |
| `stack` | base + size of the heap stack |
| `context` | saved callee-saved registers, SP, PC |
| `shadow_stack_head` | this task's GC root chain (D2) |
| `result` | a slot holding `T` on success or a `Throwable` on failure |
| `result_consumed` | set by `await`; a second `await` is a compile-time error with this as the runtime backstop |
| `scope` | owning structured scope (D5) |
| `sibling_next` / `first_child` | scope child list |
| `cancel_requested` / `cancel_reason` | cooperative cancellation flag + optional typed reason |
| `shield_depth` | `cancellation shield` nesting; delivery deferred while `> 0` |
| `waiter` | the single task blocked in `await` on this handle, if any |
| `cleanup_state` | tracks that structured cleanup ran before the TCB is freed |

`await` consumes the result exactly once. The checker enforces single-consume as
a linear-use property of the handle binding (`STRUCTURED_CONCURRENCY_SEMANTICS.md`
§2); the `result_consumed` flag + `fatalError` is the runtime backstop for cases
the checker cannot see (a handle passed through a field). Multiple observers use
`watch` / `broadcast` / a channel / a shared `inmut::strict` value, not implicit
cloning.

`_ = task_handle` acknowledges "only completion and cleanup matter"; it does
**not** detach — the scope still owns and awaits the task.

### D5: Structured scopes

Every function body is an implicit structured scope. `task scope { block }` is an
explicit nested supervisor and is also an expression (its value is the block's
value). A scope object (runtime-private) holds its child list and its state.

Lowering: `task scope { body }` becomes `enter_scope` / `body` /
`exit_scope`, with `exit_scope` installed as a **cleanup handler on every exit
edge** of the block — normal fall-through, `return`, `break`, `continue`, and
exceptional unwind — reusing the exact machinery Phase 4b/4c/4e already use for
`finally` / resource close / journal rollback. `exit_scope` runs the join /
cancel-and-cleanup protocol (D6) before control leaves.

An implicit function-body scope with no `task` in it compiles to nothing extra —
the scope is only materialized when a child is actually created, so non-async
code pays zero cost.

### D6: Failure propagation

Per `STRUCTURED_CONCURRENCY_SEMANTICS.md` §4, on the first unhandled child
exception the scope:

1. records it as the **primary** failure;
2. sets `cancel_requested` on every active sibling and unblocks them at their
   next safe point;
3. awaits every sibling's cleanup to finish;
4. appends any further sibling / cleanup failures to the primary's
   `suppressed()` list (Phase 4b machinery, unchanged);
5. propagates the primary out of the scope.

A task that returns `Result.Error(e)` **fulfilled** with that value — it is not
rejection. Only an unhandled `Throwable` escaping the task body is rejection.
`CancelledError` is a new compiler-known `RuntimeError` subclass (see D-errors),
thrown into a task at its next safe point after `cancel()`; it participates in
normal `try`/`catch` and `finally`, so a task can clean up.

### D7: Cancellation and shields

`operation.cancel()` sets `cancel_requested` (idempotent) with an optional typed
reason (default `CancellationReason.Cancelled`) and, recursively, on all
descendants. At the next **safe point** — `await`, a suspending channel
operation, a `select`, a timer wait, or an explicit `cancellation.check()` — the
runtime throws `CancelledError` into the task unless `shield_depth > 0`.

`cancellation shield { body }` increments `shield_depth` on entry and decrements
on exit (on every exit edge, D5-style). If a cancellation arrived while shielded,
it is delivered *immediately after* the shield's closing edge — it is deferred,
never swallowed (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §7). Unrelated exceptions
raised inside the shield propagate normally. Shields are expected to be bounded; a
diagnostic for a provably-unbounded shield is a lint, deferred.

### D8: Timeout

`await operation timeout duration` (duration a non-negative `Duration`):

1. arm a timer at `now + duration`;
2. `await` the operation normally;
3. if the operation completes first, disarm the timer and produce its value;
4. if the timer fires first, `cancel()` the operation, await its cleanup, and
   throw the new compiler-known `TimeoutError`. The timed operation never
   continues as a background task.

### D9: Aggregation

`Task.all` / `Task.first` / `Task.settled` are **compiler-known static methods**
on `Task` (like `Result`'s known API), backed by runtime helpers:

- `Task.all(tasks)` → `List<T>` in input order; first unhandled failure cancels
  the unfinished tasks, awaits cleanup, propagates with the rest suppressed.
- `Task.first(tasks)` → the first task to complete (success *or* unhandled
  failure per its own outcome); cancels and cleans the remainder.
- `Task.settled(tasks)` → `List<TaskSettlement<T>>` in input order; every task is
  allowed to finish; no sibling is cancelled because another rejected.

`TaskSettlement<T>` is a **compiler-known generic enum**
`{ Fulfilled(T), Rejected(Throwable), Cancelled(CancelledError) }`, registered
the way `Result<T,E>` and the merged `Either<L,R>` generic enums already are. A
`Task<Result<U,E>>` returning `Error(e)` settles as `Fulfilled(Error(e))`.

### D10: `select`

```
select {
    msg = await ch.receive() => handle(msg),
    _   = await op          => finish(),
    after 5s                => timeout_branch(),
    cancelled               => cleanup(),
    default                 => poll_branch(),   // optional, non-suspending
}
```

Lowering:

1. Evaluate each guard's *operand* once (channel, task handle, duration), in
   source order, per `STRUCTURED_CONCURRENCY_SEMANTICS.md` §10 ("branch values
   follow ordinary copy/transfer rules").
2. Register interest on every guard.
3. If any guard is already ready → skip suspension; if several are ready, pick one
   by rotating offset (fairness).
4. Otherwise, if a `default` branch exists → run it. Else suspend.
5. On wake, pick one ready guard fairly, **deregister the others (losers stay
   alive and are not cancelled)**, bind the branch pattern, run that branch.
6. Channel closure counts as a ready outcome, not an infinite wait.

### D11: `Channel<T>` runtime

New `crates/zirk-runtime/src/channel.rs`. A channel is a **GC-traced heap object**
with a custom trace hook registered with the collector (the queued `T` values are
roots when `T` is a reference type — walk the ring buffer / list).

- **Bounded** `Channel<T>(capacity: n>0)`: ring buffer of `n`, plus a FIFO queue
  of suspended senders and one of suspended receivers. Full → sender suspends
  (backpressure). Empty → receiver suspends.
- **Rendezvous** `Channel<T>(capacity: 0)`: a send completes only when paired with
  a receive; no buffer.
- **Unbounded** `Channel.unbounded(limit: n)`: growable list with a **mandatory**
  defense limit; reaching `limit` applies backpressure or returns the documented
  typed failure rather than allocating without bound
  (`zirk-structured-concurrency` "Typed channels and closure").
- `send` / `receive` suspend and cooperate with the executor; `try_send` /
  `try_receive` never suspend and return a typed outcome distinguishing value,
  temporary fullness/absence, and closure.
- `close()` is idempotent, wakes every suspended sender and receiver; queued
  values remain receivable and **drain before** closure is observed.
- `is_closed`, `capacity`, `length` are properties.
- **Families**: `broadcast` (every live subscriber sees every post-subscription
  value), `watch` (latest value only, new subscribers see the current value),
  one-shot (exactly one value, then closed). Same transfer and cancellation
  rules; same closure/drain semantics.

### D12: `Transfer` / `Share` derivation (step 3)

A new analysis in `zirk-sema` computes two **non-user-forgeable** properties for
every type and checks them at concurrency boundaries — `task` creation and its
captures, `task scope` return values, channel `send` / `receive`, and `select`
branch values (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §12–13):

- **`Transfer`** (may cross to another context): value types and their
  projections (they copy); an **exclusive** mutable *complete* reference (moves;
  the sender then cannot use it until it returns through a structured result or a
  channel — enforced with a light move/liveness check reusing the Phase 4d
  capture-analysis and Phase 4e `transfer(r)` infrastructure); a `clone()` result
  (independent graph); a complete `inmut::strict` reference.
- **`Share`** (referent may be touched concurrently): a complete `inmut::strict`
  reference. Synchronization-aware types would also qualify — none exist in this
  change, so the rule is present but currently only `inmut::strict` satisfies it.
- **Rejected**: a mutable alias that would remain reachable by the parent while a
  child can mutate it. Diagnostic names the four escape hatches: transfer, make it
  `inmut::strict`, `clone()`, or route it through a channel.

**Runtime note.** With a single OS thread and cooperative scheduling there is no
*memory-model* data race in step 1–3 — nothing preempts, nothing runs in
parallel. The analysis still matters because (a) it forbids **logical** races
(task A reads a shared mutable value, awaits, task B mutates it, task A resumes on
a stale assumption) and (b) steps 4–6 turn on real parallelism and must inherit a
finished, already-enforced analysis rather than a retrofit. This is stated
explicitly in the `zirk-memory-safety` delta.

### D13: Keyword / type gating and the pipeline touch-points

- `zirk-lexer`: `select`, `scope`, `shield`, `timeout` become **contextual**
  keywords (identifiers elsewhere). `task` / `await` already lex.
- `zirk-parser` / `zirk-ast`: new nodes `TaskExpr`, `TaskScopeExpr`, `AwaitExpr {
  operand, timeout: Option<Duration expr> }`, `SelectExpr { arms, default }`,
  `CancellationShieldStmt`. `task` / `await` / `task scope` / `select` /
  `cancellation shield` are removed from "constructs outside the subset";
  `parallel` / `thread` stay listed.
- `zirk-sema`: known types `Task<T>`, `Channel<T>`, `TaskSettlement<T>`,
  `CancellationReason`; known throwables `CancelledError`, `TimeoutError`; `await`
  typing → exactly `T`; single-consume linear check; `Task.*` static methods;
  D12 analysis; task-capture analysis.
- `zirk-ir` / `lower.rs`: suspension-point instructions, `enter_scope` /
  `exit_scope` with cleanup edges on every exit, channel ops as
  possibly-suspending runtime calls, `select` lowering, shield depth in/out,
  timeout timer arm/disarm.
- `zirk-codegen-llvm`: the context-switch shim (D14), per-task
  `zirk_rt_push_frame` targeting `CURRENT_TASK`, executor entry wrapping `main`.
- `zirk-runtime`: `executor.rs`, `task.rs`, `channel.rs`, `timer.rs`;
  `collector.rs` multi-chain root walk + channel trace hook; `lib.rs` lifecycle.
- `zirk-feature-phasing`: move the delivered constructs into the implemented
  subset (the `**` operator is the precedent for this wording).

### D14: The context-switch shim across the four target triples

`ADR-004` pins the supported targets: macOS aarch64, Linux x86_64, Linux aarch64,
Windows x86_64. The shim must save/restore callee-saved registers, SP, and the
return address, and switch stacks. Two candidate implementations, decided during
implementation:

1. **Hand-written assembly**, one `.S` per (arch, os) — ~40 lines each,
   `global_asm!` or a build-script-compiled object. Full control, no dependency,
   matches `ADR-002` (self-contained staticlib). This is the leaning choice.
2. **A vetted crate** (`corosensei`, which is `no_std`, MIT, and already
   abstracts exactly these four targets). Less code to own, but a new dependency
   in the runtime staticlib and less control over the exact frame it builds.

Either way this is isolated behind a `fn switch(from: &mut Context, to: &Context)`
seam with a Rust fallback path used only in `cargo test` on the host.

### D15: ADR deliverable

`docs/decisions/ADR-017-modelo-de-suspension.md` records D1 (stackful coroutines,
single-threaded cooperative executor, no function coloring) and D2 (per-task
shadow stack, generalized root enumeration, unchanged object header), with the
alternatives above. It is referenced from `ADR-003` (memory) and the roadmap.
`ADR-004` (portability) gets an addendum for the context-switch shim per triple.

## Risks / Trade-offs

- **[Context-switch shim is per-platform unsafe assembly.]** → Isolate behind one
  `switch()` seam; a host-only Rust fallback for `cargo test`; a dedicated
  `zirk-runtime` test that spins up N tasks doing ping-pong context switches on
  each CI target before anything else in step 1 is built on top.
- **[A suspended task's frame roots are missed → use-after-free.]** → This is the
  single highest-severity failure mode. Mitigation: the D4 "spill every transient
  managed SSA value immediately" rule already in the collector; a targeted test
  matrix — a task holding a reference only in a spilled SSA slot across an
  `await`, a task holding a reference in a named local across an `await`, a
  channel with queued reference values collected mid-drain, a `Task<T>` handle
  whose `result` slot holds the only reference to an object — each run under
  forced collection at the suspension point.
- **[Fixed 128 KiB stack per task over-reserves memory / overflows on deep
  recursion.]** → Acceptable for step 1–3 (tens to hundreds of tasks). A cheap
  software stack-limit check at function entry (codegen already emits a frame
  prologue) throws a catchable `StackOverflowError`; a guard-page implementation
  is tracked as follow-up, not blocking.
- **[`select` fairness / losing-operation lifetime bugs are easy to get subtly
  wrong.]** → Spec scenarios pin the observable behavior (message before timer;
  losers stay alive; closure is a ready outcome); property-style runtime tests
  hammer multi-ready selection for starvation.
- **[Cancellation delivered at an unsafe instant corrupts state.]** →
  `CancelledError` is only ever thrown at defined safe points, never
  asynchronously mid-instruction (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §6);
  shields cover the cleanup-critical regions.
- **[`Transfer`/`Share` analysis too strict → ergonomic cliff, or too loose →
  false safety.]** → Scope it to exactly the boundaries this change delivers;
  reuse the Phase 4d capture analysis and Phase 4e `transfer(r)` rather than a new
  engine; every rejection scenario in the spec carries the suggested fix.
- **[The collector's live-TCB iterator and the executor's queues drift out of
  sync, freeing a task the collector still walks.]** → One owner: the executor is
  the sole authority on TCB liveness; a TCB is freed only after `cleanup_state`
  says structured cleanup finished *and* no root chain references it; the
  collector borrows the iterator, never mutates it.
- **[Scope of the change.]** 1 new + 8 modified specs, ~8 crates. → Sequenced in
  `tasks.md` so each step has a runnable milestone: shim → executor + task 0 →
  `task`/`await` → scopes/failure → cancellation/shield/timeout →
  aggregation/`select` → channels → Transfer/Share. Nothing after the shim
  proceeds until the prior milestone runs.

## Migration Plan

- No source migration: `task` / `await` do not compile today, so no program
  depends on their behavior beyond the phase diagnostic.
- Roll-forward is per-milestone (see `tasks.md`); each milestone is independently
  revertable because later milestones only add nodes/instructions/runtime modules.
- Documentation and `../zirk-lang-site` update after the crates land: run
  `./scripts/sync-website-content.sh`, review the site status catalog for the
  Phase 5 rows and the shrunken concurrency limitations, pass
  `--audit-date YYYY-MM-DD`.

## Open Questions

- **Context-switch shim: hand-written asm vs `corosensei`.** Still open —
  resolved in implementation (task 1.6) after the isolated per-target ping-pong
  test (task 2.2); the seam is the same either way.
- **Does `task scope` as an expression allow `break` / `continue` to cross it, or
  only `return` and fall-through?** Leaning: same rule as a closure body — only
  `return` and fall-through; `break`/`continue` targeting an outer loop across a
  `task scope` boundary is a diagnostic. Confirm against the grammar spec's
  loop/label rules during spec review.
- **`broadcast` / `watch` / one-shot: full parity in this change or a documented
  follow-up within Phase 5 step 2?** The proposal includes them; if
  implementation pressure is high, the standard `Channel<T>` ships first and the
  families follow in the same change before archive.

### Resolved (task group 1)

- **Default task stack size and configurability (task 1.4).** Fixed at **128
  KiB**, allocated from the ordinary heap, **not user-configurable** in this
  change. A per-`task` stack-size option and a guard-page implementation are
  tracked as follow-up, not blocking. Rationale: step-1–3 workloads are tens to
  hundreds of tasks; 128 KiB × 200 tasks ≈ 25 MiB worst case is acceptable, and a
  fixed size keeps `make_context` and the switch shim trivial.
- **`StackOverflowError` (task 1.5).** Ships in this change as a concrete,
  compiler-known, catchable `RuntimeError` subclass, raised by a **cheap software
  frame-limit check** emitted in the function prologue (codegen already emits a
  frame prologue; the check is a compare against the current task's stack limit
  stored in the TCB). A hardware guard-page implementation is deferred. The
  `zirk-errors` delta and task 9.6 are updated to make this non-optional.
