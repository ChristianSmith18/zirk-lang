## 1. Unsafe journal rollback on early exits

- [x] 1.1 Extend `LoopTargets` in `crates/zirk-ir/src/lower.rs` with an `unsafe_depth: usize` field and populate it from `self.unsafe_stack.len()` in `lower_loop`, `lower_for_in`, `lower_while`, and `lower_for` so that `break` and `continue` know how many `unsafe` frames they are leaving.
- [x] 1.2 Introduce a `run_exit_cleanup` helper in `crates/zirk-ir/src/lower.rs` that combines the active `try_stack[try_depth..]` and `unsafe_stack[unsafe_depth..]` frames into one `Scope` list ordered by `pushed_at`, then calls `lower_finally_block` for `try` frames and emits `Load(journal_slot)` + `JournalRollback` for uncommitted `unsafe` frames.
- [x] 1.3 Update `lower_break` in `crates/zirk-ir/src/lower.rs` to call `run_exit_cleanup(loop_target.try_depth, loop_target.unsafe_depth, ...)` before emitting the `Jump` to `loop_target.break_to`.
- [x] 1.4 Update `lower_continue` in `crates/zirk-ir/src/lower.rs` to call `run_exit_cleanup(loop_target.try_depth, loop_target.unsafe_depth, ...)` before emitting the `Jump` to `loop_target.continue_to`.
- [x] 1.5 Update `lower_return` in `crates/zirk-ir/src/lower.rs` to store the return value, call `run_exit_cleanup(0, 0, ...)`, and then load the return slot and emit `Terminator::Return`.
- [x] 1.6 Set `UnsafeFrame.committed = true` when emitting `JournalRollback` so that `end_unsafe_frame` does not generate a phantom `JournalCommit` in the now-unreachable block after the jump/return.
- [x] 1.7 Add `zirk-ir/tests/lowering.rs` cases that assert `JournalRollback` is emitted before `Terminator::Return`, before `Jump` for `break`, and before `Jump` for `continue` inside `unsafe { ... }`.
- [x] 1.8 Add CLI fixtures in `crates/zirk-cli/tests/corpus/valid/` for `unsafe` rollback on `return`, `break`, `continue`, and nested `try`/`unsafe` combinations; add invalid fixtures for rejected escape cases.

## 2. Pointer.from over record and value class lvalues

- [x] 2.1 Confirm and adjust `check_pointer_from` in `crates/zirk-sema/src/checker.rs` so it accepts `record` and `value class` lvalues; add a rejection for root expressions that are not lvalues (e.g., `Foo().x` or `makePoint().y`).
- [x] 2.2 Rewrite `lower_pointer_from` in `crates/zirk-ir/src/lower.rs` to build an lvalue address chain: `PointerFromSlot` for the root local/parameter, then one or more `PointerFromField` for nested fields, keeping the container as a pointer rather than a loaded value.
- [x] 2.3 Update `PointerFromField` verification in `crates/zirk-ir/src/verify.rs` to accept an `object` operand whose type is either `Object` or a `Pointer` whose pointee is `Object` or `Value`.
- [x] 2.4 Generalize `field_pointer` and `object_layout_of` in `crates/zirk-codegen-llvm/src/emit.rs` to emit a `ValueLayout`-based `getelementptr` with no object header when the container is a pointer to a `record`/`value class`.
- [x] 2.5 Add `crates/zirk-sema/tests/typing.rs` cases for `Pointer.from` on `record`, `value class`, nested record fields, and temporaries.
- [x] 2.6 Add `crates/zirk-ir/tests/lowering.rs` and CLI fixtures in `crates/zirk-cli/tests/corpus/valid/` that read and write through `Pointer<T>` obtained from `record` and `value class` fields inside `unsafe`.

## 3. String[index] read-only grapheme access

