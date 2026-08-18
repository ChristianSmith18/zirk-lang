# `std.sync`

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
cannot suspend. Extract state, await, then reacquire to apply the result.

`RwLock` permits concurrent readers and favors queued writers to avoid
starvation. Semaphore permits are managed resources released on every scope
exit. Barrier is reusable through internal generations. `Once<T>` caches its
first success or failure so all callers observe one initialization outcome.
`ConditionVariable` exists for thread/native algorithms; task-level coordination
should prefer channels.

## Atomics and diagnostics

Atomics cover supported Boolean/integer/native-safe families. Sequential
consistency is the safe default; weaker memory ordering requires `unsafe` and
does not make multi-step invariants atomic. Debug profiles detect known lock
cycles, inconsistent acquisition order and excessive waits where possible,
without claiming perfect deadlock detection.

---

**Previous:** [← std.thread](09-std-thread.md) · **Next:** [ std.parallel](10a-std-parallel.md)
