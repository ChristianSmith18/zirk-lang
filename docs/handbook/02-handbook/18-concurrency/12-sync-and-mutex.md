# Synchronization and Mutexes

`sync` and mutexes protect shared mutable state. Keep critical sections small and never hold an ordinary mutex across `await` unless its type explicitly supports that pattern; the compiler or linter diagnoses it.

Prefer channels or immutable values when ownership transfer expresses the design better.

---

**Previous:** [← Workers by Composition](11-workers-by-composition.md) · **Next:** [ Atomics](13-atomics.md)
