# `std.thread`

`Thread<T>` represents an operating-system thread for native affinity, special
stack/runtime requirements or blocking work that cannot use the standard pool.
Ordinary concurrent I/O uses tasks and CPU data work uses `std.parallel`.

Threads are managed resources: scope exit joins and never abandons one. Creation
may specify a diagnostic name and stack size, but priority and CPU affinity are
not portable initial APIs. `join()` returns exhaustive `ThreadResult<T>` for a
value, startup failure, cancellation or unhandled throwable.

Cancellation is cooperative. `thread.cancel()` requests cancellation; safe
checks, waits and standard blocking operations observe it. CPU loops call
`Cancellation.check()`. Zirk never kills a thread at an arbitrary instruction,
because that could strand a lock or half-update memory. A non-cooperating native
call may delay shutdown and receives a diagnostic rather than unsafe forced
termination.

Cross-thread captures require compiler-derived transfer/share safety. Public
thread-local storage is excluded: tasks may resume on a different thread, so
application context in TLS would be incorrect. Runtime/native interop may use
controlled internal or unsafe TLS; application context uses explicit values.

---

**Previous:** [← std.time](07-std-time.md) · **Next:** [ std.sync](10-std-sync.md)
