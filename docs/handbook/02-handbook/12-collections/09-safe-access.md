# Safe Collection Access

Safe access represents a missing key or out-of-range position without undefined behavior. The collection API may return `T?` when absence needs no explanation or `Result<T, E>` when callers need a reason.

```zirk
inmut user: User? = users.get(requested_id);
inmut display = user?.name ?? "Unknown user";
```

Do not confuse receiver-safe `?.` with bounds-safe indexing: `items?.first` handles a nullable collection, while a collection lookup contract handles a missing element.

---

**Previous:** [← Slicing](./08-slicing.md) · **Next:** [Iteration and Functional Style →](../13-iteration-and-functional-style/README.md)
