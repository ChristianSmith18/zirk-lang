## Context

`fase-5-executor-core` and `fase-5-task-await` shipped a working `task` / `await`
surface on a stackful single-threaded cooperative executor. `ADR-017` fixed that
design. Design review (recorded in `docs/concurrency-model-draft.zrk`) rejected
the *surface* — `Task<T>` as a value, `await` as an operator, mandatory
single-consume, aggregation combinators — while keeping the *runtime*.

The new surface (changes #2–#5): `concurrent { }` and `parallel { }` blocks, a
`spawn` keyword, and methods on `Concurrent` / `Timer` / `Channel<T>` / `Thread`.
No `Task<T>`, no `await`, no `async`, no coloring.

This change removes the old surface only. It must not regress the runtime, the
context-switch matrix, or the collector's per-task roots — all of which the new
surface reuses unchanged.

## Goals / Non-Goals

**Goals**
- Delete `task` / `await` / `task scope` / `select` / `cancellation shield` from
  lexer, grammar, AST, sema, IR, codegen.
- Delete `Task<T>` / `Task.Final<T>` / `TaskSettlement<T>` / `Task.*` combinators.
- Rewrite `zirk-structured-concurrency` down to its model-neutral core.
- Leave `crates/zirk-runtime` functionally untouched.
- Emit a helpful "removed construct" diagnostic, not a syntax error, for old
  keywords — for one release cycle.

**Non-Goals**
- Building any part of the new surface (changes #2–#5).
- Touching the executor, timer service, context switch, or collector logic.
- Translating `ADR-001`–`ADR-016` to English (separate cleanup).

## Decisions

### D1: Keep the runtime, cut the surface

`executor.rs`, `task.rs`, `context.rs`, `timer.rs`, `collector.rs` stay. The
runtime's "task" is a scheduler concept (a stackful coroutine + control block),
not the language's `Task<T>`. Change #2 renames the C-ABI (`zirk_rt_task_spawn`
-> `zirk_rt_spawn`, etc.) when it wires the new surface; this change leaves the
symbols as-is to keep the diff reviewable.

**Alternative considered**: delete the runtime too and rebuild. Rejected — it is
correct, tested, and portable across the four target triples; the surface is the
only thing that was wrong.

### D2: Removed-construct diagnostic, not silent syntax error

`task`, `await`, `select` as leading tokens in an expression/statement position
produce `E_REMOVED_CONSTRUCT` naming the replacement (`concurrent { }` / `spawn` /
`Concurrent.of(...).first()`). `scope`, `shield` revert to ordinary identifiers.
The diagnostic ships for one release, then becomes an ordinary
undefined-name/parse error.

**Alternative**: hard parse error immediately. Rejected — a targeted message is
cheap and every existing `.zrk` snippet in the wild uses the old words.

### D3: `zirk-structured-concurrency` rewrite scope

Requirements **removed**: Task results have one consumer; Typed structured tasks;
Structured timeout; Task aggregation policies; Result and settlement separation;
Fair selection; Cooperative cancellation and shielding (the `shield` half);
Cancellation metadata (`Task<T>` half).

Requirements **kept, restated model-neutral** (a "concurrent operation" instead
of "a task"): Structured task failure -> *Structured failure propagation*;
cooperative cancellation (no shield); Supervised long-lived services; Derived
transfer and sharing; Safe task captures -> *Safe concurrent captures*; Parallel
CPU operations / reductions (change #3 owns the surface, requirement stays);
Scoped threads and blocking adapter; Structured synchronization; Safe atomics;
Safe-code data-race freedom; Single-threaded cooperative executor is the Phase 5
vehicle; A suspended operation's references stay reachable; Data-race analysis is
enforced before real parallelism exists.

Requirements **kept verbatim**: Typed channels and closure (change #4 refines).

### D4: Drop `fase-5-structured-tasks`

That active change layered `final task` + a shielded `await` on the old model.
Without `await` there is nothing to shield and no handle to mark `final`. Its
cancellation-cleanup concern is re-addressed model-neutrally in change #2
(`concurrent { }` cleanup edges + a non-cancellable region). Delete the change
directory; do not archive (it was never implemented).

### D5: ADR-017 addendum, not rewrite

Add a "Superseded surface decisions" section: D1's stackful/cooperative/per-task
choices stand; the "`task` / `await` / `task scope` nodes, no effect system"
consequence is superseded by change #2's ADR. The no-effect-system decision
survives (the new model is also colorless).

## Risks / Trade-offs

- **Intermediate broken state**: after this change, the language spec still
  discusses concurrency but no surface provides it. → Mitigation:
  `zirk-feature-phasing` already models "specified but not delivered"; the gap is
  one change wide (#2 lands the core).
- **Runtime C-ABI left with `task` names**: mildly confusing until #2 renames. →
  Mitigation: a doc-comment note in `runtime.rs` pointing at #2.
- **Fixture churn**: ~8 CLI corpus fixtures use `task` / `await`. → Mitigation:
  move the value ones to `invalid/` asserting the removed-construct diagnostic;
  delete the rest.
- **Website divergence**: the site imports the concurrency chapter. → Mitigation:
  the doc rewrite + `sync-website-content.sh` with a reviewed `--audit-date` is a
  task, gated in tasks.md before the change is called done.

## Migration Plan

1. Spec deltas + `zirk-structured-concurrency` rewrite; `openspec validate`.
2. Lexer/AST/parser: remove nodes + productions, add the diagnostic.
3. Sema: remove `Base::Task*`, `check_task`, `Task.*` resolution, single-consume.
4. IR/codegen: remove `IrType::Task`, `TaskStart`, `Await`.
5. Fixtures: port/delete.
6. Docs: `STRUCTURED_CONCURRENCY_SEMANTICS.md` rewrite, remove `08-std-task.md`,
   handbook chapter, `ADR-017` addendum, roadmap, feature status.
7. `cargo test --workspace` + fmt + clippy green.
8. Commit; run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`;
   review site status catalog; commit the site separately.
9. Delete `openspec/changes/fase-5-structured-tasks/`.

Rollback: revert the surface commits; the runtime is untouched so nothing else
regresses.

## Open Questions

- Does `CancellationReason` keep a `Custom(String)` variant now, or stay just
  `Cancelled` until change #2 needs more? Proposed: keep just `Cancelled` here,
  #2 extends.
- Keep `select` as a reserved word (for a future `select` over channels) or free
  it entirely? Proposed: free it; change #4 can re-reserve if channels want it.
