# `std.task`

`std.task` provides `Task<T>` handles, scopes, cancellation reasons, sleep, timeouts, typed channels, and support for language `task`/`await`. It does not create a second async model or expose a user-controlled event loop.

Waiting is task-suspending; scope exit joins or cancels children. Channel operations document capacity, backpressure, closure, and cancellation.

---

**Previous:** [← `std.time`](./07-std-time.md) · **Next:** [`std.thread` →](./09-std-thread.md)
