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

`.parallel` on a collection and `Parallel.each(coll, fn)` are reserved for a
per-element pipeline adapter and a parallel map, matching the sequence
methods (`map`/`filter`/`reduce`/`sum`/`count`/`collect`/`for_each`) that do
not exist on `List<T>`/`Array<T>` yet — sequentially or otherwise. `.parallel`
type-checks today, but only as an identity: it types and behaves exactly like
the collection itself, with no adapter behavior. `Parallel.each` type-checks
too, but has no lowering yet.

An associative-combiner check for a parallel reduction, and a
`reduce_ordered` deterministic escape, are follow-up work once a `reduce`
method exists to check.

---

**Previous:** [← parallel](08-parallel.md) · **Next:** [ Threads](10-threads.md)
