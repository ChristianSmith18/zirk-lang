# Typed `catch`

`catch Type(binding)` narrows the binding to the selected throwable class.

```zirk
catch NetworkError(error) {
    retry(error);
} catch Throwable(error) {
    report(error);
}
```

Explicit declared exceptions must be caught exhaustively or propagated. Place
specific recovery before a general handler; an unreachable catch is a compile
error. `catch Throwable(error)` catches all recoverable throwables and `catch
RuntimeError(error)` only implicit safety failures. A handler recovers,
translates, or uses exact `throw;` deliberately.

> **Implementation status:** matching is by class type only —
> `catch HttpError.Timeout(duration)`-style variant patterns are normative but
> not implemented, since a user exception class has no mechanism yet to
> declare internal variants for `catch` to match against.

---

**Previous:** [← Exceptions](04-exceptions.md) · **Next:** [ finally](06-finally.md)
