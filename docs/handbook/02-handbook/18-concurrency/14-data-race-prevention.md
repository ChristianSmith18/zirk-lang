# Data-Race Prevention

Safe Zirk rejects unsynchronized concurrent access when at least one access mutates shared state. Mutable globals used from `parallel` or `thread` require `sync` or `Atomic<T>`.

`inmut::strict`, ownership of local task data, channels, and explicit locks provide different safe strategies. A successful build must not rely on timing to avoid a race.

---

**Previous:** [← Atomics](./13-atomics.md) · **Next:** Modules *(next unit)*
