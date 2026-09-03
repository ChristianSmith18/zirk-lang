## Context

The four earlier catchable native failures (`DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError`) were delivered by `fase-4d-runtimeerror`. They moved the actual comparisons out of `zirk-codegen-llvm/src/emit.rs` and into `zirk-ir/src/lower.rs`, where a failing check builds the exception with `build_native_failure` and throws it with `throw_native_failure` — the same path `throw expr;` uses. That lets the exception flow through the existing `try`/`catch`/`finally` mechanism without any new runtime unwinding.

The two remaining checks (`arithmetic overflow` and `invalid cast`) are harder because they happen in codegen: `checked_arithmetic` in `emit.rs` uses LLVM's `llvm.*.with.overflow` intrinsics for the flag, and `CheckedCast` uses `zirk_rt_check_cast` for the runtime descriptor test. This change adds the right IR-level hooks so the exception can be thrown and dispatched from `lower.rs` just like the others.

## Goals / Non-Goals

**Goals:**

1. Make integer arithmetic overflow throw `ArithmeticOverflowError` and runtime `as`/`<T>` cast failure throw `InvalidCastError`, both catchable under `RuntimeError` or `Throwable` without a `throws` declaration.
2. Preserve the existing behavior for programs that do not catch these exceptions (they still terminate with a nonzero exit).
3. Add tests and fixtures covering the catchable cases and the uncaught cases.

**Non-Goals:**

- `Throwable` metadata (`suppressed`, lazy stack traces, deep immutability) — these need `List<T>` and other runtime pieces that are not yet available.
- Phase 4c resource completion, 4d callable polymorphism, or 4e memory primitives.
- Phase 5 concurrency primitives (`Task`, `Channel`, `Atomic`, `Mutex`, etc.).
- New syntax or user-facing keywords.

## Decisions

### 1. Catchable overflow

**Decision:** Add `ArithmeticOverflowError` as a concrete `RuntimeError` class with the same `reason: String` + four methods shape as the four existing catchable native classes. For the check itself, move overflow detection from `emit.rs` into the IR by lowering integer `+`, `-`, `*`, `/`, `%` (when not already guarded) to a new `InstKind::CheckedArithmetic` sequence: `Add`/`Sub`/... produces a flag. `lower.rs` then branches on the flag and calls `throw_native_failure` on overflow.

**Rationale:** The earlier four checks were moved to `lower.rs` for the same reason — `emit.rs` cannot dispatch to `catch` blocks. `CheckedArithmetic` keeps the LLVM intrinsic in codegen but exposes the overflow bit to `lower.rs` as an ordinary `Boolean` operand.

**Alternatives considered:**
- Keep the overflow check in `emit.rs` and call a runtime helper that sets the pending exception. Rejected because `emit.rs` has no way to run `finally`/`catch` dispatch; the pending exception would only be noticed at the next call site.
- Use `Result` for overflow. Rejected because the language's native safety model is exception-based.

### 2. Catchable invalid cast

**Decision:** Add `InvalidCastError` as a concrete `RuntimeError` class. Replace the `zirk_rt_check_cast` runtime call (which aborts) with `zirk_rt_is_instance` followed by an IR branch: if the value is not an instance, throw `InvalidCastError`. This requires `InstKind::CheckedCast` to be split into `IsInstance` + a user-level branch in `lower.rs`, which is already what `catch` does.

**Rationale:** `zirk_rt_is_instance` already exists and answers the same question without terminating. The cast succeeds when it returns `true`; when `false`, the same dispatch machinery as the other native failures handles the throw.

**Alternatives considered:**
- Change `zirk_rt_check_cast` to return a boolean. Rejected because it is used in `emit.rs` where there is no exception dispatch context.
- Add a `Throw` variant to `InstKind::CheckedCast`. Rejected because the dispatch needs the same `try_stack`/`finally` narrowing already implemented for `throw`.

### 3. Runtime helper for class construction

**Decision:** Use the existing `build_native_failure` IR sequence (`Alloc` + `StoreField` of the `reason` string) in `lower.rs`, the same way the four earlier classes do. No new `zirk-runtime` entry point is needed: the exception object is a normal managed object and `zirk_rt_throw` stores it in the pending-exception slot.

**Rationale:** This reuses the exact proven pattern from `fase-4d-runtimeerror` and keeps `zirk-runtime` unchanged except for the removed `fatalError` call.

## Risks / Trade-offs

- `[Risk]` Moving overflow detection from `emit.rs` to `lower.rs` may regress performance or complicate constant folding. `Mitigation`: keep the LLVM intrinsic in `emit.rs` for `CheckedArithmetic`; `lower.rs` only emits the branch and the throw.
- `[Risk]` `CheckedCast` is currently used for `as`/`<T>` casts and for checked contract conformance. `Mitigation`: only replace `as`/`<T>` with the new `IsInstance` + branch path; contract-table checks remain as `zirk_rt_check_cast` because they are internal invariants.
- `[Risk]` Catchable overflow/cast may change observable behavior of programs that previously aborted. `Mitigation`: document this as an intended semantic change; update any existing tests that relied on `fatalError` messages.
