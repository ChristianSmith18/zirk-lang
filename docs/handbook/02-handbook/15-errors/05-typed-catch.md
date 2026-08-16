# Typed `catch`

`catch Type(binding)` uses the same guard-free pattern model as `match` and
narrows the binding to the selected throwable type or variant.

```zirk
catch HttpError.Timeout(duration) {
    retry_after(duration);
} catch Throwable(error) {
    report(error);
}
```

Explicit declared exceptions must be caught exhaustively or propagated. Place
specific recovery before a general handler; an unreachable catch is a compile
error. `catch Throwable(error)` catches all recoverable throwables and `catch
RuntimeError(error)` only implicit safety failures. A handler recovers,
translates, or uses exact `throw;` deliberately.

---

**Previous:** [← Exceptions](04-exceptions.md) · **Next:** [ finally](06-finally.md)
