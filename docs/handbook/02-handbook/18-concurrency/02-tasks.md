# Tasks

`task` creates typed managed work in the current structured scope and returns
`Task<T>`.

```zirk
mut operation: Task<Result<Data, LoadError>> = task load_data();
mut result: Result<Data, LoadError> = await operation;
```

The callable form is shorthand for a task block. A task cannot become implicitly
orphaned. Scope exit waits for completion or requests cancellation and then
waits for cleanup. Ignoring a must-use task result requires explicit `_ =`; it
does not detach the work.

---

**Previous:** [← Concurrency and Parallelism](01-concurrency-vs-parallelism.md) · **Next:** [ await](03-await.md)
