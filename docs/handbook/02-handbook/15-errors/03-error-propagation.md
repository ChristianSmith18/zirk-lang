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
exception. Use `or_throw` with an explicit mapper when a boundary needs that
change. A callback passed to `map`, `and_then`, or another combinator preserves
its declared `throws`; the combinator does not absorb the exception into `E`.

---

**Previous:** [← Handling Result](02-handling-result.md) · **Next:** [ Exceptions](04-exceptions.md)
