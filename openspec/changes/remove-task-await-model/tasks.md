## 1. Spec deltas

- [ ] 1.1 `specs/zirk-structured-concurrency/spec.md`: REMOVED — Task results have one consumer; Typed structured tasks; Structured timeout; Task aggregation policies; Result and settlement separation; Fair selection. Each with **Reason** + **Migration** (naming the change #2–#5 replacement)
- [ ] 1.2 `specs/zirk-structured-concurrency/spec.md`: MODIFIED — "Structured task failure" -> "Structured failure propagation" (model-neutral: "a concurrent operation" not "a task"); "Cooperative cancellation and shielding" -> "Cooperative cancellation" (drop the `cancellation shield` sentence + scenario); "Cancellation metadata and scheduling remain safe" (drop `Task<T>` wording); "Safe task captures" -> "Safe concurrent captures"; "Data-race analysis is enforced before real parallelism exists" (drop `task scope` / `select` wording); "A suspended task's references stay reachable" -> "A suspended operation's references stay reachable"
- [ ] 1.3 `specs/zirk-grammar/spec.md`: REMOVED — "Task creation and scope syntax"; "Await and timeout syntax"; "Select syntax"; "Cancellation shield syntax". MODIFIED — "Constructs outside the subset" (drop `task`/`await`/`select` from the parsed list; add a removed-construct rule); "Safety and concurrency grammar" (drop task/await/scope/shield/select, keep `commit` + unsafe); "`final` modifier positions" (drop the `final task` position added by the dropped change)
- [ ] 1.4 `specs/zirk-type-system/spec.md`: REMOVED — "Async core types are known"; "`task` produces a typed handle and `await` produces the element type"; "A task result is consumed exactly once"; "Multiple observers do not implicitly clone a task result"; "`Task.all`, `Task.first`, and `Task.settled` typing". MODIFIED — "Memory and task type family" -> drop `Task<T>` / `TaskSettlement<T>` (keep `Weak`/`Pointer`/`NativeSlice`); "Derived concurrent capabilities" + "Concurrency boundary and capture checking" restated model-neutral
- [ ] 1.5 `specs/zirk-ir-lowering/spec.md`: REMOVED — "Lowering of task creation and awaiting"; "Lowering of structured scopes with cleanup edges"; "Lowering of cancellation, shields, and timeouts"; "Lowering of select". Keep the cleanup-edge mechanism described under `finally` / resources
- [ ] 1.6 `specs/zirk-native-codegen/spec.md`: MODIFIED — "Codegen for cooperative task suspension" (keep the context-switch requirement; drop the `TaskStart`/`Await` scenarios). Keep "Context-switch support for every supported target", "Per-task shadow-stack frame registration", "Executor lifecycle wraps the entrypoint" verbatim
- [ ] 1.7 `specs/zirk-errors/spec.md`: MODIFIED — "Cancellation and timeout are compiler-known catchable failures" (drop the shield sentence + scenario; keep `CancelledError`/`TimeoutError`); REMOVED — "Task rejection is distinct from `Result.Error`" (**Migration**: change #2 restates it as "a concurrent operation's failure is distinct from `Result.Error`"). Keep "Suppressed failures aggregate during structured cancellation"
- [ ] 1.8 `specs/zirk-lexical-syntax/spec.md`: MODIFIED — remove `Task` / `Await` from the keyword set and the Phase 5 keyword list; `select` / `scope` / `shield` are ordinary identifiers
- [ ] 1.9 `specs/zirk-feature-phasing/spec.md`: MODIFIED — "Memory and concurrency implementation order": Phase 5 now delivers `concurrent` / `parallel` / `spawn` + the method API; `task` / `await` / `select` / `cancellation shield` are **removed**, reported as removed constructs, not deferred
- [ ] 1.10 `openspec validate remove-task-await-model --strict`

## 2. Lexer + AST

- [ ] 2.1 `crates/zirk-lexer/src/token.rs`: drop `Keyword::Task` / `Keyword::Await`; update `Keyword::phase()` / `in_subset()` and their tests
- [ ] 2.2 `crates/zirk-ast/src/lib.rs`: remove `Expr::Task`, `Expr::Await`, `TaskBody`, and any `task scope` node; remove the `final` flag added for `final task`
- [ ] 2.3 `crates/zirk-ast` tests updated

## 3. Parser

- [ ] 3.1 `crates/zirk-parser/src/parser.rs`: remove the `task` / `await` prefix-expression parsing, the `task scope` production, the `await ... timeout` tail, the `select { }` block, the `cancellation shield` block
- [ ] 3.2 `report_if_from_another_phase` (parser.rs ~265): `task` / `await` / `select` / `cancellation shield` -> `E_REMOVED_CONSTRUCT` with the replacement name; `scope` / `shield` stop being special
- [ ] 3.3 New diagnostic code `REMOVED_CONSTRUCT` in the parser codes module
- [ ] 3.4 Parser tests: each removed keyword emits the removed-construct diagnostic, not a token error; `select` / `scope` / `shield` parse as identifiers

## 4. Semantic analysis

- [ ] 4.1 `crates/zirk-sema/src/types.rs`: remove `Base::Task`, `Base::TaskFinal`, the `task_types` / `task_final_types` tables, `Task<T>` / `Task.Final<T>` / `TaskSettlement<T>` resolution; keep `CancellationReason` (variant set: just `Cancelled` for now)
- [ ] 4.2 `crates/zirk-sema/src/checker.rs`: remove `check_task`, `check_await`, the `awaited_at` single-consume map, `SECOND_AWAIT`, the wide-result check, `Task.*` static-member resolution, must-use of `Task<T>`
- [ ] 4.3 `crates/zirk-sema`: keep `begin_capture_scope` / `finish_capture_scope` (change #2 reuses them for `concurrent` branches)
- [ ] 4.4 `crates/zirk-sema/tests/typing.rs`: drop the task/await typing tests

## 5. IR + codegen

- [ ] 5.1 `crates/zirk-ir/src/ir.rs`: remove `IrType::Task`, `InstKind::TaskStart`, `InstKind::Await`; every exhaustive match over `IrType` / `InstKind` updated
- [ ] 5.2 `crates/zirk-ir/src/lower.rs` + `verify.rs`: remove the task/await/scope lowering; keep the cleanup-edge machinery
- [ ] 5.3 `crates/zirk-codegen-llvm/src/emit.rs`: remove `TaskStart` / `Await` emission and the per-site task thunk
- [ ] 5.4 `crates/zirk-codegen-llvm/src/runtime.rs`: keep `zirk_rt_task_spawn` / `zirk_rt_task_await` / `zirk_rt_run_main` declarations + a doc note "reused and renamed by concurrent-blocks-and-timers"
- [ ] 5.5 `crates/zirk-ir/tests` + codegen golden tests: drop task/await cases

## 6. Runtime (doc-only)

- [ ] 6.1 `crates/zirk-runtime/src/{lib.rs, executor.rs, task.rs, context.rs, timer.rs}`: doc-comment pass — "task" means the scheduler's stackful coroutine, not a language `Task<T>`; no code change
- [ ] 6.2 `cargo test -p zirk-runtime` unchanged and green

## 7. Fixtures

- [ ] 7.1 `crates/zirk-cli/tests/corpus/valid/task_*.zrk` and `await_*` : move to `invalid/` asserting `REMOVED_CONSTRUCT`, or delete if redundant
- [ ] 7.2 `crates/zirk-cli/tests/corpus/invalid/{task_scope_deferred, await_timeout_deferred, select_*, parallel_still_deferred}.zrk`: update expected diagnostic to removed-construct / adjust

## 8. Documentation

- [ ] 8.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: rewrite to the model-neutral survivors only; add a header note that the surface is defined by the `concurrent-blocks-and-timers`, `parallel-cpu-regions`, `typed-channels`, and `concurrency-completion` changes
- [ ] 8.2 Remove `docs/handbook/04-standard-library/08-std-task.md`; renumber the chapter; fix `SUMMARY.md`
- [ ] 8.3 `docs/handbook/02-handbook/*concurrency*`: remove `task` / `await` sections
- [ ] 8.4 `docs/ZIRK_LANGUAGE_SPEC.md` section 10, `docs/ZIRK_SPEC_FINAL.md`, `docs/CORE_LANGUAGE_SEMANTICS.md`: drop `task` / `await` / `Task<T>`
- [ ] 8.5 `docs/init/ZIRK_ROADMAP.md` + `docs/init/ZIRK_FEATURE_STATUS.md`: Phase 5 rows -> new surface; `task`/`await` = removed
- [ ] 8.6 `docs/decisions/ADR-017-modelo-de-suspension.md`: add "Superseded surface decisions" section
- [ ] 8.7 `README.md`: the "not yet implemented: concurrency (`task`/`await`/...)" line -> new surface names

## 9. Website + closeout

- [ ] 9.1 Commit the zirk-lang changes; record the revision
- [ ] 9.2 `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`
- [ ] 9.3 Review `../zirk-lang-site` diff + site-owned Phase 5 status catalog; confirm no stale `task` / `await`
- [ ] 9.4 Commit `../zirk-lang-site` separately; record both revisions
- [ ] 9.5 `cargo test --workspace` green; `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] 9.6 `rm -r openspec/changes/fase-5-structured-tasks/`
- [ ] 9.7 `openspec validate remove-task-await-model --strict`
