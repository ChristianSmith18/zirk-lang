Each group leaves `cargo test --workspace` (and `cargo fmt --all --check` /
`cargo clippy --workspace --all-targets -- -D warnings`) green. Order:
keyword + parser → checker → IR + lowering → codegen → fixtures → docs.

## 1. Keyword ungating and lexer

- [x] 1.1 `crates/zirk-lexer/src/token.rs`: drop `Task | Await` from the `Phase::FIVE` arm of `Keyword::phase()` (keep `Parallel | Thread | Sync`); update the `Keyword::Task.in_subset()` / `.phase()` tests to expect `in_subset() == true`
- [x] 1.2 Confirm no lexer-level "outside the subset" path still fires for `task` / `await`; `scope` / `after` / `cancelled` / `shield` / `timeout` stay ordinary identifiers

## 2. AST + parser

- [x] 2.1 `crates/zirk-ast`: `Expr::Task { body: TaskBody, span }` where `TaskBody` is `Expr(Box<Expr>)` or `Block(Block)`; `Expr::Await { operand: Box<Expr>, span }`. Spans carry the `task` / `await` keyword position
- [x] 2.2 `crates/zirk-parser`: parse `task <expr>` and `task { <block> }` as a unary-precedence prefix expression; parse `await <expr>` likewise
- [x] 2.3 Parser: `task` immediately followed by `scope` -> targeted diagnostic "`task scope` arrives in fase-5-task-scope"; `await <expr>` followed by `timeout` -> "`await ... timeout` arrives in fase-5-select-and-channels". Neither is a syntax error
- [x] 2.4 Parser tests: `task f(1)`, `task { return g(); }`, `await h`, `mut u: Task<User> = task load(42);`, the two deferred-form diagnostics, `await` binds tighter than `=` and looser than a call

## 3. Type system (`zirk-sema`)

- [x] 3.0 Refactor `check_lambda`'s scope + capture core into `begin_capture_scope` / `finish_capture_scope`, shared with `check_task`.
- [x] 3.1 `types.rs`: `Base::Task(u32)` + `task_types` table; `resolve_type_atom` recognizes `Task<T>`; `Task<T>` prints as `Task<...>`; removed from `pending_type`
- [x] 3.2 `check_task`: infers `T` from the body (block `return` result / expression type); a `Void` body yields `Task<Void>`; registers a capturing-closure `fn_type` + `LambdaInfo`
- [x] 3.3 The `task` body runs the ordinary closure capture analysis via the pushed function scope + `capture_stack`; its `LambdaInfo` is keyed by the `task` span
- [x] 3.4 Type `Expr::Await` on `Task<T>` -> exactly `T`; `await` on a non-`Task` is a `TYPE_MISMATCH` naming `Task<T>`
- [x] 3.5 Reject a task result type wider than a machine word with an "arrives with `Task.settled`" message; allows reference, `<=64-bit` scalar, `Boolean`, `Char`, `String`, `Void`, `Duration`
- [x] 3.6 Single-consume: `await x` consumes the local (`awaited_at` map + `mark_moved`); a second `await x` or any later use of `x` is `SECOND_AWAIT` with a "first consumed on line N" note
- [x] 3.7 Statement-level unconsumed `task ...;` is the must-use diagnostic (`DISCARDED_RESULT`); `_ = task ...;` discharges it. NOTE: scope-end unconsumed *local* (`mut h = task ...;` never awaited) not yet flagged.
- [x] 3.8 New diagnostic code `SECOND_AWAIT` (`E0465`) — placed in `zirk-sema`'s `codes` module alongside the other sema codes (codebase convention), not `zirk-diagnostics`
- [x] 3.9 Checker tests in `crates/zirk-sema/tests/typing.rs`: await unwrap type, await-on-non-task, wide-result, second-await + note, must-use + `_ =` discharge, `Task<Void>`, block inference

## 4. IR + lowering (`zirk-ir`, `lower.rs`, `verify.rs`)

