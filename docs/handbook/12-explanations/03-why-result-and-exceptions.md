# Why `Result` and Exceptions?

Expected operational failure belongs in `Result<T,E>` because callers can see and compose it. Exceptions handle unusual recoverable transfer across layers. `fatalError` represents a state in which normal execution cannot safely continue.

Explicit exceptions are checked through `throws`; implicit `RuntimeError`
safety failures remain typed and catchable without making every signature list
bounds, overflow or division. A `Result` in value position stays available for
matching or projects to a nullable success value. In statement position,
success is discarded and failure raises an exception, so routine effect calls
do not need a separate discard marker. Use `try/catch` to recover from that
failure or `match` when you need the original typed error.

One mechanism for all three would either clutter every signature, hide ordinary failure, or encourage termination. The distinction communicates recovery ownership.

---

**Previous:** [← Why Structured Concurrency?](02-why-structured-concurrency.md) · **Next:** [ Why match with?](04-why-match-with.md)
