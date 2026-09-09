Each group leaves `cargo test --workspace` (and `cargo fmt --all --check` /
`cargo clippy --workspace --all-targets -- -D warnings`) green. Order:
keyword + parser → checker → IR + lowering → codegen → fixtures → docs.

## 1. Keyword ungating and lexer

- [ ] 1.1 `crates/zirk-lexer/src/token.rs`: drop `Task | Await` from the `Phase::FIVE` arm of `Keyword::phase()` (keep `Parallel | Thread | Sync`); update the `Keyword::Task.in_subset()` / `.phase()` tests to expect `in_subset() == true`
- [ ] 1.2 Confirm no lexer-level "outside the subset" path still fires for `task` / `await`; `scope` / `after` / `cancelled` / `shield` / `timeout` stay ordinary identifiers

## 2. AST + parser

- [ ] 2.1 `crates/zirk-ast`: `Expr::Task { body: TaskBody, span }` where `TaskBody` is `Call(Box<Expr>)` or `Block(Block)`; `Expr::Await { handle: Box<Expr>, span }`. Spans carry the `task` / `await` keyword position
- [ ] 2.2 `crates/zirk-parser`: parse `task <expr>` and `task { <block> }` as a unary-precedence prefix expression; parse `await <expr>` likewise
- [ ] 2.3 Parser: `task` immediately followed by `scope` -> targeted diagnostic "`task scope` arrives in fase-5-task-scope"; `await <expr>` followed by `timeout` -> "`await ... timeout` arrives in fase-5-select-and-channels". Neither is a syntax error
- [ ] 2.4 Parser tests: `task f(1)`, `task { return g(); }`, `await h`, `mut u: Task<User> = task load(42);`, the two deferred-form diagnostics, `await` binds tighter than `=` and looser than a call

## 3. Type system (`zirk-sema`)

- [ ] 3.1 `types.rs`: `Base::Task(Box<Type>)`; `Type::from_name` + generic-argument resolution recognize `Task<T>` for any valid `T`; `Task<T>` prints as `Task<...>`
- [ ] 3.2 `checker.rs`: type `Expr::Task` -> `Task<T>` (T = the call/block result type); a `Void` body is allowed and yields `Task<Void>`
- [ ] 3.3 Desugar the task body to a zero-arg closure so the existing closure capture analysis, must-use, and lowering apply; the synthesized closure keeps the `task` span
- [ ] 3.4 Type `Expr::Await` on `Task<T>` -> exactly `T`; `await` on a non-`Task` is a type error naming the expected type
- [ ] 3.5 Reject a task result type wider than a machine word (a `record` / large value `T`) with a clear "arrives with `Task.settled`" message; allow reference, `<=64-bit` scalar, `Boolean`, `Char`, `String`, and `Void`
- [ ] 3.6 Single-consume: track a `Task<T>` local as a linear binding (reuse the `transfer` use-after-move liveness pass); `await x` consumes it; a second `await x` or any later use of `x` is `SECOND_AWAIT` with a "first consumed here" note
- [ ] 3.7 An unconsumed `Task<T>` at scope end is the existing must-use diagnostic; `_ = x` discharges it
- [ ] 3.8 New diagnostic code `SECOND_AWAIT` in `zirk-diagnostics`
- [ ] 3.9 Checker tests: await unwrap type, await-on-non-task rejection, wide-result rejection, second-await rejection + note, must-use + `_ =` discharge, `Task<Void>`

## 4. IR + lowering (`zirk-ir`, `lower.rs`, `verify.rs`)

- [ ] 4.1 `ir.rs`: `IrType::Task` (one-word, ABI `i64`, `needs_allocation() == false`, not a GC reference); `InstKind::TaskStart { body: Operand }` and `InstKind::Await { handle: Operand, result: IrType }`
- [ ] 4.2 `lower.rs`: lower `Expr::Task` -> lower the synthesized closure to a boxed callable (`MakeCallable`), then `TaskStart { body }` yielding an `IrType::Task` value
- [ ] 4.3 `lower.rs`: lower `Expr::Await` -> `Await { handle, result }` yielding the element value
- [ ] 4.4 `verify.rs`: `TaskStart` operand is a callable; `Await` operand is `IrType::Task`; `Await` result type matches the instruction's declared type; `IrType::Task` handled in every exhaustive `match` over `IrType`
- [ ] 4.5 IR lowering tests: `task` shape (MakeCallable then TaskStart), `await` shape (Await with the right result type), a `task` with a captured local

