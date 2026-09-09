## 0. Prerequisites

- [ ] 0.1 Confirm `export LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20` is set for every `cargo build` / `cargo test` in this change
- [ ] 0.2 Re-read `crates/zirk-runtime/src/{task.rs,executor.rs,context.rs}` and confirm the TCB fields this change reuses exist: `cancel_requested`, `cancel_reason`, `shield_depth`, `cleanup_state`, `waiter`, `wait`
- [ ] 0.3 Write `docs/decisions/ADR-018-uncancelable-tasks-and-shielded-await.md`: why `final task` unifies "uncancelable spawn" and "protected cleanup", why it is a call-site modifier and not a return-type annotation (no function coloring), the shielded-await rule, and the bounded-body obligation

## 1. Runtime: cancellation and uncancelable tasks (`crates/zirk-runtime`)

- [ ] 1.1 `task.rs`: add `pub uncancelable: bool` to `TaskControlBlock` (default `false`); add a spawn path that sets it
- [ ] 1.2 `executor.rs`: `request_cancel(task, reason)` — idempotent; set `cancel_requested` / `cancel_reason`; if the target is suspended at a cancellable `WaitReason`, make it ready; no-op on an `uncancelable` target for sweeps
- [ ] 1.3 `executor.rs`: safe-point cancellation check reachable from `await_task`, `yield_now`, and the sleep/timer path — raise `CancelledError` only when `cancel_requested && shield_depth == 0 && !uncancelable`
- [ ] 1.4 `executor.rs`: parent-to-child cancellation propagation — `request_cancel` on a scope recurses into its live children, skipping uncancelable ones
- [ ] 1.5 `executor.rs`: sibling-failure sweep — on a child's unhandled exception, cancel non-terminal non-uncancelable siblings, wait for each `cleanup_state == Done`, resolve the parent with the primary exception and a suppressed list
- [ ] 1.6 `context.rs`: shielded-await primitive — raise `shield_depth` before `suspend_current`, lower it on resume/unwind via a panic-safe guard
- [ ] 1.7 `task.rs` / `exceptions.rs`: `CancelledError` as a runtime-constructed compiler-known throwable carrying the reason; participates in suppressed-exception aggregation
- [ ] 1.8 Runtime unit tests: idempotent cancel, sweep skips uncancelable, shielded await holds then delivers, parent→child propagation, suppressed aggregation ordering

## 2. Runtime: aggregation (`crates/zirk-runtime/src/aggregate.rs`)

- [ ] 2.1 New `aggregate.rs`: `Task.all` over a `TaskId` set — register the caller as shared waiter, resume on each completion, preserve input order, fail-fast cancel-the-rest (skip uncancelable), suppressed list
- [ ] 2.2 `Task.combine` — fixed arity 2..=8, heterogeneous, tuple result, same fail-fast policy
- [ ] 2.3 `Task.settled` — every member finishes, input order preserved, `TaskSettlement` per member, no sibling cancellation
- [ ] 2.4 `collector.rs`: verify the aggregation waiter's pending-result list is walked by the existing per-task root chain; add a fixture if a new root is needed
- [ ] 2.5 Runtime unit tests for 2.1–2.3 including an uncancelable member in a failing `combine`
- [ ] 2.6 Timer surface bindings over `timer.rs`: `Task.sleep` (suspend current task on one deadline, cancellable safe point), `Task.after` (scope-owned child that runs its body at the deadline), `Task.every` (fixed-delay re-arm until cancelled); negative duration -> controlled error before waiting
- [ ] 2.7 Runtime unit tests: sleep yields to a ready task then resumes, cancel-during-sleep, `after` fires once and is scope-cancellable, `every` re-arms and stops on cancel

## 3. Runtime ABI surface (`crates/zirk-runtime` extern "C")

- [ ] 3.1 Add provisional `extern "C"` entry points: uncancelable spawn, shielded await, `zirk_rt_task_cancel`, the `all` / `combine` / `settled` joins, and `sleep` / `after` / `every`
- [ ] 3.2 Integration test exercising the C ABI surface end to end from Rust

## 4. AST + parser (`crates/zirk-ast`, `crates/zirk-parser`, `crates/zirk-lexer`)

- [ ] 4.1 `zirk-lexer`: confirm `final` is available in the `final task` position; `scope` / `shield` / `cancellation` stay ordinary identifiers
- [ ] 4.2 `zirk-ast`: add a `final: bool` flag to the task expression node
- [ ] 4.3 `zirk-parser`: parse `final task expression` and `final task { block }` as a task node with `final = true`
- [ ] 4.4 `zirk-parser`: remove the `task scope` production; `task` followed by identifier `scope` -> removed-construct diagnostic naming `Task.combine` / `Task.all`
- [ ] 4.5 `zirk-parser`: remove the `cancellation shield` production and diagnostic; `cancellation shield` -> removed-construct diagnostic naming `await final task { ... }`
- [ ] 4.6 `zirk-parser`: keep `select`, `after`, `cancelled`, `await ... timeout` on their existing deferred-form diagnostics
- [ ] 4.7 Parser tests: `final task f()`, `await final task { ... }`, `mut h: Task.Final<Void> = final task c();`, both removed-construct diagnostics, `final` still rejected on fields/params/vars

