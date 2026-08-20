# Error Propagation

Zirk 1.x has no `?` propagation operator. Propagation is explicit through `match`, helper methods whose contracts are visible, or direct return of a compatible result.

```zirk
return match load(path) {
    Ok(document) => Ok(transform(document));
    Error(error) => Error(error);
};
```

Translate errors at abstraction boundaries when callers need domain meaning; preserve the original cause for diagnostics where safe.

There is no automatic conversion between `Result.Error` and a thrown
exception. The normative boundary for that change is `or_throw` with an
explicit mapper, and a callback passed to `map`, `and_then`, or another
combinator is meant to preserve its declared `throws` rather than have the
combinator absorb the exception into `E` — but `or_throw` and the generic
combinators (`map`, `map_error`, `and_then`, `or_else`) are not implemented
yet (see [Result](01-result.md)). Today, translate errors explicitly with
`match` at the boundary instead.

---

**Previous:** [← Handling Result](02-handling-result.md) · **Next:** [ Exceptions](04-exceptions.md)
