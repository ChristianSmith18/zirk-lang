# ADR-017 — Task suspension model: stackful coroutines on a single-threaded cooperative executor

- **Status:** accepted
- **Date:** September 8, 2026
- **Phase:** 5 (steps 1–3)

## Context

`ZIRK_ROADMAP.md` Phase 5 delivers `task` / `await` / `task scope`,
cancellation, aggregation, `select`, and `Channel<T>` in six sub-steps. Steps 1–3
are the whole cooperative model on "a custom single-threaded executor"; steps 4–6
add real parallelism (`parallel`, `thread`, `Atomic<T>`, the synchronizer
library). The `fase-5-async-core` change implements steps 1–3.

Two decisions cascade into every pipeline stage and must be fixed before any code
is written, exactly as `ADR-003` fixed the collector strategy before Phase 4e:

1. **How does `await` suspend a task?** This determines the calling convention of
   generated code, whether the checker needs an effect system, whether the IR
   grows a frame type, and whether codegen emits one ABI or two.
2. **How does the garbage collector find roots inside a suspended task?**
   `ADR-003` closed on a function-granularity shadow stack with a single
   process-global chain and cooperative, allocation-triggered collection, and
   explicitly deferred the concurrency interaction: *"at that point, triggering
   and 'stop the world' need to be revisited, and this is noted as work for that
   phase"*.

`STRUCTURED_CONCURRENCY_SEMANTICS.md` §1 constrains decision 1 up front:

> There is no `async fn`. A function describes its logical input/output contract;
> `task` explicitly decides to execute a call concurrently.

So a plain function call must look and behave the same whether or not the callee
awaits internally. That rules out the model where suspendability is part of a
function's type.

## Decision

### D1 — Stackful coroutines, single-threaded cooperative executor, no function coloring

Each `task` owns a **heap-allocated contiguous stack** and a **task control
block** (TCB). Starting a task allocates both and enqueues the task as ready.
`await`, a suspending channel operation, `select`, and a timer wait perform a
**cooperative context switch**: the runtime saves the running task's
callee-saved registers, stack pointer, and resume address into its TCB, restores
the executor's own context, and returns into the executor loop. Resuming a task
is the reverse switch.

A plain function call is an ordinary native `call` with the platform's ordinary
calling convention. A function that internally does `task x(); await x;` runs on
whatever task's stack is current; its activation frame is part of that stack and
is suspended and resumed **wholesale with the stack**. The compiler does **not**
transform function bodies into resumable state machines, does **not** introduce a
second calling convention for suspendable code, and the checker needs **no**
effect/color system for `await`.

The executor is **one operating-system thread**. Scheduling is cooperative: a
task yields only at a defined safe point and is never preempted mid-instruction.
This keeps `ADR-003`'s "cooperative triggering inside `zirk_rt_alloc`, without
threads" correct as-is for steps 1–3 — a collection can still only begin while
exactly one task is running and every other task sits at a safe point with a
consistent, fully-pushed shadow-stack frame. No stop-the-world machinery is
introduced now; it becomes real work when steps 4–6 add a second thread.

`ADR-003`'s constraint that the collector strategy "must be thread-safe by
design, not adapted afterward" is honored at the level of the *strategy*:
non-moving mark-sweep with an intrusive allocation list and per-frame roots
parallelizes later without a redesign. The single-threaded *executor* is a
step-1–3 scope decision, not a collector decision.

### D2 — Per-task GC roots: one shadow-stack chain per task

`ADR-003`'s function-granularity shadow stack is kept unchanged in shape. The
only change is ownership of its **head**: it moves from one process-global
pointer in `collector.rs` into the TCB (`tcb.shadow_stack_head`). The executor
sets a `CURRENT_TASK` runtime global on every resume; `zirk_rt_push_frame` /
`zirk_rt_pop_frame` operate on `CURRENT_TASK.shadow_stack_head`. `main` runs as
task 0 and its chain behaves exactly as today's global chain.

Root enumeration in `collect()` changes from "walk the one chain" to "for every
task the executor still owns — ready, running, suspended, or running structured
cleanup — walk its chain". The executor exposes an iterator over live TCBs to the
collector; `crate::clone::mark_clone_roots` remains a separate extra root set,
walked as before.

`ADR-003`'s SSA-spill rule is what makes a suspended frame safe: *"every managed
reference-type value is spilled to its own synthetic slot as soon as it is
produced"*. A task that suspends between two call arguments has already spilled
the first argument's reference to a slot that its own chain roots, so the
collector sees it from the suspended chain just as it would from the running one.

