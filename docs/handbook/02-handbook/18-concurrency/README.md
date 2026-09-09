# Concurrency

Zirk's pending concurrency surface distinguishes structured `concurrent` work,
`parallel` CPU work, and operating-system `thread` work. Typed channels and
synchronization coordinate them without exposing a global event loop. Safe code
prevents data races while allowing explicitly coordinated nondeterministic
completion.

The normative contract is
[Structured Concurrency Semantics](../../../STRUCTURED_CONCURRENCY_SEMANTICS.md).

---

**Previous:** [← Transactional Unsafe and commit](../17-memory-and-safety/12-transactional-unsafe-and-commit.md) · **Next:** [ Concurrency and Parallelism](01-concurrency-vs-parallelism.md)