## 5. Type system (`crates/zirk-sema`)

- [ ] 5.1 `types.rs`: `Base::TaskFinal(u32)` + `task_final_types` table; `resolve_type_atom` recognizes `Task.Final<T>`; prints as `Task.Final<...>`; removed from `pending_type`
- [ ] 5.2 `types.rs`: `CancellationReason` known enum with `Cancelled` (default) and `Custom(String)`
- [ ] 5.3 `checker.rs`: `final task expr` -> `Task.Final<T>`; `final task { block }` -> `Task.Final<T>` over the block result; reuse the `begin_capture_scope` / `finish_capture_scope` path
- [ ] 5.4 `checker.rs`: `await h` on `Task.Final<T>` -> `T`; mark the checked await node `shielded`
- [ ] 5.5 `checker.rs`: reject `.cancel()` on a `Task.Final<T>` -> `CANCEL_ON_FINAL_TASK` with a note at the `final task` site
- [ ] 5.6 `checker.rs`: single-consume tracking applies to `Task.Final<T>` identically to `Task<T>`
- [ ] 5.7 `checker.rs`: `Task.combine(a..h)` signatures (arity 2..=8), heterogeneous, tuple result; arity > 8 -> diagnostic pointing at `Task.all`
- [ ] 5.8 `checker.rs`: finish `Task.all` / `Task.settled` / `TaskSettlement<T>` typing; `TaskSettlement<T>` resolves as an ordinary enum and is exhaustively matchable
- [ ] 5.8b `checker.rs`: `Task.sleep(Duration): Task<Void>`, `Task.after(Duration, (): T): Task<T>`, `Task.every(Duration, (): Void): Task<Void>`; body under `task`-body capture rules; non-`Duration` delay -> diagnostic; `final task` wrapping a `Task.every` -> diagnostic
- [ ] 5.9 `checker.rs`: sibling-failure and cancellation are sema-visible where needed (must-use of a `Task<T>`, `_ =` discharge unchanged)
- [ ] 5.10 `checker.rs`: best-effort `FINAL_TASK_UNBOUNDED_WAIT` lint — `final task` body awaits with no reachable completion and no timeout
- [ ] 5.11 `checker.rs`: reject `final task` inside a reversible unsafe transaction body (spawning already forbidden there)
- [ ] 5.12 New diagnostic codes in `zirk-sema`'s `codes` module: `CANCEL_ON_FINAL_TASK`, `FINAL_TASK_UNBOUNDED_WAIT`, `REMOVED_TASK_SCOPE`, `REMOVED_CANCELLATION_SHIELD`
- [ ] 5.13 Checker tests in `crates/zirk-sema/tests/typing.rs`: `Task.Final<T>` typing, shielded await marker, cancel-on-final rejection, `Task.combine` tuple + arity ceiling, `Task.settled` type, removed-construct diagnostics

## 6. IR + lowering (`crates/zirk-ir`)

- [ ] 6.1 `ir.rs`: `InstKind::TaskStart` gains `uncancelable: bool`; `InstKind::Await` gains `shielded: bool`; `IrType` unchanged (`Task.Final<T>` is `IrType::Task`)
- [ ] 6.2 `lower.rs`: set `uncancelable` from the AST `final` flag; set `shielded` from the checked await node
- [ ] 6.3 `lower.rs`: structured-scope enter/exit cleanup edges anchored on a function body or a `task` block only (no `task scope`)
- [ ] 6.4 `lower.rs`: lower `Task.all` / `Task.combine` / `Task.settled` calls to the aggregation runtime calls, and `Task.sleep` / `Task.after` / `Task.every` to the timer runtime calls (`after` / `every` lower the body as a boxed callable like a `task` body)
- [ ] 6.5 `verify.rs`: carry the two flags; every exhaustive `match` over `IrType` and `InstKind` handled
- [ ] 6.6 IR lowering tests: `final task` sets `uncancelable`, `await final` sets `shielded`, `Task.combine` shape, scope edges present with a child / absent without

## 7. Native codegen (`crates/zirk-codegen-llvm`)

- [ ] 7.1 `runtime.rs`: declare the new ABI entry points + `symbols` constants (uncancelable spawn, shielded await, cancel, all/combine/settled)
- [ ] 7.2 `emit.rs` `TaskStart`: `uncancelable` flag selects the uncancelable-spawn call; handle representation unchanged
- [ ] 7.3 `emit.rs` `Await`: `shielded` flag selects the shielded-await call; narrow result as today
- [ ] 7.4 `emit.rs`: emit the aggregation calls; tuple result materialization for `Task.combine`; emit the `sleep` / `after` / `every` calls
- [ ] 7.5 `IrType::Task` covers `Task.Final<T>` in type lowering and every codegen `match`
- [ ] 7.6 Codegen golden tests: `final task` site emits the uncancelable-spawn symbol, `await final` emits the shielded-await symbol, `Task.combine` emits the join + tuple build

