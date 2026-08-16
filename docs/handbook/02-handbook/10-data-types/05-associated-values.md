# Associated Values

Associated values attach payloads to the enum variant for which they are valid.

```zirk
match state {
    Ready(document) => render(document);
    Failed(error) => report(error);
    Idle => show_idle();
    Loading(progress) => show_progress(progress);
}
```

Bindings receive the declared payload types and remain scoped to their branch. A pattern with the wrong payload shape is a compile-time error.

Associated values are preferable to a class with several nullable fields because invalid cross-variant combinations cannot be constructed.

---

**Previous:** [← Algebraic Enums](04-algebraic-enums.md) · **Next:** [ Type Aliases](06-type-aliases.md)
