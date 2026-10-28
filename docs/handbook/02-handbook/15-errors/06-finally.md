# `finally`

`finally` runs when control leaves its `try`, whether by success, exception, or return.

Use it for local non-resource obligations that must always occur. External resources should implement `Resource<E>` and use `match with`, whose closure and close-error semantics are stronger and typed.

Cleanup failure must not silently replace the primary failure; the governing API contract determines how both are reported.

Direct `return`, `break`, `continue`, `throw`, or `fatalError` inside `finally`
cannot replace an active outcome. When cleanup throws during propagation, the
original throwable stays primary; `throw;` inside a catch preserves exact
identity and trace, and wrapping constructs a new throwable with the original
as `cause`.

> **Implementation status:** `Throwable.suppressed()` and `stack_trace()` are
> delivered. The runtime links a newly thrown exception inside a `catch` to
> the exception being handled. `stack_trace()` currently returns and caches a
> placeholder string rather than captured frames. Automatic suppression of a
> resource close failure while a different exception is propagating remains
> specified work; see [resource close errors](../16-resources/04-close-and-flush-errors.md).

---

**Previous:** [← Typed catch](05-typed-catch.md) · **Next:** [ fatalError](07-fatal-error.md)
