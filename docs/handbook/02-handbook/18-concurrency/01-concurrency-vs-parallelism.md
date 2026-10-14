# Concurrency and Parallelism

Concurrency structures overlapping work; parallelism requests simultaneous CPU execution. A branch suspended in a cooperative channel or timer wait does not occupy an operating-system worker thread. Synchronous I/O can still block the cooperative executor because a general asynchronous I/O reactor is not delivered. `parallel` divides CPU work across a separate runtime pool. Choose by intent, not performance folklore.

---

**Previous:** [← Concurrency](README.md) · **Next:** [ Structured Concurrency](04-structured-concurrency.md)
