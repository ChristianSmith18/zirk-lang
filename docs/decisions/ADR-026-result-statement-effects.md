# ADR-026: `Result` Statement Effects

- **Status:** Accepted
- **Date:** 2026-10-23
- **Decision owners:** Zirk language authors

## Context

The must-use rule for a `Result` in statement position forced callers to write
`_ =` even when the operation should simply succeed or fail the current flow.
This boilerplate obscures the operation and its real error-handling boundary.

## Decision

When a `Result<T,E>` expression is used as a standalone expression statement,
the compiler evaluates it as an effect. `Ok(value)` discards `value`;
`Error(error)` raises an exception. If `E` is a `Throwable`, the same throwable
is raised. For any other `E`, the runtime raises a catchable `NativeError`
whose reason is the error's string representation, or `Result failed` when
`E` is `Void`.

The exception follows ordinary Zirk exception propagation and can be handled
with `try/catch`. A direct `match` still exposes both variants without raising
an exception. Assignments, returns, parameters, and other value contexts keep
the existing contextual Result rules: explicit Result contexts preserve both
variants, and ordinary value contexts project to the nullable success value.
`_ = expression` remains the deliberate way to discard both success and
failure silently.

## Consequences

- Routine effectful operations no longer need `_ =` at every call site.
- Callers that need to recover from failure can use `try/catch`; callers that
  need typed failure detail can keep or match the Result directly.
- A non-Throwable error raised from statement position is represented by
  `NativeError`, so its original type information is available only when the
  Result is handled directly.
- ADR-024 remains authoritative for value projection and is superseded only
  for the old must-use rule on standalone Result statements.

## Alternatives Considered

### Require explicit `_ =` for every discarded Result

Rejected because it adds repeated syntax to ordinary effect calls and was the
source of the avoidable middleware and route-registration boilerplate.

### Convert every error to `RuntimeError`

Rejected because `NativeError` is the existing runtime representation for
native failures and carries a reason while remaining catchable as
`RuntimeError`.
