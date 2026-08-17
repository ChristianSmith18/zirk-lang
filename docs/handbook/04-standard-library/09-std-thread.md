# `std.thread`

`std.thread` exposes `Thread<T>` handles, names, results, cooperative cancellation where applicable, and `join` for language-created OS threads.

Threads are managed resources and cannot be abandoned at scope exit. Shared mutation requires synchronization; ordinary task work should not create a thread unnecessarily.

Crossing the thread boundary uses the compiler-derived transfer/share rules.
Use a thread only for native affinity, specialized stack/runtime behavior, or
blocking isolation that cannot use the standard task reactor/blocking pool.

---

**Previous:** [← std.task](08-std-task.md) · **Next:** [ std.sync](10-std-sync.md)
