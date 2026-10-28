# Concurrency

Zirk's delivered structured concurrency surface includes `concurrent`,
`spawn`, `Job<T>`, `Timer`, `Concurrent.of(...).first()`,
`Concurrent.of(...).settled()`, and `parallel`. The completion terminals support
`.within(Duration)` deadlines. `Concurrent.each` and `Concurrent.each_settled`
also provide bounded native collection work: they accept at most 256 elements
from an `inmut::strict` collection whose complete element graph is `Share`,
with default concurrency 32 and explicit limits from 1 to 256. They preserve
input order; `each` cancels and joins siblings after failure, while
`each_settled` returns typed outcomes. These collection combinators require the
native scheduler and are excluded by the browser interpreter. `concurrent`/
`spawn`/`Timer` run on one cooperative executor: overlap is useful for waiting
work, not a claim of multi-core execution. `parallel` runs on a separate worker
pool and *is* real multi-core execution, for the `for`-over-`Array`/`List`
shape [parallel for](09-parallel-for.md) covers. Typed channels are available
to cooperative branches; channel operations inside `parallel` regions are
unavailable. `Thread.run` is a delivered managed OS-thread callback API; the
general `Thread<T>` handle lifecycle remains specified only.

Start with [Structured Concurrency](04-structured-concurrency.md), then use
the [`Timer` reference](15-timer.md) for delays, the [`Job<T>` reference](16-job.md)
for dynamic branch handles, [timeouts](06-timeouts.md) and
[`parallel`](08-parallel.md) for CPU-bound work.

The normative contract is
[Structured Concurrency Semantics](../../../STRUCTURED_CONCURRENCY_SEMANTICS.md).

---

**Previous:** [← Transactional Unsafe and commit](../17-memory-and-safety/12-transactional-unsafe-and-commit.md) · **Next:** [ Concurrency and Parallelism](01-concurrency-vs-parallelism.md)
