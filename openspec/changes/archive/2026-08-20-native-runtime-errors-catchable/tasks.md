## 1. Checker — registration of the four concrete classes

- [x] 1.1 `Checker::register_native_exception_hierarchy` (or a new helper called from the same site): registers `DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError` as concrete (not abstract) classes, each `implements RuntimeError`, with a `reason: String` field and the four `Error`/`Throwable` methods (`message`, `code`, `cause`, `stack_trace`) populated in their table — the same aligned-index mechanism D4 already uses so `catch RuntimeError(e)` dispatches correctly.
- [x] 1.2 Confirm (test) that `catch DivisionByZeroError(e)`, `catch RuntimeError(e)`, and `catch Throwable(e)` type-check against the four new classes the same as against any user `RuntimeError`.
- [x] 1.3 Tests: one case per class confirming it is instantiable only by the compiler (a user cannot write `DivisionByZeroError("x")` by hand — consider whether this needs an explicit rejection or whether it is enough that no construction syntax exposes the constructor; document the decision taken in `design.md` if it differs from what is already written)

## 2. IR — hand-synthesized method bodies (D8)

- [x] 2.1 In `zirk-ir::lower()`, alongside where `UNREACHABLE_ABSTRACT_METHOD` is synthesized, hand-build the IR functions for `construct`/`message`/`code`/`cause`/`stack_trace` for the four classes (16 trivial bodies) — each a single block, with no branches except where the body itself requires one (none of the five need a branch)
- [x] 2.2 Verify with `zirk_ir::verify` that these hand-built bodies pass without error (correct field indices, correct return types)
- [x] 2.3 Shared text constants for the four `reason`/`code()` values (see Risks in `design.md` about duplication with `zirk-runtime/src/failure.rs`)

## 3. IR — the four checks moved from codegen (D10)

- [x] 3.1 Division by zero: check moved from `checked_division` (`zirk-codegen-llvm/src/emit.rs`) to `zirk-ir/src/lower.rs`, before lowering `BinaryOp::Div`/`Mod` on integers — builds `DivisionByZeroError`, calls `zirk_rt_throw`, runs `lower_pending_exception_dispatch`
- [x] 3.2 Invalid shift: same pattern, moved from `checked_shift`
- [x] 3.3 Invalid repeat: moved from `zirk_rt_invalid_repeat` (`zirk-runtime/src/string.rs`) — the count check moves to `zirk-ir`, before the call to the runtime function that performs the repeat; confirm whether `str_repeat` in the runtime needs to keep its own check as a safety net or can assume an already-validated count (document the decision)
- [x] 3.4 `NaN`: moved from `check_not_nan` — unlike the other three, the check runs *after* emitting the `Float` operation, on the result
- [x] 3.5 Simplify `checked_division`/`checked_shift`/`check_not_nan` in `emit.rs` to only emit the operation (remove `trap_if` from all three); confirm whether `trap_if` still has other callers (invalid cast, missing contract, failed allocation) before considering removing the function itself — it likely remains in use, so do not remove the function
- [x] 3.6 Manual tests (a real `.zrk` compiled and run, not just unit tests) for each of the four: `try { 1 / 0; } catch DivisionByZeroError(e) { ... }` running the `catch`, with verified output

## 4. Unconditional `lower_throws_check` (D11)

- [x] 4.1 `lower_throws_check`/`call_throws` run after every call (direct function, method, contract method, and their statement-position counterparts), not only when the target's declared `throws` is non-empty
- [x] 4.2 Confirm this does not break any existing test due to unexpected cost/side effect (a call inside a function with no active `try` and no `throws` of its own now also checks the pending slot and must correctly re-throw/propagate without a local `try_stack`)
- [x] 4.3 Test: a native failure inside a function `g()` called by `f()`, with neither declaring `throws` nor having its own `try`, caught by a `try`/`catch` in `f()`'s caller — confirms D11 makes the implicit failure observable across two levels of calls

## 5. Closeout

- [x] 5.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` all green, with `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20` exported
- [x] 5.2 Valid corpus: at least one `.zrk` catching each of the four failures, and one that leaves one uncaught and confirms the process still aborts with a nonzero exit code (uncaught behavior unchanged)
- [x] 5.3 `design.md`: `## Decisions` section revised after real implementation (the same way `fase-4b` replaced its originally documented propagation mechanism with the one actually built) — update D8/D10/D11 if the real implementation differs from what is written here
- [x] 5.4 Confirm that no existing test relying on the direct abort of these four failures (search for `division by zero`, `shift`, `repeat`, `NaN` in `zirk-runtime`/`zirk-codegen-llvm` tests) broke because of the behavior change — if any assumed `fatalError`'s exact message, update it
