# Mutability in Parameters

A parameter binding is not permission to mutate all reachable state. Rebinding, mutating a value through its API, and modifying caller-visible state follow distinct contracts.

Functions should make mutation visible through parameter/type design rather than relying on a familiar name. `inmut::strict` values cannot be modified transitively. At the implemented `parallel` boundary, mutable reference captures are rejected; managed threads and synchronization APIs are not currently available.

When a function conceptually produces a changed value, returning that value is often clearer than hidden mutation. Use mutation when identity or incremental state is genuinely part of the abstraction.

---

**Previous:** [← Closures and Capture](08-closures-and-capture.md) · **Next:** [ No Traditional Overloading](10-no-traditional-overloading.md)
