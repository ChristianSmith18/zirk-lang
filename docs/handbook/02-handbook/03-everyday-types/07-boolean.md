# Boolean

`Boolean` has exactly two values: `true` and `false`. Conditions and logical operators require Boolean operands.

```zirk
inmut ready: Boolean = items.count > 0;
if ready {
    process(items);
}
```

Zirk has no numeric, string, collection, or object truthiness. This is invalid:

```zirk
if items.count { /* ... */ }
```

Compare explicitly, such as `items.count > 0`. `Boolean?` can additionally hold `null`, but it cannot be used as a condition until narrowed or given a fallback.

---

**Previous:** [← Overflow and Arithmetic Safety](./06-overflow-and-arithmetic-safety.md) · **Next:** [Char →](./08-char.md)
