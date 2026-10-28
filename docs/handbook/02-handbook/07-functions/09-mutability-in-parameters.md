# Mutability in Parameters

A parameter binding is not permission to mutate all reachable state. Rebinding, mutating a value through its API, and modifying caller-visible state follow distinct contracts.

Functions should make mutation visible through parameter/type design rather than relying on a familiar name. `inmut::strict` values cannot be modified transitively. At implemented concurrency boundaries, mutable reference captures are checked; `Thread.run`, scalar atomics, and scoped `Mutex<T>.with` are available, while the general `Thread<T>` lifecycle and other synchronization APIs remain unavailable.

When a function conceptually produces a changed value, returning that value is often clearer than hidden mutation. Use mutation when identity or incremental state is genuinely part of the abstraction.

---

**Previous:** [← Closures and Capture](08-closures-and-capture.md) · **Next:** [ No Traditional Overloading](10-no-traditional-overloading.md)
