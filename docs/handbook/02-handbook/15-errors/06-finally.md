# `finally`

`finally` runs when control leaves its `try`, whether by success, exception, or return.

Use it for local non-resource obligations that must always occur. External resources should implement `Resource<E>` and use `match with`, whose closure and close-error semantics are stronger and typed.

Cleanup failure must not silently replace the primary failure; the governing API contract determines how both are reported.

Direct `return`, `break`, `continue`, `throw`, or `fatalError` inside `finally`
cannot replace an active outcome. When cleanup throws during propagation, the
original throwable stays primary and cleanup is appended to `suppressed`.
`throw;` inside a catch preserves exact identity and trace; wrapping constructs
a new throwable with the original as `cause`. Traces materialize lazily and do
not capture locals or secrets by default.

---

**Previous:** [← Typed catch](05-typed-catch.md) · **Next:** [ fatalError](07-fatal-error.md)
