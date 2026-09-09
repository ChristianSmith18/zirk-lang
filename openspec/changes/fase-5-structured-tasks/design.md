## Context

`fase-5-executor-core` built the single-threaded cooperative executor, the task
control block (`crates/zirk-runtime/src/task.rs`), the stackful-coroutine seam
(`context.rs`), the timer service, and per-task garbage-collection roots.
`fase-5-task-await` wired the bare `task` / `await` language surface onto that
runtime: `task expression`, `task { block }`, `await expression`, the `Task<T>`
type, single-consume tracking, and GC-rooted captures, all end to end from lexer
to CLI fixture.

What is still missing is everything that makes a *group* of tasks structured:

- A child that throws today just fails its own task; its running siblings are not
  cancelled and the parent is not notified in any defined way.
- `handle.cancel()` is specified but not delivered — there is no `CancelledError`,
  no safe-point delivery, no parent-to-child propagation.
- There is no protected-cleanup mechanism, so any `finally` that performs an
  `await` during unwinding would re-raise cancellation and abort partway.
- There is no ergonomic join: the normative specs answer this with a `task scope`
  keyword and homogeneous `Task.all`, but no heterogeneous combinator.

The task control block already carries the fields this change needs:
`cancel_requested`, `cancel_reason`, `shield_depth`, `cleanup_state`, `waiter`,
and `wait: WaitReason`. `shield_depth` in particular was added by executor-core
in anticipation of shielded regions and is currently always zero.

Constraints: the executor and the garbage collector both stay single-threaded
(roadmap Phase 5 steps 1 to 3). No task is preempted between safe points. Every
OpenSpec artifact and every repository document is written in English (ADR-006);
the companion site `../zirk-lang-site` must be resynced because public
concurrency documentation and Phase 5 status change.

## Goals / Non-Goals

**Goals:**

- Deliver sibling-failure propagation, cooperative cancellation with
  `CancelledError`, and parent-to-child cancellation on the single-threaded
  executor.
- Introduce `final task` / `final task { block }` as the single primitive for
  "this task cannot be cancelled" and, via the shielded-await rule, for
  "protect this cleanup".
- Introduce `Task.combine` (heterogeneous fail-fast join) and finish
  `Task.all` / `Task.settled` / `TaskSettlement<T>`.
- Introduce the `Task.sleep` / `Task.after` / `Task.every` timer surface on the
  `Task` namespace, backed by the existing timer service.
- Remove the `task scope` keyword and the `cancellation shield` statement from
  the language and the normative specs, preserving every capability they carried.
- Ship one runnable `examples/` file that exercises and validates the whole
  slice.

**Non-Goals:**

- `select`, `await ... timeout`, the `Channel<T>` family, and `Task.first` —
  next slice (`fase-5-select-and-channels`). Note `Task.sleep` / `Task.after` /
  `Task.every` DO land here; only the `await ... timeout` operator is deferred.
- The fixed-rate, missed-tick-reporting ticker resource in the temporal family.
- `parallel`, `thread`, `task.blocking`, `Mutex<T>` / `RwLock<T>` / `Semaphore` /
  `Barrier` / `Once<T>`, and `Atomic<T>` — Phase 5 steps 4 to 6, and a
  multi-threaded collector.
- The `application` root supervisor / `spawn_service` surface — stays specified,
  not implemented here.
- Making the executor or collector multi-threaded.
- Watch / broadcast / one-shot channel families.

## Decisions

### D1: Remove the `task scope` keyword; the function body is the only explicit scope

Every executing function already "belongs to a task scope"
(`zirk-structured-concurrency`, *Typed structured tasks*): its body will not
return while it has unfinished children, and it propagates the first child
failure. `task scope { ... }` only added a *nested* boundary and evaluated to the
block's value directly.

The nested boundary is expressible without a keyword:

```zirk
// was: mut prep = task scope { mut a = task a(); mut b = task b(); return (await a, await b); };
mut ha = task a();
mut hb = task b();
mut prep = await Task.combine(ha, hb);   // fail-fast join, same failure semantics
```

The one case that loses direct sugar is unbounded fan-out without keeping
handles:

```zirk
// was:
task scope { for u in urls { _ = task fetch(u); } }
// now:
mut handles = urls.map(u => task fetch(u));
await Task.all(handles);
```

This is more explicit and arguably clearer; it is the documented migration.

**Alternatives considered:** (a) Keep `task scope` as pure sugar for
`await Task.combine(...)` — rejected: two syntaxes for one concept, and the
block form hides that the members are already-running tasks. (b) Keep `task
scope` only for the fan-out case — rejected: a keyword earning its keep in one
narrow pattern is worse than a `.map` + `Task.all`.

The implicit-scope guarantee itself is untouched — it is simply re-anchored on
"a function body or a `task` block" instead of "a function body, a `task` block,
or a `task scope` block" in `zirk-structured-concurrency` and `zirk-grammar`.

