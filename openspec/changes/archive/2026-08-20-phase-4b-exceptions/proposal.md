## Why

`fase-4a-errores` covered the *expected*-failure half (`Result<T,E>`) of `ZIRK_LANGUAGE_SPEC.md` section 9. This phase covers the *extraordinary*-failure half: `throw`/`try`/`catch`/`finally`, the `Error`/`Throwable`/`RuntimeError` hierarchy, and `throws` in function signatures, verified by the checker (catch or declare).

The full exception mechanism (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3) is large: real unwinding through arbitrary stack frames, lazily-built structured stack traces, `suppressed` populated from a cleanup failure in `finally`, converting the existing native runtime failures (division by zero, overflow, invalid cast, out-of-range index) to catchable `RuntimeError`, and `Resource<E>`/`match with` (which depends on all of the above existing). Attempting all of it in a single change would risk the same two things that splitting off `fase-4a` already avoided: size and coupling.

This change builds the checkable *and executable* core — full syntax, the class hierarchy, "catch or declare" effect analysis, and a real propagation mechanism — without building the table-based stack unwinding (landing pads / `personality function`) that a production compiler would use: the checker's own "catch or declare" analysis already proves, at compile time, that every exception not caught locally is declared in every function along the call chain up to where it is caught. That makes a simpler, equally sound propagation mechanism possible — every function whose body can throw (transitively) returns, in addition to its ordinary value, an implicit "did I throw / did I not throw" outcome; every call site to such a function checks that outcome and, if it threw, jumps to the local `catch` that covers it or immediately re-propagates from the current function (decision D1 in `design.md`). It is not real native stack unwinding — it is an invisible `Result` the programmer never writes — but it is correct for every program the checker accepted, and it is the piece needed to say this "runs" rather than "only checks".

## What Changes

- New `throw`/`throws` keywords; `try`/`catch`/`finally` graduate from "reserved for Phase 4" to implemented.
- `throw expr;` (throws) and `throw;` (rethrows, valid only inside a `catch`).
- `try { } catch Type(name) { } ... finally { }` — at least one `catch` or one `finally`. Each `catch` tests by exact type or ancestor (no variant patterns yet — see "out of scope").
- `throws Type (| Type)*` on a function or method signature (not on a `Fn(...)` type: function type syntax does not exist yet — decision D9, `fase-4a-errores`).
- The compiler-known hierarchy `abstract class Error { ... }`, `abstract class Throwable implements Error { ... }`, `abstract class RuntimeError implements Throwable {}` (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3), registered the same way as `Result` — injected directly, not parsed from a user declaration.
- "Catch or declare" effect analysis: an exception thrown or propagated from a call to a `throws` function must be covered by a reachable `catch`, or the enclosing function must declare a `throws` type that covers it. A more general catch before a more specific one is an error (unreachable).
- Real execution of `throw`/`try`/`catch`/`finally` and of `throws` propagated between functions, on top of D1's propagation mechanism — a real compiled program runs, with correct output and exit code.
- `Throwable`'s `stack_trace()` returns a real but empty `StackTrace` (no frames) — the type exists and is callable, real context capture is out of scope.

### Explicitly out of scope

- **Structured stack traces, `suppressed` populated from a cleanup failure in `finally`.**
- **Converting the existing native runtime failures** (division by zero, overflow, invalid cast, out-of-range index) **into catchable `RuntimeError`** — they still abort the process as today; this is its own, separate change, touching every `zirk-runtime` site that calls `fatal()` today.
- **Variant patterns in `catch`** (`catch NetworkError.Timeout(duration)`) — needs a user exception to declare internal variants, a mechanism that does not exist; only `catch Type(name)` by class type.
- **`Fn(...) => T throws X`** — function type syntax does not exist (D9, `fase-4a-errores`).
- **`Resource<E>`/`match with`** — depends on exceptions actually running, not just type-checking.
- **Lowering to IR and codegen** (real unwinding) — gated behind `NOT_LOWERED`, a separate change after this one.

## Impact

- Specs affected: `zirk-errors` (implements the other three of the four requirements it already documents, except traces/suppressed), `zirk-type-system`, `zirk-grammar`, `zirk-lexical-syntax`.
- No breaking changes: nothing that compiles today stops compiling.
