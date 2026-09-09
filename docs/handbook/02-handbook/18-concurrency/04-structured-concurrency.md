# Structured Concurrency

Concurrent branches belong to a lexical owner. That owner cannot finish while
child work is abandoned. Failure and cancellation propagate through the
structure, making lifetimes inspectable and cleanup deterministic. The pending
`concurrent { }` surface defines the syntax; general detach does not exist.

The first unhandled child exception cancels siblings, waits for their cleanup,
and propagates with later failures suppressed. A returned `Result.Error` is an
ordinary completed value. Long-lived work transfers explicitly to the
application root supervisor.

---

**Previous:** [← Concurrency and Parallelism](01-concurrency-vs-parallelism.md) · **Next:** [ Cancellation](05-cancellation.md)
