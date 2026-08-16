# Handling `Result`

Handle both variants with `match`:

```zirk
match load(path) {
    Ok(document) => render(document);
    Error(error) => report(error);
}
```

Expression form can convert both outcomes into one value. Exhaustiveness prevents errors from being silently ignored. If an API intentionally discards a result, the linter may warn because ignored failure is usually a defect.

---

**Previous:** [← Result](01-result.md) · **Next:** [ Error Propagation](03-error-propagation.md)
