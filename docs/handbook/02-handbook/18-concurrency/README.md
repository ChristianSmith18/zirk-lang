# Concurrency

Zirk's delivered structured concurrency surface is `concurrent`, `spawn`,
`Job<T>`, `Timer`, and `parallel`. `concurrent`/`spawn`/`Timer` run on one
cooperative executor: overlap is useful for waiting work, not a claim of
multi-core execution. `parallel` runs on a separate worker pool and *is*
real multi-core execution, for the `for`-over-`Array`/`List` shape [parallel
for](09-parallel-for.md) covers. Typed channels and operating-system
`thread` work remain separate phases.

Start with [Structured Concurrency](04-structured-concurrency.md), then use
the [`Timer` reference](15-timer.md) for delays, the [`Job<T>` reference](16-job.md)
for dynamic branch handles, and [`parallel`](08-parallel.md) for CPU-bound
work.

The normative contract is
[Structured Concurrency Semantics](../../../STRUCTURED_CONCURRENCY_SEMANTICS.md).

---

**Previous:** [← Transactional Unsafe and commit](../17-memory-and-safety/12-transactional-unsafe-and-commit.md) · **Next:** [ Concurrency and Parallelism](01-concurrency-vs-parallelism.md)
