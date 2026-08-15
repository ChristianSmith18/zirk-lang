# `for ... in`

`for ... in` visits values supplied by an iterable.

```zirk
for item in items {
    stdout.println(item.name);
}
```

The loop binding is scoped to the body. Iteration order, mutation rules, and whether values are borrowed or copied follow the iterable's public contract; they are not inferred from storage layout.

Use iterator transformations for a clear data pipeline and `for ... in` when the body performs control flow or several effects. A `break` stops iteration; `continue` requests the next value.

---

**Previous:** [← `for`](./05-for.md) · **Next:** [`while` →](./07-while.md)
