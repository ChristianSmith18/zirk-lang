# `await`

`await` suspends the current task until an operation completes; it does not
reserve or block an OS thread.

```zirk
mut result = await operation;
```

It returns exactly the task's `T`: `await Task<Int32>` yields `Int32`; it adds
no implicit wrapper. The same local `Task<T>` cannot be awaited twice, and a
use after its await is rejected. Zirk 1.x has no `async fn`; `task` makes
concurrent execution explicit.

`await ... timeout` is not part of this first slice and gives a targeted
deferred-feature diagnostic.

---

**Previous:** [← Tasks](02-tasks.md) · **Next:** [ Structured Concurrency](04-structured-concurrency.md)