## 8. End-to-end CLI fixtures (`crates/zirk-cli/tests`)

- [ ] 8.1 `valid/task_sibling_failure.zrk`: one child throws while a sibling runs; the sibling is cancelled and cleaned; the primary exception is observed
- [ ] 8.2 `valid/task_cancel.zrk`: `h.cancel(); await h;` observes `CancelledError` at the await and runs `finally` cleanup
- [ ] 8.3 `valid/final_task_survives_cancel.zrk`: a cancelled parent awaits a `final task { }` cleanup block; the cleanup completes and its output appears before the process ends
- [ ] 8.4 `valid/task_combine_tuple.zrk`: `await Task.combine(a, b, c)` prints the three results in order
- [ ] 8.5 `valid/task_all_order.zrk` and `valid/task_settled.zrk`: order preservation and mixed `Fulfilled` / `Rejected` / `Cancelled`
- [ ] 8.6 `invalid/cancel_on_final_task.zrk`: `.cancel()` on a `Task.Final<T>` -> `CANCEL_ON_FINAL_TASK`
- [ ] 8.7 `invalid/task_scope_removed.zrk` and `invalid/cancellation_shield_removed.zrk`: the removed-construct diagnostics
- [ ] 8.8 `invalid/task_combine_arity.zrk`: nine-argument `Task.combine` -> diagnostic pointing at `Task.all`
- [ ] 8.9 `invalid/select_still_deferred.zrk`: `select` still names the select-and-channels slice
- [ ] 8.10 `valid/task_timers.zrk`: `await Task.sleep(d)` yields then resumes; `Task.after` fires once; `Task.every` ticks then stops on `.cancel()`
- [ ] 8.11 `invalid/final_interval.zrk`: `final task` wrapping a `Task.every` -> diagnostic

## 9. Example / validation file

- [ ] 9.1 `examples/structured_tasks_examples.zrk` (already staged): keep sections in sync with final decisions — sibling failure and suppressed exceptions, `h.cancel()` with `finally` cleanup, `await final task { ... }` cleanup that survives cancellation, `Task.combine` heterogeneous join, `Task.all` order, `Task.settled` mixed outcomes, `Task.sleep` / `Task.after` / `Task.every`
- [ ] 9.2 Add a CLI test that compiles and runs `examples/structured_tasks_examples.zrk` and checks its stdout, matching how `examples/range_feature_examples.zrk` is validated
- [ ] 9.3 Confirm the example compiles and runs: `LLVM_SYS_201_PREFIX=... cargo run -p zirk-cli -- run examples/structured_tasks_examples.zrk`

## 10. Normative documentation and status

- [ ] 10.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: rewrite §3 (scopes — drop `task scope`), §6–§7 (cancellation and shields -> `final task` + shielded await), §9 (aggregation -> add `Task.combine`); update the implementation checklist
- [ ] 10.2 `docs/init/ZIRK_ROADMAP.md` Phase 5 step 1: structured-tasks slice delivered; `task scope` and `cancellation shield` removed from the language
- [ ] 10.3 `docs/init/ZIRK_FEATURE_STATUS.md` Phase 5 rows: sibling failure, cancellation, `final task`, `Task.all` / `Task.combine` / `Task.settled` delivered; `select` / `await ... timeout` / `Channel<T>` / `Task.first` still deferred
- [ ] 10.4 Handbook concurrency chapter: cancellation, `final task`, aggregation, and timer (`Task.sleep` / `Task.after` / `Task.every`) sections with runnable examples; update the `Task<T>` reference page and add a `Task.Final<T>` reference page
- [ ] 10.5 `docs/CORE_LANGUAGE_SEMANTICS.md` and any other doc that mentions `task scope` / `cancellation shield`: update or remove

## 11. Companion website (`../zirk-lang-site`)

- [ ] 11.1 Commit the zirk-lang changes first; record the revision
- [ ] 11.2 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`
- [ ] 11.3 Review the `../zirk-lang-site` diff: handbook import, concurrency examples, normative-semantics copy, and the site-owned Phase 5 status catalog — confirm no stale `task scope` / `cancellation shield` references
- [ ] 11.4 Commit `../zirk-lang-site` separately; record both revisions in the change closeout

## 12. Closeout

- [ ] 12.1 `LLVM_SYS_201_PREFIX=... cargo test --workspace` green; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] 12.2 `openspec validate fase-5-structured-tasks --strict`
- [ ] 12.3 Confirm `select`, `await ... timeout`, `Task.first`, `Channel<T>`, the fixed-rate ticker resource, `parallel`, `thread`, `Mutex`, `Atomic` still emit their phase diagnostics
- [ ] 12.4 Confirm `task scope` and `cancellation shield` emit the removed-construct diagnostic, not the phase diagnostic
- [ ] 12.5 `openspec archive fase-5-structured-tasks` after merge
