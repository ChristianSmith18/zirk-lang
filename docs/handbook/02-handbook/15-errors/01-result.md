# `Result`

`Result<T, E>` represents either `Ok(T)` or `Error(E)` and keeps expected failure in the return type.

```zirk
fn load(path: String): Result<Document, LoadError> { /* ... */ }
```

Callers can see and compose the failure without hidden control transfer. Choose a specific error type that contains actionable context without exposing secrets.

---

**Previous:** [← Errors](./README.md) · **Next:** [Handling `Result` →](./02-handling-result.md)
