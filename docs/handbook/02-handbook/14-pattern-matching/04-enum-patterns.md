# Enum Patterns

Enum patterns select variants and bind associated values.

```zirk
match result {
    Ok(value) => consume(value);
    Error(error) => report(error);
}
```

For a closed enum, listing every variant makes the match exhaustive. Adding a new variant then reveals all decisions that need updating.

Bindings inherit the associated field types. A pattern with the wrong arity or field shape is rejected at compile time.

This match-only extraction rule prevents code from treating a payload as
present before the variant has been proven. Bound payloads are independent
projections, including nested reference-backed values.

---

**Previous:** [← Type Patterns](03-type-patterns.md) · **Next:** [ Union Patterns](05-union-patterns.md)
