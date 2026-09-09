## Context

`ADR-017` chose stackful coroutines on a single-threaded cooperative executor.
`crates/zirk-runtime/src/context.rs` (delivered) is the seam: `TaskContext`
owns a task's stack, the executor drives it with `resume() -> Run`, the task
yields with `Suspender::suspend()`. On the four supported triples the switch is
`corosensei`; every other host gets a thread-backed fallback with the same API.

`async-runtime-core` (main spec) defines what the executor, timer, and root
enumeration must do. `zirk-memory-safety` requires root enumeration to span
suspended tasks. `zirk-native-codegen` requires per-task shadow-stack frame
registration and an executor entry wrapping `main`.

The collector today (`collector.rs`, `ADR-003`) is non-moving mark-sweep with one
process-global shadow-stack head, cooperative collection triggered only inside
`zirk_rt_alloc`, and the design-D4 rule that every transient managed reference is
spilled to a synthetic root slot the instant it is produced.

## Goals / Non-Goals

**Goals**

- A working executor: ready queue, timer service, `spawn`, cooperative
  `suspend`/`resume`, `await`-by-id with wake-on-completion, unresolvable-wait
  detection, `main`-as-root-task lifecycle.
- Per-task garbage-collection roots: a suspended task's references survive
  collection, verified by a targeted test matrix.
- Everything exercised by Rust tests. No language surface, no codegen changes.
- The `extern "C"` entry points a future codegen will call exist, even if their
  final ABI is settled in `fase-5-structured-tasks`.

**Non-Goals**

- `task` / `await` / `select` / `cancellation shield` parsing, checking,
  lowering, codegen — `fase-5-structured-tasks`.
- Structured scopes, sibling-failure propagation, cancellation delivery,
  `cancellation shield`, `await ... timeout` semantics — those need the language
  surface; only the executor primitives they will build on are in scope here.
- Channels, aggregation, `Transfer`/`Share` — later changes.
- Anything multi-threaded. The collector stays single-threaded.
- A public application-root-supervisor API.

## Decisions

### D1: Task registry — a generational slab

Tasks live in a `Vec<Option<TaskSlot>>` in the executor; a `TaskId` is
`{ index: u32, generation: u32 }`. Freeing a task clears its slot and bumps the
slot generation, so a `TaskId` held after the task is gone (a stale `await`
target, a `Task<T>` handle) is detectably dead rather than aliasing a new task.
Rationale: `await` must reject a second consume and must not confuse a reused
slot for the original task; a generation check is O(1) and needs no allocation.

_Alternative:_ `Box<Task>` + raw pointer as the id. Rejected — dangling-pointer
hazard, and the collector needs to iterate all live tasks anyway, which a slab
gives for free.

### D2: The task control block

```
state:            Ready | Running | Suspended | Completed | Failed | Cancelled
context:          TaskContext            (owns the stack)
outcome:          Option<TaskOutcome>    (Value(usize) | Panicked(Box<dyn Any>))
result_consumed:  bool
waiter:           Option<TaskId>         (the single task blocked awaiting this one)
wait:             WaitReason             (None | Yielded | AwaitingTask(TaskId) | Timer(TimerId))
cancel_requested: bool
cancel_reason:    Option<CancelReason>
shield_depth:     u32
shadow_stack:     ShadowStackHead        (this task's GC root chain, D4)
cleanup_state:    Pending | Done
```

`cancel_*` and `shield_depth` are laid down now but only *read* by
`fase-5-structured-tasks` (cancellation delivery needs safe points that only the
language surface introduces). Keeping the fields here avoids reshaping the TCB
later.

### D3: `suspend_current()` and the thread-local yielder

Generated code (and future runtime functions like `zirk_rt_task_await`) suspend
without holding a `&Suspender`. `context.rs` gains a thread-local
`*const Yielder`, set at task-body entry and **re-armed immediately after every
resume** (inside both `Suspender::suspend` and `suspend_current`). Because the
executor is single-threaded and cooperative, whenever any task is running the
thread-local points at *its* yielder: a task only ever resumes from inside its
own last suspend call, which re-arms before returning control to the task body.

`context.rs` exports `pub fn suspend_current()`. The executor calls it (never the
`Suspender` directly) from `suspend_current(reason)` after recording the reason
in the running task's TCB.

### D4: Per-task shadow-stack chain

`collector.rs`'s single global head moves into `TaskSlot.shadow_stack`. A
`CURRENT_TASK: Cell<Option<TaskId>>` thread-local is set by the executor on every
resume. `zirk_rt_push_frame` / `zirk_rt_pop_frame` resolve `CURRENT_TASK` and
operate on that task's chain; when no task is current (before the executor
starts, or between turns) they operate on a **bootstrap chain** that becomes
task 0's chain once `main` is spawned.

`collect()` changes from "walk the one chain" to "walk the bootstrap chain, then
every live task's chain". The executor exposes `for_each_live_task_roots(f)`.
`crate::clone::mark_clone_roots` stays a separate extra set.

Object header unchanged. No stop-the-world: a collection still only begins inside
`zirk_rt_alloc`, i.e. while exactly one task runs and every other task sits at a
suspension point with a fully-pushed frame.

_Migration hazard:_ existing collector tests assume the global head. They are
updated to drive a task 0 (or the bootstrap chain) explicitly.

### D5: `await`-by-id

`await(target: TaskId) -> TaskOutcome`:
1. generation check — a dead id is a `fatalError` (the checker will make the
   common case unreachable; this is the backstop).
2. if `target` already `Completed`/`Failed` and `!result_consumed` → take the
   outcome, set `result_consumed`, return.
3. a second consume → `fatalError` "task result already consumed".
4. otherwise record `wait = AwaitingTask(target)`, set `target.waiter = self`,
   `suspend_current`. On resume, the target is done — take and return.