The object header is **unchanged** — three words per `ADR-012`. No per-object
task tag. The channel runtime object gets a custom trace hook (queued reference
values are roots), which is an ordinary collector extension, not a header change.

### D3 — Context switch is per-target and isolated behind one seam

The switch is `fn switch(from: *mut Context, to: *const Context)` in
`crates/zirk-runtime/src/context.rs`, with one implementation per target triple
`ADR-004` supports (macOS aarch64, Linux x86_64, Linux aarch64, Windows x86_64)
plus a host-only Rust fallback used under `cargo test` on an unsupported host.
Whether each implementation is hand-written assembly or a vetted crate
(`corosensei`) is left to the implementation and recorded in the change's
`design.md`; the seam is identical either way. `ADR-004` carries an addendum for
this.

## Discarded alternatives

- **Stackless state-machine transformation (LLVM coroutine intrinsics or a
  hand-rolled IR pass).** Every function that can reach an `await` —
  transitively, because a callee's suspension must suspend its caller's frame —
  would be rewritten into a resumable state machine with a heap frame and a
  second entry ABI. This is the "what color is your function" problem, and it
  directly contradicts `STRUCTURED_CONCURRENCY_SEMANTICS.md` §1. It also forces
  an effect system into the checker, a frame type into the IR, and two ABIs into
  codegen. The one thing it buys — precise cheap frames with no reserved stack —
  is not worth reworking the whole compiler for a single-threaded executor.
  Left as a possible future direction only if fixed-size task stacks become a
  measured problem.
- **Segmented / growable stacks (Go's early model).** Avoids over-reserving
  memory per task but adds "hot split" costs and complicates both the switch
  shim and `extern "C"` calls, which need a contiguous stack. Deferred; a
  default-size contiguous stack is adequate for step-1–3 workloads (tens to
  hundreds of tasks) and a guard-page or growth scheme can be added later without
  touching the language surface.
- **One OS thread per task (1:1, or virtual threads over a thread pool).**
  `task` is explicitly *not* `thread` (`STRUCTURED_CONCURRENCY_SEMANTICS.md` §1,
  §16). 1:1 threads would drag the collector into multi-threading immediately —
  the exact work steps 4–6 are scheduled to do deliberately — and give up the
  cheap, deterministic scheduling the cooperative model provides.
- **Conservative scanning of suspended task stacks (no shadow stack for tasks).**
  Would avoid per-task chains entirely, but `ADR-003` already rejected
  conservative scanning for the main stack over precision and future-moving
  concerns. Keeping one root-enumeration strategy across the whole runtime is
  worth the small generalization to per-task heads.
- **Copying each task's roots into a side table at suspension.** Extra work on
  the hot suspend path and a second source of truth for roots. The intrusive
  per-TCB chain is already the natural representation.

## Consequences

- `crates/zirk-runtime` gains `context.rs`, `executor.rs`, `task.rs`, `timer.rs`,
  and `channel.rs`. `collector.rs`'s single global shadow-stack head becomes
  per-TCB and `collect()` walks every live task's chain. `lib.rs`'s
  entry wrapper runs `main` as the executor's root task.
- `zirk-codegen-llvm` emits `zirk_rt_push_frame` / `pop_frame` against
  `CURRENT_TASK` and emits each suspension point as a runtime call plus a resume
  label. The ordinary function calling convention is untouched.
- `zirk-sema`, `zirk-ir`, and `zirk-parser` gain `task` / `await` / `task scope`
  / `select` / `cancellation shield` nodes and lowering, but **no** effect
  system and **no** function-body transformation.
- `ADR-003`'s deferred concurrency note is partially discharged: for a
  single-threaded cooperative executor, "revisiting triggering and stop-the-world"
  resolves to "generalize the root chain to one per task; nothing else changes".
  The full stop-the-world question returns with steps 4–6.
- `ADR-004` gains the context-switch addendum and its verification matrix gains a
  per-triple switch round-trip test.
- Fixed-size task stacks are a known limitation; a `StackOverflowError` from a
  cheap software frame check, and a later guard-page implementation, are tracked
  by the `fase-5-async-core` change, not by this ADR.

## Related

- [ADR-003](./ADR-003-memoria.md) — the shadow stack and cooperative collection
  this ADR generalizes.
- [ADR-004](./ADR-004-portabilidad.md) — the target triples the switch shim must
  cover.
- [ADR-012](./ADR-012-layout-de-objetos.md) — the object header this ADR leaves
  unchanged.
- `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` — the normative behavior being
  implemented.
