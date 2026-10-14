# `std.sync`

> **Implementation status — partially delivered.** The compiler exposes the
> scalar `Atomic<T>` source API described in the [Atomics chapter](../02-handbook/18-concurrency/13-atomics.md).
> `Mutex`, `RwLock`, `Semaphore`, `Barrier`, `Once`, and `ConditionVariable`
> remain unavailable as Zirk source APIs. See the
> [feature status catalog](../../init/ZIRK_FEATURE_STATUS.md).

`std.sync` provides scoped synchronization for shared state: `Mutex<T>`,
`RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`, `ConditionVariable` and supported
atomics. It does not make standard collections implicitly thread-safe.

## Scoped locks

Safe mutex access uses a callback so a guard cannot escape or cross `await`:

```zirk
mut users = Mutex(List<User>());

users.with((items) => {
    items.add(user);
});

inmut count = users.with((items) => items.length);
```

Returned values must be independent of protected storage. Exceptions release
the lock and preserve completed mutations; there is no automatic rollback or
Rust-style poisoning. Waiting for a mutex is cancelable, but the callback itself
cannot suspend. Extract state, perform the suspending operation, then reacquire
to apply the result.

`RwLock` permits concurrent readers and favors queued writers to avoid
starvation. Semaphore permits are managed resources released on every scope
exit. Barrier is reusable through internal generations. `Once<T>` caches its
first success or failure so all callers observe one initialization outcome.
`ConditionVariable` exists for thread/native algorithms; task-level coordination
should prefer channels.

## Atomics and diagnostics

Atomics support `Boolean` and signed/unsigned integer widths through 64 bits
where the target provides atomic operations. Their scalar payload is inline in
the GC-managed object. Sequential consistency is the default; weaker memory
ordering requires `unsafe`, and a dynamically invalid ordering raises
`InvalidArgumentError`. Pointer atomics are unsupported. Atomics protect
individual operations, not multi-step invariants. The remaining lock diagnostics
apply only when those synchronization primitives are implemented.

---

**Previous:** [← std.thread](09-std-thread.md) · **Next:** [Parallel CPU operations →](10a-std-parallel.md)
