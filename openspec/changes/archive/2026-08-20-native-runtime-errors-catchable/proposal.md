## Why

`fase-4b-excepciones` built `throw`/`try`/`catch`/`finally` with real execution, but deliberately left out an asymmetry documented in its own `design.md` (decisions D5–D7): the five native safety failures (division by zero, overflow, invalid cast, invalid shift, invalid repeat, `NaN`) still abort the process via `fatalError`, instead of producing a catchable `RuntimeError` as `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3 explicitly requires ("can be caught but need not be listed in every function signature"). A program that today writes `try { 1 / 0; } catch DivisionByZeroError(e) { ... }` does not compile that expectation: the checker lets it through (nothing tells it `DivisionByZeroError` does not exist as a real exception) and at runtime the process still aborts without going through the `catch`.

D7 already split the work into a tractable slice and one that needs its own spike: the four zero/negative/range checks (division by zero, invalid shift, invalid repeat, `NaN`) can be moved to `zirk-ir` reusing `fase-4b`'s D1–D3 mechanism as-is; the fifth, overflow, needs to first resolve the multi-result IR instruction question (`{value, overflowed}`) that `checked_arith` uses today only at the LLVM level, and is out of scope for this change.

## What Changes

- Four new concrete native classes, injected with the same "direct tables" mechanism as `Error`/`Throwable`/`RuntimeError` (`fase-4b`, D4) but **concrete and instantiable**, not abstract: `DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError` — all four `implements RuntimeError`, each with real, callable `message()`/`code()`/`cause()`/`stack_trace()` (not the `UNREACHABLE_ABSTRACT_METHOD` stub).
- The four corresponding checks move from `zirk-codegen-llvm/src/emit.rs` (`checked_division`, `checked_shift`, `check_not_nan`, and the `zirk_rt_invalid_repeat` check currently in `zirk-runtime/src/string.rs`) to `zirk-ir/src/lower.rs`, building the exception and calling the existing D1–D3 mechanism (`zirk_rt_throw` + `lower_pending_exception_dispatch`) before emitting the real arithmetic/repeat instruction — the same way an explicit `throw` is lowered today.
- `Lowering::lower_throws_check` now runs after **every** call, not only those declaring `throws` (D6) — an implicit native failure never appears in a `throws` by design (`ERROR_RESOURCE_PERMISSION_SEMANTICS.md` states this explicitly), so no static analysis of the callee's declared `throws` is enough to decide whether to check the pending slot after the call.
- `catch RuntimeError(e)` and `catch Throwable(e)` catch the four new exceptions the same as any other; a program that neither catches nor declares them (declaring is not required — they are implicit) still aborts if they escape `main`, via the same `zirk_rt_uncaught_exception` from `fase-4b`.

### Explicitly out of scope

- **`OverflowError`** — needs the multi-result IR instruction question resolved first (D5/D7). Still aborts via `fatalError`, unchanged.
- **Invalid cast** (`InvalidCastError`) — not in the tractable list D7 identified; the check lives in a different codegen path (`zirk_rt_check_cast`) not investigated in this pass. Still aborts.
- **`suppressed` populated from a cleanup failure in `finally`** — depends on `List<T>` (Phase 7), unchanged from `fase-4b`.
- **Real stack traces** (`stack_trace()` still returns an empty `StackTrace`, with no frames) — unchanged from `fase-4b`.

## Impact

- Specs affected: `zirk-errors` (closes the "can be caught" asymmetry that `fase-4b-excepciones` left documented but unimplemented for these four).
- Execution cost: one `zirk_rt_has_pending_exception` plus a branch, at every call site of the compiled program (D6) — previously, only at calls whose target declared `throws`.
- No breaking changes: no program that compiles today stops compiling; the four checks still abort the process if nobody catches them, exactly as before — only now an explicit `catch` can intercept them.
