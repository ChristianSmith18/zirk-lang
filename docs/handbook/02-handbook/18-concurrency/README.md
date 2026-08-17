# Concurrency

Zirk names different work explicitly: `task` for structured concurrent work,
`parallel` for finite CPU work across cores, and `thread` for an operating-system
thread. Typed channels and synchronization coordinate them without exposing a
global event loop. Safe code prevents data races while allowing explicitly
coordinated nondeterministic completion.

The normative contract is
[Structured Concurrency Semantics](../../../STRUCTURED_CONCURRENCY_SEMANTICS.md).

---

**Previous:** [← Transactional Unsafe and commit](../17-memory-and-safety/12-transactional-unsafe-and-commit.md) · **Next:** [ Concurrency and Parallelism](01-concurrency-vs-parallelism.md)