## 5. Native codegen (`zirk-codegen-llvm`)

- [ ] 5.1 `runtime.rs`: declare `zirk_rt_task_spawn` (`i64 (ptr, ptr)`) and `zirk_rt_task_await` (`i64 (i64)`); `symbols::TASK_SPAWN` / `TASK_AWAIT`
- [ ] 5.2 `emit.rs`: a generic `extern "C" fn(ptr) -> i64` task thunk that treats its argument as the boxed-callable value, invokes it through the same load-fn-ptr path `CallCallable` uses, and widens the `T` result to `i64` (pointer bits / zero-extended scalar / 0 for `Void`)
- [ ] 5.3 `emit.rs` `TaskStart`: emit the `MakeCallable` capture block, then `call zirk_rt_task_spawn(thunk, capture_block_ptr)` -> the `i64` handle
- [ ] 5.4 `emit.rs` `Await`: `call zirk_rt_task_await(handle)` -> `i64`, then narrow to the result `IrType` (pointer reinterpret / truncate / discard for `Void`); define a resume label
- [ ] 5.5 `IrType::Task` -> LLVM `i64` in the type lowering; handled in every codegen `match` over `IrType`
- [ ] 5.6 Codegen golden tests: `task` site emits `zirk_rt_task_spawn` with a thunk + capture block; `await` emits `zirk_rt_task_await` + narrow + resume label

## 6. End-to-end CLI fixtures (`crates/zirk-cli/tests`)

- [ ] 6.1 `valid/task_await_value.zrk`: `mut u = task compute(21); stdout.println(await u);` prints `42`
- [ ] 6.2 `valid/task_block.zrk`: `task { ... return ...; }` awaited
- [ ] 6.3 `valid/task_captures_local.zrk`: the body reads a captured local; two independent tasks awaited in turn
- [ ] 6.4 `valid/task_background_outlives_main.zrk`: `main` spawns a task and returns; the task's `stdout.println` still runs (executor keeps the process alive)
- [ ] 6.5 `valid/task_result_discarded.zrk`: `_ = task fire_and_forget();` compiles and the task still runs
- [ ] 6.6 `valid/task_under_gc_pressure.zrk`: a suspended task holding a captured reference survives allocation pressure from `main`
- [ ] 6.7 `invalid/await_consumed_twice.zrk`: second `await` -> `SECOND_AWAIT`
- [ ] 6.8 `invalid/await_non_task.zrk`: `await 5` -> type error
- [ ] 6.9 `invalid/task_result_ignored.zrk`: unconsumed `Task<T>` -> must-use
- [ ] 6.10 `invalid/task_scope_deferred.zrk` / `invalid/await_timeout_deferred.zrk`: the targeted "arrives in ..." diagnostics
- [ ] 6.11 `invalid/parallel_still_deferred.zrk`: `parallel` still names Phase 5 step 6

## 7. Documentation and status

- [ ] 7.1 `docs/init/ZIRK_FEATURE_STATUS.md` Phase 5: `task`/`await` row -> lexer→CLI = yes for the bare forms; note `task scope` / `await ... timeout` still pending
- [ ] 7.2 `docs/init/ZIRK_ROADMAP.md` Phase 5 step 1: bare `task`/`await` delivered (`fase-5-task-await`)
- [ ] 7.3 Handbook `02-handbook/18-concurrency/`: fill in the `task` / `await` section with runnable examples; add a `11-reference` `Task<T>` page
- [ ] 7.4 After merge: `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`

## 8. Closeout

- [ ] 8.1 `cargo test --workspace` green; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] 8.2 `openspec validate fase-5-task-await --strict`
- [ ] 8.3 Confirm `task scope`, `await ... timeout`, `select`, `cancellation shield`, `parallel`, `thread` still emit their diagnostics
