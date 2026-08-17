# `parallel for`

```zirk
mut squares = parallel for value in 0..1000 { value * value };
```

Iterations must be safely independent or use explicit reduction primitives. Result ordering follows the operation contract, not physical completion order.

Parallel reductions require an associative combiner because the runtime may
regroup inputs. Floating results may therefore differ slightly from sequential
left-to-right rounding. Use the deterministic reduction variant when grouping
is observable.

---

**Previous:** [← parallel](08-parallel.md) · **Next:** [ Threads](10-threads.md)