### D2: `final task` is a call-site modifier, not a return-type property

`final` sits in front of `task` exactly where `task` sits today:
`final task f()` and `final task { ... }`. It never appears in a function
signature. `f` stays `fn f() -> T`; the *caller* decides the task is
uncancelable. This is the decisive difference from an earlier sketch where
`persist_commit()` would return `Task.Final<T>` — that would reintroduce
function coloring, which `zirk-structured-concurrency` §1 and ADR-017 explicitly
reject.

The result type is `Task.Final<T>`, a distinct known generic (new
`Base::TaskFinal(u32)` and a `task_final_types` table, mirroring
`Base::Task(u32)`). It is distinct rather than a flag on `Task<T>` because the
checker must decide *statically* whether an `await` is shielded (D4) and whether
`.cancel()` is legal (D3).

`Task.Final<T>` lowers to the same one-word `IrType::Task` / LLVM `i64` handle as
`Task<T>`; the distinction is erased after checking except for the two IR flags
in D5.

**Alternatives considered:** a `bool` field on `Task<T>` — rejected, not
statically decidable at the await; a `final` block statement (`final { ... }`)
detached from `task` — rejected, it would need its own capture and lowering path
instead of reusing `TaskStart`.

### D3: `handle.cancel()` on a `Task.Final<T>` is a compile-time error

A `Task.Final<T>` promises to run to completion. `.cancel()` on it is rejected in
the checker (new diagnostic `CANCEL_ON_FINAL_TASK`) with a note pointing at the
`final task` site. A no-op would be a silent trap.

Parent and sibling cancellation *sweeps* also skip `Task.Final<T>` tasks (D6);
they are runtime-identified by the `uncancelable` TCB marker, not by the static
type.

### D4: Awaiting a `Task.Final<T>` is a shielded await

`await h` where `h: Task.Final<T>` produces exactly `T` and, additionally,
**defers delivery of the current task's own pending cancellation** until that
await returns. Concretely: the compiler emits the shielded await ABI variant
(D5), which raises `shield_depth` on the current task for the duration of the
wait; while `shield_depth > 0` the safe-point check does not throw
`CancelledError`. When the awaited final task completes and the await returns,
`shield_depth` drops and the pending `CancelledError` is delivered at the next
safe point.

This is the entire replacement for `cancellation shield { ... }`:

```zirk
fn transfer(conn: Conn, from_id: Int, to_id: Int, amount: Money) {
    mut tx = conn.begin();
    try {
        await conn.execute("... balance - ? WHERE id = ?", amount, from_id);
        await conn.execute("... balance + ? WHERE id = ?", amount, to_id);
        await tx.commit();
    } finally {
        await final task {          // shielded: the ROLLBACK reaches the server
            tx.rollback_if_open();
        };
    }
}
```

Multi-step cleanup goes inside the one `final task { }` block. Non-`await`
cleanup does not need a shield at all (nothing interrupts it). A bare
`final task f();` that is never awaited is the fire-and-forget uncancelable case
and needs no shielding.

**Alternatives considered:** (a) keep `cancellation shield` as a statement —
rejected, a second keyword for a behavior `final task` already implies. (b) an
`await shield expr` modifier that shields *any* await — rejected, it does not
also give you an uncancelable task, so we would still need a second concept for
that; `final task` gives both. (c) `asyncio.shield` semantics where the caller
still gets `CancelledError` and the inner runs detached — rejected, that is the
known-bad behavior that orphans the cleanup result.

### D5: IR — `TaskStart { uncancelable }` and `Await { shielded }`

`InstKind::TaskStart` gains `uncancelable: bool`; `InstKind::Await` gains
`shielded: bool`. The checker sets `uncancelable` from `final task` and
`shielded` from "the awaited operand's static type is `Task.Final<T>`". `verify.rs`
gains no new type rules beyond carrying the flags; `IrType` is unchanged
(`Task.Final<T>` is already `IrType::Task`). Golden IR tests assert the flags at
the `final task` and `await final` sites.

**Alternative considered:** separate `FinalTaskStart` / `ShieldedAwait`
instructions — rejected, they duplicate every existing match arm for a boolean.

### D6: Runtime — one `uncancelable` marker, sweeps skip it, shielded await gates the safe point

`TaskControlBlock` gains `pub uncancelable: bool` (default `false`). Changes in
`crates/zirk-runtime`:

