# Handling `Result`

Handle both variants with `match`:

```zirk
match load(path) {
    Ok(document) => render(document);
    Error(error) => report(error);
}
```

Expression form can convert both outcomes into one value. Exhaustiveness
prevents errors from being silently ignored. A `Result` expression used as an
ordinary discarded statement is a compile-time error:

```zirk
load(path);     // error: Result not handled
_ = load(path); // explicit discard; linter may request justification
```

---

**Previous:** [← Result](01-result.md) · **Next:** [ Error Propagation](03-error-propagation.md)
