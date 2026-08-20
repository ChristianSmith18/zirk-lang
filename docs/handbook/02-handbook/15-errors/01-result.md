# `Result`

`Result<T, E>` represents either `Ok(T)` or `Error(E)` and keeps expected failure in the return type.

It provides Zirk's algebraic equivalent of Go's visible `value, err` outcome:
exactly one variant exists, so “both present” and “neither present” are
impossible. Any `E` is valid; it need not implement the throwable `Error`
contract.

```zirk
fn load(path: String): Result<Document, LoadError> { /* ... */ }
```

Callers can see and compose the failure without hidden control transfer. Choose a specific error type that contains actionable context without exposing secrets.

> **Implementation status:** the current checker recognizes seven structural
> methods — `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`,
> `unwrap`, and `unwrap_error`. `get_or_else`, `map`, `map_error`, `and_then`,
> `or_else`, and `or_throw` are normative but not yet implemented: each needs
> its own method-level type parameter (`map<U>(transform: Fn(T) => U)`), and
> `MethodInfo` has no such field today — only the containing class/contract/enum
> can be generic. `get_or_else` additionally needs `Fn` as a writable parameter
> type, which the language does not have yet.

Wrong-variant unwrap is a programmer assertion and invokes `fatalError`, not a
recoverable exception.

---

**Previous:** [← Errors](README.md) · **Next:** [ Handling Result](02-handling-result.md)
