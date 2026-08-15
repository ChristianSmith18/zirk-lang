# `finally`

`finally` runs when control leaves its `try`, whether by success, exception, or return.

Use it for local non-resource obligations that must always occur. External resources should implement `Resource<E>` and use `match with`, whose closure and close-error semantics are stronger and typed.

Cleanup failure must not silently replace the primary failure; the governing API contract determines how both are reported.

---

**Previous:** [← Typed `catch`](./05-typed-catch.md) · **Next:** [`fatalError` →](./07-fatal-error.md)
