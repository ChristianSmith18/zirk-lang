# Data-Race Prevention

Safe Zirk rejects unsynchronized concurrent access when at least one access mutates shared state. Mutable globals used from `parallel` or `thread` require `sync` or `Atomic<T>`.

`inmut::strict`, exclusive inferred transfer, independent `clone()`, channels,
and explicit locks provide different safe strategies. Internal `Transfer` and
`Share` capabilities are compiler-derived and cannot be forged by normal user
code. A successful build must not rely on timing to avoid a race, although concurrent
completion order may remain nondeterministic.

---

**Previous:** [← Atomics](13-atomics.md) · **Next:** [ Transfer, Sharing, and Captures](17-transfer-sharing-and-captures.md)
