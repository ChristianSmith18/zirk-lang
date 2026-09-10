## 1. ADR + spec deltas

- [x] 1.1 `docs/decisions/ADR-018-concurrency-surface.md`: `concurrent` / `spawn` as the structural surface; no coloring on the stackful runtime; binding hoisting; Rule B DAG; ambient timers; the `spawn` vs `Timer.*` await/cancel asymmetry
- [x] 1.2 `specs/concurrent-scopes/spec.md` — the block, `spawn`, `Job<T>`, Rule B, failure + cancellation, implicit `main` scope (this change; already drafted)
- [x] 1.3 `specs/timer-operations/spec.md` — `Timer.sleep` / `after` / `every` (this change; already drafted)
- [x] 1.4 `specs/zirk-grammar/spec.md`: ADDED — `concurrent` block grammar; `spawn` expression grammar; MODIFIED — keyword set gains `concurrent` / `spawn`; "Constructs outside the subset" removes any removed-construct rule for them
- [x] 1.5 `specs/zirk-type-system/spec.md`: ADDED — `Job<T>` known generic; `spawn expr : Job<T>`; `job.wait() : T` single-consume; `Timer` static-member typing; `concurrent` block binding hoisting and per-branch types
- [x] 1.6 `specs/zirk-ir-lowering/spec.md`: ADDED — `ScopeEnter` / `ScopeExit` on cleanup edges; `BranchStart`; `JobWait`; `concurrent` block lowering builds the branch DAG and emits branch thunks
- [x] 1.7 `specs/zirk-native-codegen/spec.md`: MODIFIED — "Codegen for cooperative task suspension" scenarios updated to `BranchStart` / `JobWait`; the renamed C-ABI symbols
- [x] 1.8 `specs/async-runtime-core/spec.md`: MODIFIED — replace "Aggregation runtime for task collections" with "Scope join runtime"; "Timer service" adds `Timer.sleep` / `after` / `every`; "Single-threaded cooperative executor" safe-point list = blocking channel op, timer wait, `Timer.sleep`, explicit check
- [x] 1.9 `specs/zirk-errors/spec.md`: MODIFIED — `CancelledError` thrown at the new safe points; ADDED — "a concurrent branch's failure is distinct from `Result.Error`"
- [x] 1.10 `specs/zirk-feature-phasing/spec.md`: MODIFIED — Phase 5 marks `concurrent` / `spawn` / `Timer` delivered
- [x] 1.11 `specs/zirk-lexical-syntax/spec.md`: MODIFIED — `concurrent` / `spawn` keywords
- [x] 1.12 `openspec validate concurrent-blocks-and-timers --strict`

## 2. Runtime (`crates/zirk-runtime`)

- [x] 2.1 `task.rs`: control block gains `parent: Option<ScopeId>`; a scope table (generational slab of `ScopeControlBlock { branches: Vec<TaskId>, cancel_requested, primary_failure, suppressed }`)
- [x] 2.2 `executor.rs`: `scope_enter() -> ScopeId`, `branch_register(scope, task)`, `scope_exit(scope)` running the join-or-cancel-and-clean protocol; first-failure sweep with suppressed aggregation
- [x] 2.3 `executor.rs`: `request_cancel(task)` idempotent, wakes a branch at a cancellable `WaitReason`; scope-level `request_cancel` recurses to branches
- [x] 2.4 `executor.rs`: safe-point cancellation check (from `Timer.sleep`, blocking channel ops later, `check_cancelled`) — raise `CancelledError` when `cancel_requested && shield_depth == 0`
- [x] 2.5 `timer.rs`: `Timer.after` (one-shot branch at deadline), `Timer.every` (fixed-delay re-arm loop, cancellable); negative-duration controlled error
- [x] 2.6 Runtime unit tests: scope join waits for all branches; sibling-failure sweep + suppressed order; `Timer.sleep` yields then resumes; cancel-during-sleep; `Timer.every` re-arms and stops; ambient timer cancelled on scope exit
- [x] 2.7 `collector.rs`: confirm the per-branch shadow-stack root walk covers branch control blocks (add a fixture if a new root is needed for the scope table)

## 3. Runtime C-ABI (`crates/zirk-runtime` + `crates/zirk-codegen-llvm`)

- [x] 3.1 Rename: `zirk_rt_task_spawn` -> `zirk_rt_spawn`, `zirk_rt_task_await` -> `zirk_rt_job_wait`, `zirk_rt_task_is_done` -> `zirk_rt_job_done`
- [x] 3.2 New entry points: `zirk_rt_scope_enter`, `zirk_rt_scope_exit`, `zirk_rt_branch_register`, `zirk_rt_cancel`, `zirk_rt_timer_after`, `zirk_rt_timer_every`, `zirk_rt_sleep`
- [x] 3.3 `runtime.rs` (codegen): update `symbols::*` and the LLVM declarations
- [x] 3.4 Integration test exercising the renamed + new C-ABI

## 4. Lexer + AST + parser

- [x] 4.1 `crates/zirk-lexer`: `concurrent` / `spawn` keywords; tests
- [x] 4.2 `crates/zirk-ast`: `Stmt::Concurrent { body }` (body carries its binding list + branch statements), `Expr::Spawn { body: SpawnBody }`, `Job<T>` type ref
- [x] 4.3 `crates/zirk-parser`: parse the `concurrent { }` block; parse `spawn <call>` / `spawn { block }` as a prefix expression; expose the block's top-level binding list for the DAG
- [x] 4.4 Parser tests: block with named branches, `spawn` in a loop, `inmut h = spawn f()`, `spawn` outside a `concurrent` block -> diagnostic

