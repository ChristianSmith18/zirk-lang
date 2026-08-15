# Exceptions

Exceptions represent unusual but recoverable control transfer. They are not the default mechanism for ordinary absence or validation failure.

```zirk
try {
    execute();
} catch<HttpError> error {
    stderr.println(error);
}
```

An exception crosses frames until a compatible handler is found, while resource and `finally` cleanup still runs. Public APIs must document exceptions callers are expected to recover from.

---

**Previous:** [← Error Propagation](./03-error-propagation.md) · **Next:** [Typed `catch` →](./05-typed-catch.md)