- [x] 3.1 Add a `Base::String` entry to `indexable_element` in `crates/zirk-sema/src/checker.rs` with element `Type::CHAR` and `writable = false`, so `String[i]` type-checks and `s[i] = c` is rejected.
- [x] 3.2 Define `InstKind::StringGraphemeOffset { string: Operand, index: Operand }` returning `I64` in `crates/zirk-ir/src/ir.rs` alongside the existing `GraphemeLenAt`/`GraphemeSlice` instructions.
- [x] 3.3 Implement `zirk_str_grapheme_offset` in `crates/zirk-runtime/src/string.rs` using `UnicodeSegmentation` to return the byte offset of the i-th grapheme or `-1` if out of bounds.
- [x] 3.4 Declare the `zirk_str_grapheme_offset` C-ABI symbol and `FunctionValue` in `crates/zirk-codegen-llvm/src/runtime.rs`.
- [x] 3.5 Add emission for `StringGraphemeOffset` in `crates/zirk-codegen-llvm/src/emit.rs`.
- [x] 3.6 Bifurcate `lower_index_read` in `crates/zirk-ir/src/lower.rs` for `String` to emit `StringGraphemeOffset`, bounds-check, `GraphemeLenAt`, and `GraphemeSlice`, producing an `IrType::Char`.
- [x] 3.7 Guard `assign_target_type` and `store_assign_target` in `crates/zirk-ir/src/lower.rs` for `String` indexing, relying on the checker rejection but ensuring the lowerer does not panic.
- [x] 3.8 Add tests in `crates/zirk-sema/tests/typing.rs`, `crates/zirk-ir/tests/typing.rs`, and CLI corpus for valid `String[i]`, out-of-bounds, negative-index, and rejected `s[i] = c`.

## 4. Closeout

- [x] 4.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` rows for memory-safety (`unsafe` rollback), data-types (`Pointer.from` on value types), and scalars/indexing (`String[index]`) to reflect completion.
- [x] 4.2 Update `docs/init/ZIRK_ROADMAP.md` to remove the `unsafe` journal early-exit limitation and note the `String` indexing pull-in if applicable.
- [x] 4.3 Update `docs/handbook/13-appendices/07-current-limitations.md` to remove or rephrase the limitations closed by this change.
- [x] 4.4 Run `cargo fmt --all` and `cargo clippy --workspace`, fixing any formatting or lint warnings introduced.
- [x] 4.5 Run `cargo test --workspace` and fix all failures before considering the change complete.
- [x] 4.6 Run `openspec validate phase-4e-pending-closeout --strict` and archive or sync the change once the implementation is merged.

## 5. Quality gates and test coverage

- [x] 5.1 Define concrete test cases for unsafe rollback in `crates/zirk-ir/tests/lowering.rs` and `crates/zirk-cli/tests/corpus/` before implementation, covering `return`, `break`, `continue`, and mixed `try`/`unsafe` nesting.
- [x] 5.2 Define concrete test cases for `Pointer.from` in `crates/zirk-sema/tests/typing.rs`, `crates/zirk-ir/tests/lowering.rs`, and `crates/zirk-cli/tests/corpus/`, covering positive record/value class fields, nested fields, and negative temporary-root and escape cases.
- [x] 5.3 Define concrete test cases for `String[index]` in `crates/zirk-sema/tests/typing.rs`, `crates/zirk-ir/tests/lowering.rs`, and `crates/zirk-cli/tests/corpus/`, covering valid grapheme access, out-of-bounds, negative index, and rejected write assignment.
- [x] 5.4 Verify that every new CLI fixture has a matching `.out` file and that `crates/zirk-cli/tests/end_to_end.rs` passes all corpus fixtures.
- [x] 5.5 Run `cargo test -p zirk-sema`, `cargo test -p zirk-ir`, and `cargo test -p zirk-codegen-llvm` individually to isolate regressions during development.
- [x] 5.6 Confirm that `cargo clippy --workspace` produces no new warnings introduced by this change.
- [x] 5.7 Confirm that `cargo test --workspace` and `openspec validate phase-4e-pending-closeout --strict` pass before archiving.
