## Context

The current language contract treats `Result<T,E>` as must-use and requires callers to unpack it through `match` or helper methods. The proposed change touches type checking, generic argument inference, lowering, printing/interpolation, the standard-library surface, normative documentation, and the compiler status inventory. See `proposal.md` and the delta specs for the observable contract.

## Goals / Non-Goals

**Goals:**
- Make the preserved-Result versus projected-nullable choice deterministic from the resolved source context.
- Ensure projection runs exactly once and has identical behavior on supported native targets.
- Preserve `match` as the exhaustive, lossless path to error details.
- Remove redundant extraction APIs and update all examples that rely on them.
- Make the breaking behavior and migration path visible in ADRs and public documentation.

**Non-Goals:**
- Introduce a propagation operator, exception conversion, or a new `Result` representation.
- Change the meaning of `throws`, `RuntimeError`, `fatalError`, or `??` on ordinary nullable values.
- Project a nested Result recursively or change explicit member invocation on a statically Result-typed receiver.
- Make standalone Result expression statements silently acceptable; explicit discard remains `_ = expression`.

## Decisions

### 1. Resolve the context before projecting

The checker will first identify the complete value-demanding context and resolve its declared or inferred expected type. It will preserve the source expression when that context requires `Result<T,E>` or when the expression is the direct operand of `match`. Otherwise, a Result expression projects once to `T?`. This is a compiler-known conversion equivalent to an internal `to_value_or_null()` operation, not a user-dispatched method and not a general implicit-conversion hook.

For a generic call, explicit type arguments and constraints are resolved first. If the instantiated parameter is `Result<T,E>`, the argument remains intact; for an ordinary generic parameter inferred from an argument, inference sees the projected `T?`. This makes `consume_result<Result<T,E>>(value)` and `consume(value)` observably distinct without making `Result` inference depend on incidental overload search. Zirk has no function overloading, so there is one selected signature.

The checker will reject ambiguous cases rather than silently choose a lossy projection. Direct `match` is syntactic and limited to the Result expression being matched; assigning a call to an unannotated binding first creates `T?`, so matching that binding handles nullable cases instead. An explicit Result annotation is the stable way to retain the original error for later inspection.

### 2. Keep the projection lossy and consistent

`Ok(value)` yields `value`; every `Error(error)` yields `null`, with no inspection of the error type. This intentionally means `Env.get<T>(...) ?? fallback` uses the fallback for absence, parse errors, permission denial, and host failures alike. Programs that must treat denial differently must write a direct match or preserve `Result<T,E>` in an explicitly typed variable. The conversion cannot throw, call `fatalError`, or synthesize an exception.

Projection applies only to the outer Result. `Result<Result<T,E1>,E2>` becomes `Result<T,E1>?`; the successful inner Result is not recursively transformed. Direct member access on an explicitly Result-typed receiver continues to resolve against `Result`, while assignment, argument, return, print, and interpolation contexts follow their resolved expected types.

### 3. Preserve must-use at statement boundaries

A bare Result-producing call used as a standalone expression statement remains rejected, since it has no value-demanding context and otherwise would become a loophole for silently ignoring expected failures. `_ = expression` remains the explicit discard form. The change affects value consumption, not the standalone statement rule.

### 4. Remove extraction APIs, retain composition

Remove the Result helpers whose meaning is superseded by `match` or projection: `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, and `unwrap_error`. Remove `Env.get_or_null` and `Env.get_or`; `Env.get` remains the one read operation returning a typed Result. Keep `map`, `map_error`, `and_then`, `or_else`, and `or_throw`, since they transform or compose outcomes without discarding the Result distinction.

This is a deliberate public API break. Collection APIs such as `List.get_or_null` are separate nullable collection lookup contracts and are not removed. Migrations use direct match when handling detail is required and `Env.get(...) ?? fallback` for a fallback that intentionally treats all errors alike.

### 5. Render at value sinks

`print`, `println`, and interpolation are value-demanding sinks, so a Result expression there projects before nullable `to_string()` rendering. Existing nullable rendering defines absence as `<Null>`. Explicit `result.to_string()` remains a deliberate request for the Result enum's diagnostic representation, preserving existing `Ok(...)`/`Error(...)` formatting. This distinction will be documented and covered by tests to prevent the generic print pathway from accidentally selecting a different behavior.

### 6. Update the compiler in semantic and lowering layers

Semantic analysis must annotate the chosen projection and nullable result type in the typed expression representation; lowering must consume that annotation to branch on `Ok`/`Error` and produce either the payload or the existing nullable null representation. The compiler must not infer this conversion later from source spelling, runtime behavior, or a method name. The implementation audit must trace the same typed projection through interpreter/browser IR and native LLVM lowering, or accurately retain any target limitation in status documentation.

The implementation work will locate and update Result method recognition in `crates/zirk-sema`, projection representation and transformation in `crates/zirk-ir`, and the native/runtime paths required by the existing target matrix. All user-written Result methods removed from the checker must receive stable diagnostics rather than falling through to a confusing missing-member error.

### 7. Record the language decision and publish one consistent contract

Add `docs/decisions/ADR-024-contextual-result-value-projection.md` to record the two-context model, intentional error-detail loss, preserved explicit path, helper removals, and considered alternatives. Update `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`, core and language specifications, the standard-library specification, the Result/environment handbook, examples and status/limitations. Treat those docs as normative sources; derive compiler status from verified implementation, not from the accepted proposal.

The companion `../zirk-lang-site` is part of the same public change. After the Zirk documentation is committed, sync the website from that exact clean commit, inspect its generated and site-owned status changes, and validate both repositories.

## Risks / Trade-offs

- **Projection can conceal an operational failure** → Make every `Error`-to-`null` conversion explicit in the language contract and environment docs; retain direct `match` and explicit Result annotations as lossless paths.
- **Context-sensitive typing may surprise generic callers** → Specify expected-type and inference ordering, test explicit and inferred generic calls, and emit a diagnostic when the context cannot be resolved safely.
- **Compiler target paths may diverge** → Carry projection as typed IR semantics and validate interpreter/browser/native behavior independently; do not claim target parity without evidence.
- **Removing established methods breaks source compatibility** → Include an explicit migration table and diagnostics; keep composition methods that preserve Result distinctions.
- **Print behavior differs from explicit `to_string()`** → Document that print/interpolation are projecting value contexts while direct `to_string()` preserves a statically Result-typed receiver; add paired tests.
- **Generated public site may lag the compiler contract** → Make website synchronization, diff review, and validation release tasks rather than optional follow-up.

## Migration Plan

1. Implement the type-system and lowering behavior behind compiler conformance tests while preserving the existing behavior until both are internally consistent.
2. Remove the Result and Env convenience methods in the same release that enables projection; update all in-repository callers and examples in one change.
3. Publish ADR-024 and synchronized normative documentation with migration guidance; update feature status only after the supported target paths are verified.
4. Commit the authoritative Zirk change, run the documented website sync against that commit, review the website diff, then validate and commit the companion repository separately.
5. Rollback consists of reverting the compiler behavior and restoring the previous method surface and normative contract together. Do not roll back only the documentation or only the compiler because that would leave source semantics contradictory.

## Open Questions

None. The context rules, error-loss policy, removals, standalone discard rule, and print rendering are fixed by the proposal and delta specifications.
