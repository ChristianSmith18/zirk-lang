# Atomics

`Atomic<T>` exists only for supported types and operations: load, store, exchange, compare-exchange, and selected numeric updates. The default memory order is safe; weaker orders are explicit advanced tools whose proof obligation belongs to the programmer.

Atomics protect individual operations, not multi-step invariants.

---

**Previous:** [← Synchronization and Mutexes](12-sync-and-mutex.md) · **Next:** [ Data-Race Prevention](14-data-race-prevention.md)
