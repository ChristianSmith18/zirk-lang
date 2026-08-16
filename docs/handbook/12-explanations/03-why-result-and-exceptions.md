# Why `Result` and Exceptions?

Expected operational failure belongs in `Result<T,E>` because callers can see and compose it. Exceptions handle unusual recoverable transfer across layers. `fatalError` represents a state in which normal execution cannot safely continue.

Explicit exceptions are checked through `throws`; implicit `RuntimeError`
safety failures remain typed and catchable without making every signature list
bounds, overflow or division. `Result` cannot be ignored and never converts
implicitly into an exception. This retains Go-like visible operational failure
while preserving descriptive cross-layer exception contracts.

One mechanism for all three would either clutter every signature, hide ordinary failure, or encourage termination. The distinction communicates recovery ownership.

---

**Previous:** [← Why Structured Concurrency?](02-why-structured-concurrency.md) · **Next:** [ Why match with?](04-why-match-with.md)
