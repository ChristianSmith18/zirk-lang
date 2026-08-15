# Multiple Type Parameters

Use multiple parameters when an API relates independently varying types.

```zirk
fn map_value<T, U>(value: T, transform: (T): U): U {
    return transform(value);
}
```

Names such as `T` and `U` suit short mathematical relationships; domain APIs benefit from descriptive parameter names. Constraints may apply independently or express relationships among parameters.

Do not split one semantic type into several parameters merely to increase flexibility. Every new parameter expands the combinations callers and implementations must understand.

---

**Previous:** [← Constraints with `from`](./03-constraints-with-from.md) · **Next:** [Generic Inference →](./05-inference.md)
