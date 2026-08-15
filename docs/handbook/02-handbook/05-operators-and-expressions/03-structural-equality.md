# Structural Equality

`==` and `!=` compare values according to their structural equality contract.

```zirk
if expected == actual {
    stdout.println("same value");
}
```

For composite values, structural equality concerns relevant contents rather than whether two references identify the same instance. A type's equality contract must remain consistent and cannot depend on unstable addresses.

Use `is` when identity itself is the question. Choosing `==` for identity-sensitive logic can accidentally equate distinct mutable entities with equal current fields.

---

**Previous:** [← Comparison](./02-comparison.md) · **Next:** [Object Identity →](./04-object-identity.md)
