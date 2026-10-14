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

`.parallel` routes supported eager sequence terminals (`map`, `filter`,
`for_each`, associative `reduce`, and `sum`) through the worker pool outside a
region; `reduce_ordered` remains sequential. `map` and `filter` preserve input
order, with filter compaction after predicate results are available.
`Parallel.each(coll, fn)` is also lowered to the same worker pool and returns
its materialized `List<R>` in input order. It accepts the same CPU-only callback
constraints as the collection adapter.

Ordinary parallel `reduce` requires an associative lambda. Use
`reduce_ordered` when grouping must remain left-to-right; it is sequential by
definition.

---

**Previous:** [← parallel](08-parallel.md) · **Next:** [ Threads](10-threads.md)
