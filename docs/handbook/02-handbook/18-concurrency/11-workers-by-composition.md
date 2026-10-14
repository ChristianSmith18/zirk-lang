# Workers by Composition

> **Status:** This page describes a design direction for worker composition. The
> `thread` API is not currently available, and the examples are illustrative;
> they are not guaranteed to compile with the current compiler. Check the
> [Phase 5 feature status catalog](../../../init/ZIRK_FEATURE_STATUS.md) for
> implemented concurrency capabilities.

Zirk 1.x has no standalone `worker` primitive. A future dedicated worker could
be composed from a supervised concurrent operation or a `thread` plus one or
more `Channel<T>` values. This design keeps scheduling, ownership, shutdown,
and backpressure visible through explicit contracts. `thread` is a proposed
design element here, not an available language capability.

---

**Previous:** [← Threads](10-threads.md) · **Next:** [ Synchronization and Mutexes](12-sync-and-mutex.md)
