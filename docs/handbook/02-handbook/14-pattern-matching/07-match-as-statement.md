# `match` as a Statement

Statement-form `match` chooses effects or control flow and does not produce a result.

```zirk
match command {
    Start => service.start();
    Stop => service.stop();
    Status => show_status();
}
```

Exhaustiveness remains valuable even when the syntax context does not require a value. It prevents a new domain variant from being silently ignored. For intentionally open inputs, a wildcard can define the fallback.

Do not assign from statement form; use expression form so branch result types are checked.

---

**Previous:** [← Destructuring Patterns](./06-destructuring-patterns.md) · **Next:** [`match` as an Expression →](./08-match-as-expression.md)
