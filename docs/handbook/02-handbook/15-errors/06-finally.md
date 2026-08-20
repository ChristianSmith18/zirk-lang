# `finally`

`finally` runs when control leaves its `try`, whether by success, exception, or return.

Use it for local non-resource obligations that must always occur. External resources should implement `Resource<E>` and use `match with`, whose closure and close-error semantics are stronger and typed.

Cleanup failure must not silently replace the primary failure; the governing API contract determines how both are reported.

Direct `return`, `break`, `continue`, `throw`, or `fatalError` inside `finally`
cannot replace an active outcome. When cleanup throws during propagation, the
original throwable stays primary; `throw;` inside a catch preserves exact
identity and trace, and wrapping constructs a new throwable with the original
as `cause`.

> **Implementation status:** `suppressed` (recording a cleanup failure that
> occurs while another throwable is already propagating) is normative but not
> implemented — `Error` has no `suppressed` member yet, since it needs
> `List<T>`, a Phase 7 collection that does not exist. `stack_trace()` exists
> and is callable, but every override returns an empty `StackTrace` with no
> captured frames; real frame capture is unimplemented.

---

**Previous:** [← Typed catch](05-typed-catch.md) · **Next:** [ fatalError](07-fatal-error.md)
