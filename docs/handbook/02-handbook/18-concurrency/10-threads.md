# Threads

`thread` creates a real OS thread with optional name, result, `join`, and cooperative cancellation.

```zirk
mut native_thread = thread "worker" { process(); };
native_thread.join();
```

A managed thread cannot be abandoned at scope exit. Shared mutation requires synchronization.

Use threads for native affinity, unavoidable blocking isolation, or specialized
stack/runtime needs. Ordinary legacy blocking calls use the pending explicit
blocking bridge, which runs on a separate pool.

---

**Previous:** [← parallel for](09-parallel-for.md) · **Next:** [ Workers by Composition](11-workers-by-composition.md)
