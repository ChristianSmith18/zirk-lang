# Cancellation

Cancellation is cooperative, idempotent, and observed at `await`, I/O, channel operations, timers, and explicit checks in long loops. `task.cancel()` requests cancellation; it never destroys execution at an arbitrary instruction.

Children receive parent cancellation, close resources, and finish with a typed recoverable cancellation exception unless an API converts it to `Result`.

---

**Previous:** [← Structured Concurrency](./04-structured-concurrency.md) · **Next:** [Timeouts →](./06-timeouts.md)