- **`task.rs`**: the field, plus a spawn entry point that sets it.
- **`executor.rs`**:
  - `request_cancel(task, reason)`: idempotent; sets `cancel_requested` /
    `cancel_reason`; if the task is suspended at a cancellable `WaitReason`, make
    it ready so it observes the cancel at its safe point. On an `uncancelable`
    task it is a no-op for the sweeps and a panic/abort for a direct misuse that
    slipped past the checker.
  - Sibling-failure sweep: on a child's unhandled exception, walk the parent
    scope's child list, `request_cancel` every non-terminal, non-`uncancelable`
    sibling, move to a "draining" state that waits for each child's
    `cleanup_state == Done`, then resolve the parent with the primary exception
    and the suppressed list.
  - Parent-to-child propagation: `request_cancel` on a scope recurses into its
    children.
  - Safe-point check (called from `await_task`, `yield_now`, and the
    sleep/timer path): if `cancel_requested && shield_depth == 0 && !uncancelable`,
    throw `CancelledError` carrying `cancel_reason` instead of returning the
    value.
- **`context.rs`**: shielded await raises `shield_depth` before `suspend_current`
  and lowers it on resume (balanced, panic-safe via a guard).
- **new `aggregate.rs`**: `Task.all`, `Task.combine`, `Task.settled` as executor
  operations over a set of `TaskId`s — register the caller as the shared waiter,
  resume on each completion, apply the per-policy cancellation of the rest.
- **`collector.rs`**: unchanged in mechanism; the aggregation waiter's pending
  result list is a new root the mark phase must walk (a `Vec<Handle>` anchored in
  the calling task's frame, so the existing per-task chain already covers it —
  verify with a fixture).

`CancelledError` is a runtime-constructed throwable of a compiler-known type
(`zirk-errors`), catchable like any exception, and it participates in the
suppressed-exception aggregation.

### D7: `Task.combine` is fixed-arity, heterogeneous, tuple-returning

`Task.combine(a, b)` … `Task.combine(a, b, c, d, e)` — a small fixed family
(2..=8, say), each returning the corresponding tuple type. It is fail-fast:
identical failure semantics to `Task.all`, but over a heterogeneous set. Members
that are `Task.Final<T>` are not cancelled by a sibling failure; the combine
waits for them before propagating.

**Alternatives considered:** (a) require `Task.all` over a `List<Task<T>>` only —
rejected, forces a common type or an existential and loses the tuple result that
makes the `task scope` migration clean. (b) true variadic generics — rejected,
the type system has no variadic generic support and adding it is out of scope; a
fixed arity family is the pragmatic Zirk-1.x answer, matching how tuple arity is
already bounded elsewhere.

### D8: New ADR-018 records the model

`docs/decisions/ADR-018-uncancelable-tasks-and-shielded-await.md`: why `final
task` unifies "uncancelable spawn" and "protected cleanup", why it is a call-site
modifier and not a type annotation (no coloring), the shielded-await rule, and
the boundedness obligation. It is an addendum in spirit to ADR-017 (suspension
model).

### D9: Timer primitives are static members of `Task`

`Task.sleep(d: Duration): Task<Void>`, `Task.after<T>(d: Duration, body: (): T):
Task<T>`, and `Task.every(d: Duration, body: (): Void): Task<Void>` join
`Task.all` / `Task.combine` on the `Task` namespace. All three return `Task<T>`
so they are awaitable through the one `await` rule — Zirk has no async-function
coloring, so a suspension can only be `await` over a `Task`-typed expression, and
a bare suspending `sleep` statement would need a special typing carve-out. They
are backed by the timer service `fase-5-executor-core` already built
(`crates/zirk-runtime/src/timer.rs`); a negative duration is the controlled error
that service already defines.

Semantics:

- `Task.sleep(d)` suspends the awaiting task for `d`. The timer arms at creation
  (consistent with "task creation starts the child immediately"). It is a
  cancellation safe point: `.cancel()` on the handle, or the awaiting task being
  cancelled, makes the `await` raise `CancelledError`. The runtime represents it
  without a coroutine stack — a timer registration plus a waiter slot.
- `Task.after(d, body)` is equivalent to `task { await Task.sleep(d); return
  body(); }` as a primitive: an ordinary scope-owned child. You need not await it
  (the timer drives it), but the scope owns it — cancelled or joined at scope
  exit, never orphaned. `body` follows the ordinary `task`-body capture rules.
