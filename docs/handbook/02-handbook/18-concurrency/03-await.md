# `await`

`await` suspends the current task until an operation completes; it does not reserve or block an OS thread.

```zirk
mut result = await operation;
```

It is a cancellation-safe point and propagates the operation's typed outcome. Zirk 1.x has no `async fn`; `task` makes concurrent execution explicit.

---

**Previous:** [← Tasks](02-tasks.md) · **Next:** [ Structured Concurrency](04-structured-concurrency.md)
