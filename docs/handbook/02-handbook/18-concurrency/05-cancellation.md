# Cancellation

Cancellation is cooperative, idempotent, and observed at `await`, I/O, channel operations, timers, and explicit checks in long loops. `task.cancel()` requests cancellation; it never destroys execution at an arbitrary instruction.

Children receive parent cancellation, close resources, and finish with a typed recoverable cancellation exception unless an API converts it to `Result`.

`cancellation shield { ... }` defers a pending cancellation for bounded cleanup
and delivers it immediately afterward. A shield does not swallow other failures
and should not contain an indefinitely blocking operation.

---

**Previous:** [← Structured Concurrency](04-structured-concurrency.md) · **Next:** [ Timeouts](06-timeouts.md)
