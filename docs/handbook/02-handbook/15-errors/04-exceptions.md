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
subclasses and remain catchable without appearing in every signature. An
exception crosses frames while resource and `finally` cleanup still runs.

> **Implementation status:** declared exception sets participating in `Fn`
> compatibility is normative but not applicable yet — `Fn(...) => T` has no
> writable syntax in Zirk today (function-type annotations are rejected by
> the parser on purpose, decision D9); a closure's type is inferred locally
> and cannot be written as a parameter, return, or field type.

---

**Previous:** [← Error Propagation](03-error-propagation.md) · **Next:** [ Typed catch](05-typed-catch.md)
