# Exceptions

Exceptions represent unusual but recoverable control transfer. They are not the default mechanism for ordinary absence or validation failure.

```zirk
fn synchronize(): Void throws NetworkError | StorageError { ... }

try {
    synchronize();
} catch NetworkError(error) {
    retry(error);
} catch StorageError(error) {
    report(error);
}
```

An explicit `throw` must be caught or declared; public APIs write the complete
explicit set. Built-in safety failures belong to typed `RuntimeError`
subclasses and remain catchable without appearing in every signature. Declared
exception sets participate in `Fn` compatibility. An exception crosses frames
while resource and `finally` cleanup still runs.

---

**Previous:** [← Error Propagation](03-error-propagation.md) · **Next:** [ Typed catch](05-typed-catch.md)
