# Typed `catch`

`catch<Type> name` handles a compatible exception and narrows the binding to that type.

```zirk
catch<HttpError> error {
    retry(error.retry_after);
} default error {
    report(error);
}
```

Place specific recovery before a default handler. A handler should recover, translate, or rethrow deliberately; swallowing an exception without restoring invariants is unsafe.

---

**Previous:** [← Exceptions](04-exceptions.md) · **Next:** [ finally](06-finally.md)
