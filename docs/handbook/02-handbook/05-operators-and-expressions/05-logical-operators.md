# Logical Operators

`&&`, `||`, and `!` operate only on `Boolean` values.

```zirk
if authenticated && !suspended {
    open_dashboard();
}
```

`&&` and `||` short-circuit: the right operand is evaluated only when its value is needed. Code may rely on that control flow, but hiding large side effects in a condition reduces clarity.

Numeric and object truthiness do not exist. Compare explicitly or ask the type for a Boolean property.

---

**Previous:** [← Object Identity](04-object-identity.md) · **Next:** [ Ternary Operator](06-ternary.md)
