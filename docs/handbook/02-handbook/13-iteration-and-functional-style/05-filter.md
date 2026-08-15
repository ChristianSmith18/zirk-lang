# `filter`

`filter` retains elements for which a predicate returns `true`.

```zirk
inmut active = users.filter((user: User): Boolean => user.is_active);
```

The predicate must return `Boolean`; truthiness is not accepted. Filtering preserves element type but may change count and, where promised, preserves relative order.

Use a named predicate when the rule has domain meaning or deserves direct tests.

---

**Previous:** [← `map`](./04-map.md) · **Next:** [`reduce` →](./06-reduce.md)
