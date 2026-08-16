# Tasks

`task` creates typed managed work in the current structured scope.

```zirk
mut operation = task { load_data(); };
```

A task propagates its result or error and cannot become implicitly orphaned. Scope exit waits for completion or requests cancellation and then waits for cleanup.

---

**Previous:** [← Concurrency and Parallelism](01-concurrency-vs-parallelism.md) · **Next:** [ await](03-await.md)
