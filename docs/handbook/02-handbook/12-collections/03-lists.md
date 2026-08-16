# Lists

`List<T>` represents a dynamically sized ordered collection.

```zirk
mut jobs: List<Job> = List();
jobs.add(next_job);
```

Growth, insertion, and removal follow the list API and may invalidate indexes or iterators according to its documented contract. Concurrent mutation requires a synchronization-safe abstraction.

Use a list when size changes during ordinary operation. Use an iterator when consumers should not depend on storage or random access.

Lists are shared references. `length`, indexing, iteration, insertion, removal, and `clone()` retain the element type `T`; equality is element-wise only when `T` is equatable. Structural edits require a mutable referent and are forbidden through `inmut::strict` aliases.

---

**Previous:** [← Fixed Arrays](02-fixed-arrays.md) · **Next:** [ Maps](04-maps.md)
