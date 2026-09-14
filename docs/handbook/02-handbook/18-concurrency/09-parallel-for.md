# `for` inside `parallel`

A `for x in coll { ... }` that fills the whole body of a `parallel { }`
region, over an `Array<T>`/`List<T>`, is split into one chunk per element and
run across the worker pool — the same loop written outside a `parallel`
region runs sequentially:

```zirk
parallel {
    for item in files {
        compress(item);
    }
}
```

The loop body must have no `return`, `break`, `continue`, or `throw`, and
must not reassign a name captured from the enclosing scope — a captured
scalar the body reassigns would otherwise mutate only that one chunk's own
private copy, silently going nowhere. A loop that does any of these still
compiles; it just runs sequentially inside the region instead of splitting
across the pool.

`.parallel` runs the supported eager sequence terminals (`map`, `filter`,
`for_each`, `reduce`, and `sum`) on the worker pool outside a region.
`Parallel.each(coll, fn)` is the ordered parallel-map spelling and returns a
`List<R>` in input order. `map` and `filter` preserve collection order; filter
compacts only after every predicate result is available.

Ordinary parallel `reduce` requires an associative lambda. Use
`reduce_ordered` when grouping must remain left-to-right; it is sequential by
definition.

---

**Previous:** [← parallel](08-parallel.md) · **Next:** [ Threads](10-threads.md)
