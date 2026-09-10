# Cancellation

Cancellation is cooperative, idempotent, and observed at I/O, channel operations, timers, and explicit checks in long loops. It never destroys execution at an arbitrary instruction.

Children receive parent cancellation, close resources, and finish with a typed recoverable cancellation exception unless an API converts it to `Result`.

The pending `Concurrent.protect` API defines bounded protected cleanup. It does
not swallow other failures and should not contain an indefinitely blocking
operation.

---

**Previous:** [← `Job<T>`](16-job.md) · **Next:** [ Timeouts](06-timeouts.md)
