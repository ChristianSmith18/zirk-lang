## 1. ADR + spec deltas

- [ ] 1.1 `docs/decisions/ADR-018-concurrency-surface.md`: `concurrent` / `spawn` as the structural surface; no coloring on the stackful runtime; binding hoisting; Rule B DAG; ambient timers; the `spawn` vs `Timer.*` await/cancel asymmetry
- [ ] 1.2 `specs/concurrent-scopes/spec.md` — the block, `spawn`, `Job<T>`, Rule B, failure + cancellation, implicit `main` scope (this change; already drafted)
- [ ] 1.3 `specs/timer-operations/spec.md` — `Timer.sleep` / `after` / `every` (this change; already drafted)
- [ ] 1.4 `specs/zirk-grammar/spec.md`: ADDED — `concurrent` block grammar; `spawn` expression grammar; MODIFIED — keyword set gains `concurrent` / `spawn`; "Constructs outside the subset" removes any removed-construct rule for them
- [ ] 1.5 `specs/zirk-type-system/spec.md`: ADDED — `Job<T>` known generic; `spawn expr : Job<T>`; `job.wait() : T` single-consume; `Timer` static-member typing; `concurrent` block binding hoisting and per-branch types
- [ ] 1.6 `specs/zirk-ir-lowering/spec.md`: ADDED — `ScopeEnter` / `ScopeExit` on cleanup edges; `BranchStart`; `JobWait`; `concurrent` block lowering builds the branch DAG and emits branch thunks
- [ ] 1.7 `specs/zirk-native-codegen/spec.md`: MODIFIED — "Codegen for cooperative task suspension" scenarios updated to `BranchStart` / `JobWait`; the renamed C-ABI symbols
- [ ] 1.8 `specs/async-runtime-core/spec.md`: MODIFIED — replace "Aggregation runtime for task collections" with "Scope join runtime"; "Timer service" adds `Timer.sleep` / `after` / `every`; "Single-threaded cooperative executor" safe-point list = blocking channel op, timer wait, `Timer.sleep`, explicit check
- [ ] 1.9 `specs/zirk-errors/spec.md`: MODIFIED — `CancelledError` thrown at the new safe points; ADDED — "a concurrent branch's failure is distinct from `Result.Error`"
- [ ] 1.10 `specs/zirk-feature-phasing/spec.md`: MODIFIED — Phase 5 marks `concurrent` / `spawn` / `Timer` delivered
- [ ] 1.11 `specs/zirk-lexical-syntax/spec.md`: MODIFIED — `concurrent` / `spawn` keywords
- [ ] 1.12 `openspec validate concurrent-blocks-and-timers --strict`

## 2. Runtime (`crates/zirk-runtime`)

- [ ] 2.1 `task.rs`: control block gains `parent: Option<ScopeId>`; a scope table (generational slab of `ScopeControlBlock { branches: Vec<TaskId>, cancel_requested, primary_failure, suppressed }`)
- [ ] 2.2 `executor.rs`: `scope_enter() -> ScopeId`, `branch_register(scope, task)`, `scope_exit(scope)` running the join-or-cancel-and-clean protocol; first-failure sweep with suppressed aggregation
- [ ] 2.3 `executor.rs`: `request_cancel(task)` idempotent, wakes a branch at a cancellable `WaitReason`; scope-level `request_cancel` recurses to branches
- [ ] 2.4 `executor.rs`: safe-point cancellation check (from `Timer.sleep`, blocking channel ops later, `check_cancelled`) — raise `CancelledError` when `cancel_requested && shield_depth == 0`
- [ ] 2.5 `timer.rs`: `Timer.after` (one-shot branch at deadline), `Timer.every` (fixed-delay re-arm loop, cancellable); negative-duration controlled error
- [ ] 2.6 Runtime unit tests: scope join waits for all branches; sibling-failure sweep + suppressed order; `Timer.sleep` yields then resumes; cancel-during-sleep; `Timer.every` re-arms and stops; ambient timer cancelled on scope exit
- [ ] 2.7 `collector.rs`: confirm the per-branch shadow-stack root walk covers branch control blocks (add a fixture if a new root is needed for the scope table)

## 3. Runtime C-ABI (`crates/zirk-runtime` + `crates/zirk-codegen-llvm`)

- [ ] 3.1 Rename: `zirk_rt_task_spawn` -> `zirk_rt_spawn`, `zirk_rt_task_await` -> `zirk_rt_job_wait`, `zirk_rt_task_is_done` -> `zirk_rt_job_done`
- [ ] 3.2 New entry points: `zirk_rt_scope_enter`, `zirk_rt_scope_exit`, `zirk_rt_branch_register`, `zirk_rt_cancel`, `zirk_rt_timer_after`, `zirk_rt_timer_every`, `zirk_rt_sleep`
- [ ] 3.3 `runtime.rs` (codegen): update `symbols::*` and the LLVM declarations
- [ ] 3.4 Integration test exercising the renamed + new C-ABI

## 4. Lexer + AST + parser