- `Task.every(d, body)` runs `body` every `d` (fixed-delay: the next tick arms
  after `body` returns) until cancelled; the `Task<Void>` completes only through
  cancellation. It is long-lived work and MUST be scope-owned. Wrapping it in
  `final task` is a compile-time error (an uncancelable infinite interval is a
  guaranteed stuck task under D2's boundedness rule). The fixed-rate,
  missed-tick-reporting ticker stays the lower-level temporal-family resource
  (`zirk-temporal-types`, *Timer resources report scheduling behavior*) and is
  not delivered here.

**Alternatives considered:** (a) a dedicated `Timer` or `Clock` type — rejected,
they return `Task<T>` and belong with the other `Task` factories; `Clock` stays
the injectable time *source* (`Clock.system` / `Clock.monotonic` / virtual test
clocks), and `Task.sleep` consults whatever clock the executor is running on, so
a virtual clock still drives it in tests. (b) a bare `sleep(d);` suspending
statement (Kotlin `delay()`) — rejected, it hides the suspension point that
Zirk's explicit `await` is meant to mark and needs a typing carve-out. (c) JS
`setTimeout` names (`set_timeout` / `interval`) — `after` / `every` are shorter,
read in place (`Task.after(1s, ...)`), and `after` already appears in `select`;
`timeout` would also collide with `await ... timeout`.

## Risks / Trade-offs

- **An unbounded `final task` stalls its parent's cancellation forever.**
  A `final task { await something_that_never_completes(); }` cannot be cancelled,
  so a parent that is cancelled while awaiting it is stuck. → Mitigation: a
  best-effort checker lint (`FINAL_TASK_UNBOUNDED_WAIT`) when a `final task` body
  contains an `await` with no reachable completion and no timeout; the runtime's
  existing unresolvable-wait detector (empty ready queue + no armed timer + a
  suspended task) is the backstop and aborts with a diagnostic. Document that
  `final task` bodies must be bounded, same obligation `cancellation shield`
  carried.

- **`final task` is a weaker visual danger signal than `cancellation shield`.**
  It reads like an ordinary task. → Mitigation: the handbook and ADR-018 call out
  the pattern explicitly; the lint above fires on the most common misuse.

- **`final task` cannot be used where spawning is forbidden.** A reversible
  `unsafe` transaction "cannot await, spawn, …" (`zirk-structured-concurrency`,
  memory-and-unsafe §20). So protected cleanup is unavailable inside a reversible
  transaction. → Mitigation: cleanup for such a transaction already belongs
  outside it or in its explicit irreversible `commit` region, where `final task`
  is allowed. Document; add a targeted diagnostic if `final task` appears in a
  reversible-transaction body.

- **Removing `task scope` is a normative-surface break.** → Mitigation: the specs
  are pre-1.0, no released compiler ever parsed `task scope` (it only ever
  emitted a "not implemented" diagnostic), so there is zero real migration. The
  proposal and handbook show the `Task.combine` / `Task.all` replacement.

- **Fixed-arity `Task.combine` has an arity ceiling.** → Mitigation: 8 covers
  every realistic heterogeneous join; beyond that, `Task.all` over a list is the
  right tool anyway.

- **Cancellation correctness is subtle on a cooperative executor.** Draining
  order, suppressed-exception aggregation, re-entrancy of `request_cancel`. →
  Mitigation: the single-threaded executor makes every interleaving
  deterministic and testable; CLI fixtures pin the observable ordering, runtime
  unit tests pin the internal state machine.

## Migration Plan

1. Land the runtime (`task.rs`, `executor.rs`, `context.rs`, `aggregate.rs`,
   the `Task.sleep` / `after` / `every` timer-service bindings) with its own unit
   tests — no language surface yet.
2. Land AST + parser (`final task`, remove `task scope` production and the two
   diagnostics) behind the existing phase-diagnostic gate for the still-deferred
   forms.
3. Land sema (`Task.Final<T>`, shielded-await typing, `.cancel()` rejection,
   `TaskSettlement<T>`, the `Task.*` combinator signatures, the `Task.sleep` /
   `after` / `every` signatures, cancellation linearity).
4. Land IR + codegen (the two flags, the ABI calls).
5. Land the CLI fixtures and `examples/structured_tasks_examples.zrk`.
6. Update `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`, ADR-018, roadmap, feature
   status, handbook.
7. Commit, then resync `../zirk-lang-site` with `./scripts/sync-website-content.sh
   --audit-date YYYY-MM-DD` and review the site-owned status catalog.

Rollback: the change is additive on the runtime side and gated by the phase
diagnostic on the surface side; reverting the sema + parser commits restores the
"`task scope` / cancellation not implemented" state without touching
executor-core or task-await.

## Open Questions

- **`Task.combine` arity ceiling** — 8 is proposed; confirm against tuple arity
  limits elsewhere in the type system.
- **`Task.first`** — deferred to `fase-5-select-and-channels` (its "race" nature
  fits `select`), or pulled forward here for symmetry? Proposed: defer.
- **`cancel()` reason type** — `CancellationReason` enum shape: is
  `Cancelled` the only built-in variant in this slice, with user reasons added
  later, or is it open from the start? Proposed: `Cancelled` plus a
  `Custom(String)` variant now.
- **Shielded await and a *second* cancellation** (a hard deadline while
  `shield_depth > 0`) — deliver after a grace period, or honor the shield
  absolutely until the final task returns? Proposed: honor absolutely in this
  slice (no hard-deadline source exists yet); revisit when `timeout` lands.