- [x] 4.1 `ir.rs`: `IrType::Task` (one-word, ABI `i64`, `needs_allocation() == false`, not a GC reference); `InstKind::TaskStart { target, body }` and `InstKind::Await { handle: Operand, result: IrType }`
- [x] 4.2 `lower.rs`: lower `Expr::Task` -> lower the synthesized closure to a boxed callable (`MakeCallable`), then `TaskStart { target, body }` yielding an `IrType::Task` value
- [x] 4.3 `lower.rs`: lower `Expr::Await` -> `Await { handle, result }` yielding the element value
- [x] 4.4 `verify.rs`: `TaskStart` operand is a callable; `Await` operand is `IrType::Task`; `Await` result type matches the instruction's declared type; `IrType::Task` handled in every exhaustive `match` over `IrType`
- [x] 4.5 IR lowering tests: `task` shape (MakeCallable then TaskStart), `await` shape (Await with the right result type), a `task` with a captured local

## 5. Native codegen (`zirk-codegen-llvm`)

- [x] 5.1 `runtime.rs`: declare `zirk_rt_task_spawn` (`i64 (ptr, ptr)`) and `zirk_rt_task_await` (`i64 (i64)`); `symbols::TASK_SPAWN` / `TASK_AWAIT`
- [x] 5.2 `emit.rs`: a per-site `extern "C" fn(ptr) -> i64` task thunk uses the explicit `TaskStart.target`, reloads captured values, calls the lifted body, and widens the `T` result to `i64` (pointer bits / zero-extended scalar / 0 for `Void`)
- [x] 5.3 `emit.rs` `TaskStart`: emit the `MakeCallable` capture block, then `call zirk_rt_task_spawn(thunk, capture_block_ptr)` -> the `i64` handle
- [x] 5.4 `emit.rs` `Await`: `call zirk_rt_task_await(handle)` -> `i64`, then narrow to the result `IrType` (pointer reinterpret / truncate / discard for `Void`); define a resume label
- [x] 5.5 `IrType::Task` -> LLVM `i64` in the type lowering; handled in every codegen `match` over `IrType`
- [x] 5.6 Codegen golden tests: `task` site emits `zirk_rt_task_spawn` with a thunk + capture block; `await` emits `zirk_rt_task_await` + narrow + resume label

## 6. End-to-end CLI fixtures (`crates/zirk-cli/tests`)

- [x] 6.1 `valid/task_await_value.zrk`: `mut u = task compute(21); stdout.println(await u);` prints `42`
- [x] 6.2 `valid/task_block.zrk`: `task { ... return ...; }` awaited
- [x] 6.3 `valid/task_captures_local.zrk`: the body reads a captured local; two independent tasks awaited in turn
- [x] 6.4 `valid/task_background_outlives_main.zrk`: `main` spawns a task and returns; the task's `stdout.println` still runs (executor keeps the process alive)
- [x] 6.5 `valid/task_result_discarded.zrk`: `_ = task fire_and_forget();` compiles and the task still runs
- [x] 6.6 `valid/task_under_gc_pressure.zrk`: a suspended task holding a captured reference survives allocation pressure from `main`
- [x] 6.7 `invalid/await_consumed_twice.zrk`: second `await` -> `SECOND_AWAIT`
- [x] 6.8 `invalid/await_non_task.zrk`: `await 5` -> type error
- [x] 6.9 `invalid/task_result_ignored.zrk`: unconsumed `Task<T>` -> must-use
- [x] 6.10 `invalid/task_scope_deferred.zrk` / `invalid/await_timeout_deferred.zrk`: the targeted "arrives in ..." diagnostics
- [x] 6.11 `invalid/parallel_still_deferred.zrk`: `parallel` still names Phase 5 step 6

## 7. Documentation and status

- [x] 7.1 `docs/init/ZIRK_FEATURE_STATUS.md` Phase 5: `task`/`await` row -> lexer→CLI = yes for the bare forms; note `task scope` / `await ... timeout` still pending
- [x] 7.2 `docs/init/ZIRK_ROADMAP.md` Phase 5 step 1: bare `task`/`await` delivered (`fase-5-task-await`)
- [x] 7.3 Handbook `02-handbook/18-concurrency/`: fill in the `task` / `await` section with runnable examples; add a `11-reference` `Task<T>` page
- [ ] 7.4 After merge: `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`

## 8. Closeout

- [x] 8.1 `cargo test --workspace` green; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
- [x] 8.2 `openspec validate fase-5-task-await --strict`
- [x] 8.3 Confirm `task scope`, `await ... timeout`, `select`, `cancellation shield`, `parallel`, `thread` still emit their diagnostics
