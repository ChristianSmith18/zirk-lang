# Error Propagation

Zirk 1.x has no `?` propagation operator. Preserve expected errors with an explicitly typed `Result` return and use direct `match` when inspecting or translating them. An ordinary value context instead projects a Result to `T?` and discards the error payload.

```zirk
return match load(path) {
    Ok(document) => Ok(transform(document));
    Error(error) => Error(error);
};
```

Translate errors at abstraction boundaries when callers need domain meaning; preserve the original cause for diagnostics where safe.

In a value context, there is no automatic conversion between `Result.Error`
and a thrown exception. In statement position, a failed Result throws its
`Throwable` error or wraps another error as `NativeError`. Use direct `match`
to handle or translate errors without throwing, or use `try/catch` to recover
from the statement effect. The composition operations are specified but not
implemented yet (see [Result](01-result.md)).

---

**Previous:** [← Handling Result](02-handling-result.md) · **Next:** [ Exceptions](04-exceptions.md)
