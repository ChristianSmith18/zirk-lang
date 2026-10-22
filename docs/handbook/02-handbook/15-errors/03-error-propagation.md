# Error Propagation

Zirk 1.x has no `?` propagation operator. Preserve expected errors with an explicitly typed `Result` return and use direct `match` when inspecting or translating them. An ordinary value context instead projects a Result to `T?` and discards the error payload.

```zirk
return match load(path) {
    Ok(document) => Ok(transform(document));
    Error(error) => Error(error);
};
```

Translate errors at abstraction boundaries when callers need domain meaning; preserve the original cause for diagnostics where safe.

There is no automatic conversion between `Result.Error` and a thrown
exception. The specified `or_throw` boundary requires an explicit mapper, and
a callback passed to a Result composition operation preserves its declared
`throws` effect instead of absorbing the exception into `E`. The composition
operations are specified but not implemented yet (see [Result](01-result.md)).
Translate errors explicitly with `match` at the boundary instead.

---

**Previous:** [← Handling Result](02-handling-result.md) · **Next:** [ Exceptions](04-exceptions.md)
