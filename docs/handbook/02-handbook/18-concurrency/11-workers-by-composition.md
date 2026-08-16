# Workers by Composition

Zirk 1.x has no standalone `worker` primitive. Build a dedicated worker from a supervised task or thread plus one or more `Channel<T>` values. This keeps scheduling, ownership, shutdown, and backpressure visible through existing contracts.

---

**Previous:** [← Threads](10-threads.md) · **Next:** [ Synchronization and Mutexes](12-sync-and-mutex.md)
