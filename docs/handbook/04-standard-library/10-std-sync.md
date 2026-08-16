# `std.sync`

The module supplies `Mutex<T>`, read/write locks, justified semaphores and barriers, `Atomic<T>`, memory orders, and parallel reductions.

Ordinary mutex guards must not cross `await` unless the type explicitly supports it. Atomics cover supported operations but do not automatically protect multi-step invariants. Defaults choose safe ordering.

---

**Previous:** [← std.thread](09-std-thread.md) · **Next:** [ std.net](11-std-net.md)
