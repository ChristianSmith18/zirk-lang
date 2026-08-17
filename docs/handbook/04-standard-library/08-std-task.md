# `std.task`

`std.task` provides `Task<T>` handles, scopes, `TaskSettlement<T>`, cancellation
reasons, sleep, timeouts, typed channels, blocking adapters, and support for
language `task`/`await`/`select`. It does not create a second async model or
expose a user-controlled event loop.

Waiting is task-suspending; scope exit joins or cancels children.
`Task.all/first/settled` expose distinct failure policies. `task.blocking`
isolates legacy blocking calls. Channel operations document capacity,
backpressure, closure, cancellation, and typed temporary-unavailable results.

---

**Previous:** [← std.time](07-std-time.md) · **Next:** [ std.thread](09-std-thread.md)
