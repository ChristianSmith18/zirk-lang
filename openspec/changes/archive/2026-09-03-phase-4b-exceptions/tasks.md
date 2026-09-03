## 1. Register the two new native exception classes

- [x] 1.1 Add `ArithmeticOverflowError` and `InvalidCastError` to `NativeExceptions` in `crates/zirk-sema/src/checker.rs` and register them as concrete `RuntimeError` subclasses in `register_native_exception_hierarchy`, following the same `register_native_failure` pattern used for `DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, and `FloatNanError`.
- [x] 1.2 In `crates/zirk-ir/src/lower.rs`, extend the hand-synthesized method bodies (next to the existing `build_native_message`, `build_native_code`, `build_native_cause`, `build_native_stack_trace` helpers) to cover the two new classes. Confirm all six concrete exception classes still share field index `0` for `reason` and method indices `0..3` for `message`/`code`/`cause`/`stack_trace`.
- [x] 1.3 In `crates/zirk-sema/tests/typing.rs`, add cases confirming `catch ArithmeticOverflowError`, `catch InvalidCastError`, `catch RuntimeError`, and `catch Throwable` all type-check for the new classes.

## 2. Make arithmetic overflow catchable

- [x] 2.1 Add a new `InstKind::CheckedArithmetic { op, left, right, signed }` to `crates/zirk-ir/src/ir.rs` that returns a struct-like pair `{value, overflowed}` as two operands.
- [x] 2.2 Lower integer `+`, `-`, `*`, `/`, `%` in `crates/zirk-ir/src/lower.rs` to `CheckedArithmetic` when the target type is an integer; branch on the `overflowed` flag and call `throw_native_failure(arithmetic_overflow, ...)` on the failure block, then dispatch with `lower_pending_exception_dispatch`.
- [x] 2.3 In `crates/zirk-codegen-llvm/src/emit.rs`, implement emission for `InstKind::CheckedArithmetic` using LLVM's `llvm.*.with.overflow` intrinsics, returning the first result and the overflow flag.
- [x] 2.4 Remove or redirect the old `checked_arithmetic` and `checked_division` `trap_if` calls in `crates/zirk-codegen-llvm/src/emit.rs` so they no longer call `zirk_rt_overflow`; update `runtime.rs` accordingly.
- [x] 2.5 Add CLI fixtures and lowering tests for `try { ... } catch ArithmeticOverflowError { ... }` covering signed `+`, signed `*`, unsigned `+`, and `Int32.MIN / -1`.

## 3. Make invalid cast catchable

- [x] 3.1 In `crates/zirk-ir/src/lower.rs`, change lowering of `as` and `<T>` casts over class types to emit `InstKind::IsInstance` and branch on the result; on `false`, call `throw_native_failure(invalid_cast, "invalid cast")` and dispatch.
- [x] 3.2 Keep numeric/scalar `as` casts (e.g., `Int32` to `Int64`, `Int32` to `Float64`) as they are — only class/interface `as` casts become catchable.
- [x] 3.3 In `crates/zirk-codegen-llvm/src/emit.rs`, remove the `fatalError` call from the `CheckedCast` emission path and let `lower.rs` drive the failure path.
- [x] 3.4 Add CLI fixtures and lowering tests for `try { x as OtherClass } catch InvalidCastError { ... }` and `try { <OtherClass>x } catch InvalidCastError { ... }`.

## 4. Closeout

- [x] 4.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` to mark the `Catchable implicit native errors` row as `yes` for overflow and invalid cast, with a note referencing `fase-4b-excepciones`.
- [x] 4.2 Update `docs/init/ZIRK_ROADMAP.md` to mark Phase 4b catchable overflow/cast as delivered.
- [x] 4.3 Update `docs/handbook/13-appendices/07-current-limitations.md` to remove or rephrase the overflow/invalid-cast limitation.
- [x] 4.4 Run `cargo fmt --all`, `cargo clippy --workspace`, and `cargo test --workspace` with `LLVM_SYS_201_PREFIX` set; fix all failures.
- [x] 4.5 Run `openspec validate phase-4b-exceptions --strict`.

## 5. `Throwable.suppressed`

- [x] 5.1 Add `zirk_rt_suppressed` and `zirk_rt_set_suppressed` runtime symbols in `crates/zirk-runtime/src/exceptions.rs` and per-exception metadata maps.
- [x] 5.2 Declare the symbols in `crates/zirk-codegen-llvm/src/runtime.rs` and add `InstKind::Suppressed` / `InstKind::SetSuppressed` in `crates/zirk-ir/src/ir.rs`.
- [x] 5.3 Emit LLVM for `Suppressed` and `SetSuppressed` in `crates/zirk-codegen-llvm/src/emit.rs`.
- [x] 5.4 Type-check `Throwable.suppressed()` as `Throwable?` and `throw` inside a `catch` as setting the suppressed cause in `crates/zirk-sema/src/checker.rs` and `crates/zirk-ir/src/lower.rs`.
- [x] 5.5 Add `valid/throwable_suppressed.zrk` and `.out` fixture, and exercise it in `main2.zrk`.

## 6. Lazy `Throwable.stack_trace()`

- [x] 6.1 Implement `zirk_rt_stack_trace` in `crates/zirk-runtime/src/exceptions.rs`, building and caching a `String` on the first call.
- [x] 6.2 Add `InstKind::StackTrace`, declare `zirk_rt_stack_trace` in codegen, and emit the runtime call.
- [x] 6.3 Type-check `Throwable.stack_trace()` as `String` and lower it as a runtime intrinsic.
- [x] 6.4 Add `valid/lazy_stack_trace.zrk` and `.out` fixture, and exercise it in `main2.zrk`.

## 7. Deep immutability of thrown objects in `catch`

- [x] 7.1 Bind caught exceptions with `Mutability::Strict` in `crates/zirk-sema/src/checker.rs`.
- [x] 7.2 Extend `check_strict_alias` and `check_writable_field` to reject mutable aliases and writes through projections rooted in a caught exception.
- [x] 7.3 Add `valid/thrown_object_read_only.zrk` and `invalid/mutate_thrown_object.zrk` fixtures.

## 8. Final validation

- [x] 8.1 Run `cargo fmt --all`, `cargo clippy --workspace`, and `cargo test --workspace` with `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20`; fix regressions.
- [x] 8.2 Run `openspec validate phase-4b-exceptions --strict`.
- [x] 8.3 Update `main2.zrk` with `Throwable.suppressed()` and `stack_trace()` examples.
