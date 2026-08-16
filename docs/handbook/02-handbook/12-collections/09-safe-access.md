# Safe Collection Access

Safe access represents a missing key or out-of-range position without undefined
behavior. Direct `[]` failure is a typed controlled error. `get` returns a
`Result<T, E>`; a family may additionally expose `get_or_null`.

```zirk
inmut user: User? = users.get_or_null(requested_id);
inmut display = user?.name ?? "Unknown user";
```

Do not confuse receiver-safe `?.` with bounds-safe indexing: `items?.first` handles a nullable collection, while a collection lookup contract handles a missing element.

---

**Previous:** [← Slicing](08-slicing.md) · **Next:** [Collection Contracts and Complexity →](10-collection-contracts-and-complexity.md)
