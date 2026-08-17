# Task Aggregation

`Task.all(tasks)` preserves input order and requires all tasks to succeed. The
first unhandled exception cancels and cleans the rest. `Task.first(tasks)`
returns the first completion and cancels/cleans every loser.

`Task.settled(tasks)` allows all work to finish and preserves each outcome:

```zirk
enum TaskSettlement<T> {
    Fulfilled(T),
    Rejected(Throwable),
    Cancelled(CancelledError),
}
```

```zirk
mut outcomes = await Task.settled(tasks);

for outcome in outcomes {
    match outcome {
        TaskSettlement.Fulfilled(value) => use(value),
        TaskSettlement.Rejected(error) => report(error),
        TaskSettlement.Cancelled(error) => log(error),
    }
}
```

A returned `Result.Error` is `Fulfilled(Error(...))`; only an unhandled
throwable is `Rejected`.

---

**Previous:** [← Data-Race Prevention](14-data-race-prevention.md) · **Next:** [ select](16-select.md)
