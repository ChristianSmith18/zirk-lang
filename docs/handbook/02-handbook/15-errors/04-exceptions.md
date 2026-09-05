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

## API

The contract implemented by every thrown value: deeply immutable reference
identities with stable `message`, `code`, `cause`, and `stack_trace`.

> **Delivery caveat:** `stack_trace()` exists. `suppressed` (recording a
> cleanup failure during a pending exception) is normative but not implemented
> — `Error` has no `suppressed` member yet because it needs `List<T>`.

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `e.message()` | `String` | Human-readable failure description | implemented |
| `e.code()` | `String` | Stable machine-readable code | implemented |
| `e.cause()` | `Throwable?` | Underlying cause, or `null` | implemented |
| `e.suppressed()` | `List<Throwable>` | Secondary failures recorded during cleanup | specified — pending `List<T>` |
| `e.stack_trace()` | `String` | Captured trace | implemented |
| `e.to_string()` | `String` | Default rendering | implemented |

Built-in safety failures are typed `RuntimeError` subclasses, catchable without
appearing in every signature; explicit `throw` must be caught or declared in a
`throws` set.

### Examples

```zirk
try {
    synchronize();
} catch NetworkError(error) {
    stdout.println("{error.code()}: {error.message()}");
    report(error.stack_trace());
} catch Throwable(error) {
    stdout.println("unhandled: {error.message()}");
}
```

---

**Previous:** [← Error Propagation](03-error-propagation.md) · **Next:** [ Typed catch](05-typed-catch.md)
