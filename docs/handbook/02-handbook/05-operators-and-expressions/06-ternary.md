# Ternary Operator

The ternary operator selects one of two expressions:

```zirk
inmut label = enabled ? "enabled" : "disabled";
```

The condition must be `Boolean`, and the branch types must be compatible with a common result type. Only the selected branch is evaluated.

Use ternary syntax for a short, symmetric choice. Prefer an `if` expression when branches need multiple statements, names, or explanation.

---

**Previous:** [← Logical Operators](05-logical-operators.md) · **Next:** [ Increment and Decrement](07-increment-and-decrement.md)
