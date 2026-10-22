# Safe Collection Access

Safe access represents a missing key or out-of-range position without undefined
behavior. Direct `[]` failure is a typed controlled error. `get` returns a
`Result<T, E>`. A nullable value context projects that Result when a caller
intentionally treats every lookup error as absence.

```zirk
inmut user: User? = users.get(requested_id);
inmut display = user?.name ?? "Unknown user";
```

Replace `get_or_null(index)` or `get_or_null(key)` calls with `get`. Use the
nullable projection or `?? fallback` when absence is expected; keep an
explicit `Result<T, E>` and match `Ok`/`Error` when the lookup error matters.

Do not confuse receiver-safe `?.` with bounds-safe indexing: `items?.first` handles a nullable collection, while a collection lookup contract handles a missing element.

---

**Previous:** [← Slicing](08-slicing.md) · **Next:** [Collection Contracts and Complexity →](10-collection-contracts-and-complexity.md)
