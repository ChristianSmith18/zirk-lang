# `std.thread`

`std.thread` exposes `Thread<T>` handles, names, results, cooperative cancellation where applicable, and `join` for language-created OS threads.

Threads are managed resources and cannot be abandoned at scope exit. Shared mutation requires synchronization; ordinary task work should not create a thread unnecessarily.

---

**Previous:** [← std.task](08-std-task.md) · **Next:** [ std.sync](10-std-sync.md)