- [ ] 4.1 `crates/zirk-lexer`: `concurrent` / `spawn` keywords; tests
- [ ] 4.2 `crates/zirk-ast`: `Stmt::Concurrent { body }` (body carries its binding list + branch statements), `Expr::Spawn { body: SpawnBody }`, `Job<T>` type ref
- [ ] 4.3 `crates/zirk-parser`: parse the `concurrent { }` block; parse `spawn <call>` / `spawn { block }` as a prefix expression; expose the block's top-level binding list for the DAG
- [ ] 4.4 Parser tests: block with named branches, `spawn` in a loop, `inmut h = spawn f()`, `spawn` outside a `concurrent` block -> diagnostic

## 5. Semantic analysis (`crates/zirk-sema`)

- [ ] 5.1 `types.rs`: `Base::Job(u32)` + `job_types` table; `Job<T>` resolves and prints
- [ ] 5.2 `checker.rs`: `Timer` known type with `sleep` / `after` / `every` static members and their signatures
- [ ] 5.3 `checker.rs`: `concurrent` block — build the binding DAG (edges via name refs), reject cycles, type each branch, hoist bindings into the enclosing scope, reject early reads
- [ ] 5.4 `checker.rs`: `spawn expr : Job<T>`; body via `begin_capture_scope` / `finish_capture_scope`; `spawn` only inside a `concurrent`/`main` scope
- [ ] 5.5 `checker.rs`: `job.wait() : T` with single-consume tracking (`SECOND_WAIT` code); `Job<T>` must-use with `_ =` discharge
- [ ] 5.6 `checker.rs`: `Timer.every` may not be placed where it cannot be cancelled (deferred: no non-cancellable region until #5 — for now no restriction)
- [ ] 5.7 New diagnostic codes: `SECOND_WAIT`, `SPAWN_OUTSIDE_SCOPE`, `CONCURRENT_BINDING_CYCLE`, `CONCURRENT_EARLY_READ`
- [ ] 5.8 Checker tests: hoisting, DAG ordering, cycle rejection, `Job` typing + single-consume, `Timer` member typing, spawn-scope rule

## 6. IR + codegen

- [ ] 6.1 `crates/zirk-ir/ir.rs`: `InstKind::ScopeEnter { scope }`, `ScopeExit { scope }`, `BranchStart { scope, target, uncancelable: false }`, `JobWait { job }`; `IrType::Job` (one-word, `i64`); every exhaustive match updated
- [ ] 6.2 `lower.rs`: `concurrent` block -> `ScopeEnter`, lower each branch to a boxed thunk + `BranchStart` in DAG order, `ScopeExit` on every exit edge (reuse `finally` cleanup edges); `spawn` -> `BranchStart` yielding `IrType::Job`; `job.wait()` -> `JobWait`
- [ ] 6.3 `verify.rs`: `BranchStart` target is a callable; `JobWait` operand is `IrType::Job`; `ScopeExit` reachable on every edge of its block
- [ ] 6.4 `crates/zirk-codegen-llvm/emit.rs`: emit `zirk_rt_scope_enter/exit`, a per-branch `extern "C" fn(ptr) -> i64` thunk (reload captures, call lifted body, widen result), `zirk_rt_spawn`, `zirk_rt_job_wait` + narrow
- [ ] 6.5 IR + codegen golden tests

## 7. Fixtures + example

- [ ] 7.1 `crates/zirk-cli/tests/corpus/valid/concurrent_fanout.zrk` — named branches, hoisted results
- [ ] 7.2 `valid/concurrent_spawn_loop.zrk` — dynamic branches
- [ ] 7.3 `valid/concurrent_dataflow.zrk` — Rule B: `b = g(a)` waits for `a`
- [ ] 7.4 `valid/concurrent_sibling_failure.zrk` — one branch throws, sibling cancelled + cleaned
- [ ] 7.5 `valid/timer_sleep_yields.zrk`, `valid/timer_every_stops.zrk`
- [ ] 7.6 `invalid/spawn_outside_scope.zrk`, `invalid/concurrent_cycle.zrk`, `invalid/job_waited_twice.zrk`, `invalid/concurrent_early_read.zrk`
- [ ] 7.7 `examples/concurrent_examples.zrk` — dashboard fan-out, spawn loop, timers, sibling failure; compile-and-run CLI test checking stdout

## 8. Documentation

- [ ] 8.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: add the `concurrent { }` / `spawn` / `Job<T>` / `Timer` sections
- [ ] 8.2 Handbook concurrency chapter: rewrite around `concurrent { }`; new `Timer` reference page; new `Job<T>` reference page
- [ ] 8.3 `docs/init/ZIRK_ROADMAP.md` + `docs/init/ZIRK_FEATURE_STATUS.md`: Phase 5 — `concurrent` / `spawn` / `Timer` delivered
- [ ] 8.4 `README.md`: concurrency line updated

## 9. Website + closeout

- [ ] 9.1 `cargo test --workspace` green; fmt; clippy
- [ ] 9.2 Commit zirk-lang; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review status catalog; commit `../zirk-lang-site` separately; record both revisions
- [ ] 9.3 `openspec validate concurrent-blocks-and-timers --strict`
