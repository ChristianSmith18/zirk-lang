## Why

Zirk's concurrency was specified around `task` / `await` / `Task<T>` — a
spawn-a-handle-then-await model with `task scope`, `select`, `cancellation
shield`, aggregation combinators (`Task.all` / `Task.first` / `Task.settled`),
and the `TaskSettlement<T>` enum. Design review concluded this surface is the
wrong fit for Zirk's stated goals: it reads like "promises without the good
parts", every result becomes a `Task<T>` value the programmer threads around,
and the everyday case (fan out a few calls, wait for all) needs the most
ceremony.

The replacement model (captured in `docs/concurrency-model-draft.zrk`) keeps the
runtime `fase-5-executor-core` and `fase-5-task-await` already built — the
single-threaded cooperative executor, stackful-coroutine suspension, the timer
service, and per-task garbage-collection roots — and puts a different **language
surface** on top: a `concurrent { }` block, a `parallel` block, a `spawn`
keyword, and a method API (`Concurrent.*`, `Timer.*`, `Channel<T>`, `Thread`).
There is no `Task<T>`, no `await`, no `async`, and no function coloring.

This change is the first of five. It **removes the old surface** from the grammar,
type system, IR, codegen, and the normative concurrency spec, so the four
follow-up changes build the new surface on a clean base. It deliberately keeps
every piece of runtime infrastructure.

## What Changes

- **BREAKING (pre-1.0 spec + implemented subset)**: the `task` and `await`
  keywords are removed from the lexer and grammar. `task expr`, `task { block }`,
  `task scope`, `await expr`, `await ... timeout`, `select`, and `cancellation
  shield` no longer parse; source using them gets a "removed construct"
  diagnostic that names the replacement.
- **BREAKING**: `Task<T>`, `Task.Final<T>`, `TaskSettlement<T>`, and the
  `Task.all` / `Task.first` / `Task.settled` / `Task.combine` static members are
  removed from the type system. `CancellationReason` stays (reused by the new
  cancellation model).
- **BREAKING**: the IR instructions `TaskStart` and `Await` and their codegen
  (`zirk_rt_task_spawn` / `zirk_rt_task_await` call sites) are removed. The
  `IrType::Task` one-word handle type is removed from lowering and codegen.
- The `zirk-structured-concurrency` capability is rewritten: its task/await,
  `task scope`, aggregation, selection, and shield requirements are removed; the
  requirements that survive conceptually (structured failure, cooperative
  cancellation, transfer/share safety, the single-threaded executor vehicle,
  suspended-task reachability) are restated in model-neutral terms so the
  follow-up changes attach the new surface to them.
- **Kept, untouched**: `crates/zirk-runtime/src/{executor.rs, task.rs,
  context.rs, timer.rs, collector.rs}`, the context-switch seam and its
  per-target matrix, per-task shadow-stack roots, and `zirk_rt_run_main`. The
  provisional C-ABI (`zirk_rt_task_*`) is retained as internal runtime surface
  and renamed in change #2.
- `docs/handbook/04-standard-library/08-std-task.md` is removed; the concurrency
  chapter's `task` / `await` sections are removed. `ADR-017` gets an addendum:
  its runtime decisions (stackful coroutines, cooperative executor, per-task
  roots) stand; its language-surface decisions (`task` / `await` / `task scope`
  nodes, no effect system) are superseded by the new-model ADRs in change #2.
- `zirk-feature-phasing` Phase 5 wording is updated: the delivered `task` /
  `await` forms move from "implemented subset" to "removed"; the new surface is
  listed as the Phase 5 deliverable.

## Capabilities

### New Capabilities

_None._ This change only removes and neutralizes existing requirements.

### Modified Capabilities

- `zirk-structured-concurrency`: remove all `task` / `await` / `task scope` /
  `select` / `cancellation shield` / aggregation requirements; restate the
  model-neutral survivors (structured failure, cooperative cancellation,
  transfer/share, single-threaded vehicle, suspended-task GC reachability).
- `zirk-grammar`: remove the task-creation, await-and-timeout, select, and
  cancellation-shield requirements; the "constructs outside the subset" list
  loses `task` / `await` / `select` and gains a removed-construct rule for them.
- `zirk-type-system`: remove "async core types are known", "`task` produces a
  typed handle", "a task result is consumed exactly once", "multiple observers
  do not implicitly clone", and "`Task.all` / `Task.first` / `Task.settled`
  typing". Keep "derived transfer and share" and "concurrency boundary and
  capture checking" (restated model-neutral).
- `zirk-ir-lowering`: remove "lowering of task creation and awaiting", "lowering
  of structured scopes with cleanup edges", "lowering of cancellation, shields,
  and timeouts", "lowering of select". Keep the cleanup-edge mechanism itself.
- `zirk-native-codegen`: remove `TaskStart` / `Await` codegen; keep "codegen for
  cooperative task suspension" (context switch), "context-switch support for
  every supported target", "per-task shadow-stack frame registration", and
  "executor lifecycle wraps the entrypoint".
- `zirk-errors`: keep `CancelledError` / `TimeoutError` as compiler-known
  catchable failures and "suppressed failures aggregate during structured
  cancellation"; remove the `Task<Result>` vs `Rejected` settlement-separation
  requirement and the `cancellation shield` wording.
- `zirk-lexical-syntax`: remove `Task` / `Await` from the keyword set; `select`,
  `scope`, `shield` become ordinary identifiers again.
- `zirk-feature-phasing`: Phase 5 lists the new surface; `task` / `await` are
  removed, not deferred.

## Impact

- **Code**: `crates/zirk-lexer` (keyword table + tests), `crates/zirk-ast`
  (drop `Expr::Task` / `Expr::Await` / task-scope nodes), `crates/zirk-parser`
  (drop the productions, add removed-construct diagnostics),
  `crates/zirk-sema` (drop `Base::Task` / `Base::TaskFinal`, the `task_types`
  tables, `check_task`, single-consume tracking, `Task.*` member resolution),
  `crates/zirk-ir` (`ir.rs` drop `IrType::Task` / `InstKind::TaskStart` /
  `InstKind::Await`; `lower.rs`; `verify.rs`), `crates/zirk-codegen-llvm`
  (`emit.rs` drop the two instructions; `runtime.rs` keep the symbols, they get
  reused/renamed in #2). Existing CLI fixtures under
  `crates/zirk-cli/tests/corpus` that use `task` / `await` are removed or ported
  to `invalid/`.
- **Runtime**: no functional change. `crates/zirk-runtime` is untouched except a
  doc comment pass.
- **Normative docs**: `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` (large rewrite
  to model-neutral survivors; the new surface is documented by changes #2–#5),
  `docs/ZIRK_LANGUAGE_SPEC.md` (section 10 concurrency), `docs/init/
  ZIRK_ROADMAP.md`, `docs/init/ZIRK_FEATURE_STATUS.md`, `ADR-017` addendum,
  `docs/handbook` concurrency chapter and `08-std-task.md`.
- **Companion repository `../zirk-lang-site`**: public concurrency docs,
  examples, and Phase 5 status change. After the zirk-lang commits land, run
  `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` and review the
  site-owned status catalog. Not complete while the site still describes
  `task` / `await`.
- **OpenSpec**: the active `fase-5-structured-tasks` change is dropped (its
  `final task` / shielded-await ideas do not survive without `await`). The
  archived `fase-5-async-core` / `fase-5-task-await` / `fase-5-executor-core`
  stay archived as historical record.
