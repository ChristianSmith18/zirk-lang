# Synchronization and Mutexes

`sync` and `Mutex<T>` protect shared mutable state. Structured
`mutex.with(value => { ... })` access prevents a guard or writable view from
escaping. Keep critical sections small; holding an ordinary mutex across
`await` is a compile-time error.

Prefer channels or immutable values when ownership transfer expresses the design better.

`RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` are standard-library types,
not additional language syntax.

---

**Previous:** [← Workers by Composition](11-workers-by-composition.md) · **Next:** [ Atomics](13-atomics.md)