When a task completes, the executor moves its return value into `outcome` and, if
`waiter` is set, marks the waiter `Ready` and enqueues it.

### D6: The executor loop

```
loop {
    now = clock();
    for timer in timers.poll_expired(now): wake its waiter
    task = ready.pop_front()  else:
        if timers.is_empty() && any task still Suspended: abort "unresolvable wait"
        if timers.is_empty(): break        // all done
        sleep_until(timers.peek_deadline()); continue
    CURRENT_TASK = task.id
    match task.context.resume():
        Suspended  -> leave it Suspended (its `wait` says why); do not re-enqueue
                      unless `wait == Yielded`, then re-enqueue at the back
        Finished(v) -> outcome = Value(v); state = Completed;
                       wake waiter; run reclamation check
}
```

Fairness is strict FIFO. No priorities. `sleep_until` on an idle executor bounds
the wait by the nearest deadline.

### D7: Reclamation

A `TaskSlot` is freed (slot set to `None`, generation bumped, `TaskContext`
dropped → stack freed) when: `state` is terminal, `result_consumed` is true (or
the task returned `Void`/no waiter ever), `cleanup_state == Done`, and it is not
the currently-running task. `cleanup_state` is `Done` trivially in this change
(structured cleanup arrives with the language surface); the field and the gate
exist so `fase-5-structured-tasks` only has to flip it.

Dropping a still-suspended `TaskContext` unwinds its stack (delivered in
`context.rs`), so an abandoned task (e.g. after `unresolvable wait` abort in
tests) cleans up its locals.

### D8: `main` lifecycle

`zirk_rt_main` (entry wrapper) creates task 0 from `main`'s body, runs the
executor loop until task 0 and all descendants are terminal, then returns task
0's exit code. An exception escaping task 0 propagates as today (nonzero exit).
For this change, `main`'s body is still ordinary synchronous generated code that
never suspends — task 0 runs start to finish on the first `resume()`. The
lifecycle plumbing is in place for when `main` can spawn children.

### D9: The `extern "C"` surface

Defined now, Rust-tested now, codegen-wired later:
`zirk_rt_task_spawn(body: extern "C" fn(*mut c_void) -> usize, arg: *mut c_void)
-> u64` (packed `TaskId`), `zirk_rt_task_await(id: u64) -> usize`,
`zirk_rt_task_is_done(id: u64) -> bool`. The packed-`u64` `TaskId` and the
`usize` result are provisional — `fase-5-structured-tasks` may change them when
the real `Task<T>` value representation is decided. Marked as such in the module
docs.

## Risks / Trade-offs

- **[A suspended task's roots are missed → use-after-free.]** Highest severity.
  → The D4 spill rule already in the collector; a dedicated test matrix (named
  local across a suspend, spilled-temporary across a suspend, task-0 root, a
  cycle owned only by a suspended task), each run under a forced collection at
  the suspension point.
- **[Thread-local yielder points at the wrong task.]** → Re-arm on every resume
  (D3); an assertion in `suspend_current` that `CURRENT_TASK` is set and the
  yielder is non-null; a test that interleaves three tasks each suspending via
  `suspend_current` and checks each resumes correctly.
- **[Collector test migration misses a case.]** → Run the full existing
  allocation-pressure and cycle suite plus the new tests; the object-header
  three-word assertion stays.
- **[The `extern "C"` ABI churns in the next change.]** → Accepted and
  documented; only Rust tests depend on it here, and `fase-5-structured-tasks`
  owns the real `Task<T>` representation.
- **[Reused-slot confusion in `await`.]** → Generational `TaskId` (D1); a test
  that spawns, awaits, frees, spawns again into the same slot, and confirms an
  old id is rejected.

## Migration Plan

- Per-task-group milestones in `tasks.md`; each leaves `cargo test -p
  zirk-runtime` green. `context.rs` change first (it is small and everything
  depends on it), then `task.rs` + `executor.rs`, then the collector
  integration, then the lifecycle.
- No source migration — no `.zrk` behavior changes.
- Docs + `../zirk-lang-site`: after the crate lands, fill in
  `ZIRK_RUNTIME_SPEC.md` §3 timer/lifecycle detail, add the
  `ZIRK_FEATURE_STATUS.md` Phase 5 infrastructure row, run
  `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`.

## Open Questions

### Resolved (group 8)

- **Root task stack size.** `main` used to run on the OS main-thread stack, so
  the root task gets **8 MiB** (`executor::ROOT_TASK_STACK_BYTES`), not the
  128 KiB a spawned task gets — one allocation for the life of the program.
  Verified: the full `zirk-cli` fixture suite (42 compiled + run `.zrk`
  programs) passes with `main` on the root task.
- **Does `zirk_rt_run_main` sit before or after the uncaught-exception check?**
  Before. `zirk_rt_run_main` runs the executor to completion, then the generated
  C `main` runs its existing `has_pending_exception` → `uncaught_exception`
  check, then `zirk_rt_shutdown` — Zirk exceptions are a pending-slot mechanism,
  not Rust unwinding, so they survive the return from `zirk_rt_run_main`.

### Open

- **Bootstrap chain vs. requiring task 0 before any allocation.** Leaning:
  bootstrap chain, folded into task 0 on spawn — generated `main` prologue may
  allocate before `zirk_rt_main` finishes wiring the executor.
- **Does `zirk_rt_task_spawn` need to run the child eagerly (one `resume` before
  returning) to match "task creation starts the child immediately"?** Leaning:
  enqueue Ready but do not resume until the loop picks it up; "immediately" is
  satisfied by "before the parent's next suspension point", which the loop
  guarantees. Confirm against the `async-runtime-core` scenario wording.
