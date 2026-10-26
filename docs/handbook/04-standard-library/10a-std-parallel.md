# Parallel CPU operations

CPU-bound collection operations run across a runtime-managed worker pool.
They are distinct from task-aware I/O and from explicit OS threads. The
available source surface is the `.parallel` adapter and `Parallel.each`. This
chapter describes compiler/runtime features; it is not an importable
`std.parallel` module.

```zirk
inmut::strict values: List<Int32> = [1, 2, 3, 4];
inmut doubled: List<Int32> = values.parallel.map((n: Int32): Int32 => n * 2);
inmut even: List<Int32> = doubled.parallel.filter((n: Int32): Boolean => n % 4 == 0);
values.parallel.for_each((n: Int32): Void => { });
inmut total: Int32 = values.parallel.reduce(0,(a: Int32, b: Int32): Int32 => a + b);
inmut shifted: List<Int32> = Parallel.each(values,(n: Int32): Int32 => n + 1);
```

`map`, `filter`, `Parallel.each`, and `reduce` preserve input order in their
materialized results. `for_each` may perform independent effects in completion
order. `reduce` requires an associative reducer because grouping is
implementation-dependent; `reduce_ordered` stays sequential. `sum` is also
available. The language surface does not provide `parallel.settled`; ordered
settled outcomes exist only in Rust runtime internals, not as a Zirk API.

The runtime selects worker count and minimum useful workload automatically,
reuses one pool and flattens nested parallelism to prevent oversubscription.
These operations use the runtime worker pool and follow the ownership and
callback restrictions enforced by the compiler. They do not provide a general
shared-mutable-state synchronization API; language-level locks and atomics are
still pending.

---

**Previous:** [← std.sync](10-std-sync.md) · **Next:** [ std.net](11-std-net.md)
