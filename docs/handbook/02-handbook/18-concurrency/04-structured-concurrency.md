# Structured Concurrency

Child tasks belong to a lexical scope. The scope cannot finish while child work is abandoned. Failure and cancellation propagate through the structure, making lifetimes inspectable and cleanup deterministic.

Detached work is not part of the initial contract; a future form would require explicit transfer to a root supervisor.

---

**Previous:** [← `await`](./03-await.md) · **Next:** [Cancellation →](./05-cancellation.md)
