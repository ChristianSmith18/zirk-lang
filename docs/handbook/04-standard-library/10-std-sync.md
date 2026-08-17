# `std.sync`

The module supplies `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`,
`Atomic<T>`, memory orders, and parallel reductions.

`Mutex<T>.with(...)` scopes writable access; an ordinary guard cannot cross
`await`. Atomics cover supported operations but do not automatically protect
multi-step invariants. Sequential consistency is the default, and explicitly
weaker orderings require `unsafe`.

---

**Previous:** [← std.thread](09-std-thread.md) · **Next:** [ std.net](11-std-net.md)