## 5. Semantic analysis (`crates/zirk-sema`)

- [x] 5.1 `types.rs`: `Base::Job(u32)` + `job_types` table; `Job<T>` resolves and prints
- [x] 5.2 `checker.rs`: `Timer` known type with `sleep` / `after` / `every` static members and their signatures
- [x] 5.3 `checker.rs`: `concurrent` block — build the binding DAG (edges via name refs), reject cycles, type each branch, hoist bindings into the enclosing scope, reject early reads
- [x] 5.4 `checker.rs`: `spawn expr : Job<T>`; body via `begin_capture_scope` / `finish_capture_scope`; `spawn` only inside a `concurrent`/`main` scope
- [x] 5.5 `checker.rs`: `job.wait() : T` with single-consume tracking (`SECOND_WAIT` code); `Job<T>` must-use with `_ =` discharge
- [x] 5.6 `checker.rs`: `Timer.every` may not be placed where it cannot be cancelled (deferred: no non-cancellable region until #5 — for now no restriction)
- [x] 5.7 New diagnostic codes: `SECOND_WAIT`, `SPAWN_OUTSIDE_SCOPE`, `CONCURRENT_BINDING_CYCLE`, `CONCURRENT_EARLY_READ` (+ `UNUSED_JOB`)
- [x] 5.8 Checker tests: hoisting, DAG ordering, cycle rejection, `Job` typing + single-consume, `Timer` member typing, spawn-scope rule

## 6. IR + codegen

- [x] 6.1 `crates/zirk-ir/ir.rs`: `InstKind::ScopeEnter`, `ScopeExit { scope }`, `BranchStart { target, body, scope, kind, delay }` (kind = Spawn/TimerAfter/TimerEvery), `JobWait { job, result }`, `JobDone`, `JobCancel`, `TimerSleep`; `IrType::Job`; verify + operands matches updated
- [x] 6.2 `lower.rs`: `concurrent` block -> `ScopeEnter` + branch thunks in DAG order (waiting predecessors before a dependent branch) + `ScopeExit`; `spawn` -> `BranchStart`; `job.wait()`/`.cancel()`/`.done`; `Timer.sleep`/`after`/`every`; `main` implicit root scope
- [x] 6.3 `verify.rs`: `BranchStart` target/callable/scope + kind↔delay; `JobWait`/`JobDone`/`JobCancel` operand is `IrType::Job`; `ScopeEnter`/`ScopeExit`/`TimerSleep` typing
- [x] 6.4 `crates/zirk-codegen-llvm/emit.rs`: per-site `extern "C" fn(ptr) -> i64` branch thunk (reload captures, call lifted body, exception->branch-fail, widen), `zirk_rt_spawn`/`branch_register`/`timer_after`/`timer_every`/`job_wait`+narrow/`job_done`/`cancel`/`sleep`/`scope_enter`/`scope_exit`; `uwtable` on generated funcs for native cancel/exception unwind
- [x] 6.5 IR golden tests (`zirk-ir/tests/lowering.rs`) + codegen emission tests (`zirk-codegen-llvm/tests/emission.rs`)

## 7. Fixtures + example

- [x] 7.1 `crates/zirk-cli/tests/corpus/valid/concurrent_fanout.zrk` — named branches, hoisted results
- [x] 7.2 `valid/concurrent_spawn_loop.zrk` — dynamic branches
- [x] 7.3 `valid/concurrent_dataflow.zrk` — Rule B: `b = g(a)` waits for `a`
- [x] 7.4 `valid/concurrent_sibling_failure.zrk` — one branch throws, sibling cancelled, exception re-raised
- [x] 7.5 `valid/timer_sleep_yields.zrk`, `valid/timer_every_stops.zrk`
- [x] 7.6 `invalid/spawn_outside_scope.zrk`, `invalid/concurrent_cycle.zrk`, `invalid/job_waited_twice.zrk`, `invalid/concurrent_early_read.zrk`
- [x] 7.7 `examples/concurrent_examples.zrk` — dashboard fan-out, spawn loop, `Job<T>`, timers, sibling failure; `end_to_end.rs` compile-and-run test checking stdout

## 8. Documentation

- [x] 8.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: add the `concurrent { }` / `spawn` / `Job<T>` / `Timer` sections
- [x] 8.2 Handbook concurrency chapter: rewrite around `concurrent { }`; new `Timer` reference page; new `Job<T>` reference page
- [x] 8.3 `docs/init/ZIRK_ROADMAP.md` + `docs/init/ZIRK_FEATURE_STATUS.md`: Phase 5 — `concurrent` / `spawn` / `Timer` delivered
- [x] 8.4 `README.md`: concurrency line updated

## 9. Website + closeout

- [x] 9.1 `cargo test --workspace` green (1250+ tests); `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
- [x] 9.2 Commit zirk-lang (`d788e92`); `./scripts/sync-website-content.sh --audit-date 2026-09-10`; reviewed status catalog; committed `../zirk-lang-site` separately (`a99a8a2`)
- [x] 9.3 `openspec validate concurrent-blocks-and-timers --strict` — valid
