# `parallel for`

```zirk
mut squares = parallel for value in 0..1000 { value * value };
```

Iterations must be safely independent or use explicit reduction primitives. Result ordering follows the operation contract, not physical completion order.

---

**Previous:** [← parallel](08-parallel.md) · **Next:** [ Threads](10-threads.md)
