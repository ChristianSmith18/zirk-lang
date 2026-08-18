# `std.parallel`

`std.parallel` distributes CPU-bound collection work across a runtime-managed
worker pool. It is distinct from task-aware I/O and from explicit OS threads.

```zirk
inmut hashes = parallel.map(files, hash_file);
parallel.for_each(records, validate);
inmut total = parallel.reduce(values, 0, add);
```

`parallel.map` preserves input order. `for_each` may perform independent effects
in completion order. `reduce` requires an associative reducer because grouping
is implementation-dependent. `parallel.settled` runs every item and returns
ordered settlements; ordinary operations cancel remaining work on the first
unhandled throwable and attach secondary failures as suppressed.

The runtime selects worker count and minimum useful workload automatically,
reuses one pool and flattens nested parallelism to prevent oversubscription.
Cancellation is cooperative and cleanup completes before return. Mutable shared
captures require synchronization; independent projections follow normal deep-
copy/transfer rules. Results and allocation remain bounded by the same
collection limits as sequential operations.

---

**Previous:** [← std.sync](10-std-sync.md) · **Next:** [ std.net](11-std-net.md)
