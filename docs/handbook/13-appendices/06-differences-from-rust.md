# Differences from Rust

Zirk safe code is designed to prevent use-after-free and undefined behavior, but Zirk 1.x does not expose ownership, borrowing, or lifetime syntax. Memory strategy is automatic and may combine several techniques. Concurrency checks apply only at implemented boundaries; for example, `parallel` rejects mutable reference captures, but the language does not claim a general data-race guarantee for all shared state.

Resources use `match with`; structured scopes, CPU `parallel` work, typed channels, managed `Thread.run`, scalar atomics, and scoped `Mutex<T>.with` are available. General `Thread<T>` handles and other synchronization APIs remain specified only; native interoperability uses C ABI wrappers.

---

**Previous:** [← Differences from Python](05-differences-from-python.md) · **Next:** [ Current Limitations](07-current-limitations.md)
