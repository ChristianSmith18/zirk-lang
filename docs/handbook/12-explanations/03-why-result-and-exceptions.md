# Why `Result` and Exceptions?

Expected operational failure belongs in `Result<T,E>` because callers can see and compose it. Exceptions handle unusual recoverable transfer across layers. `fatalError` represents a state in which normal execution cannot safely continue.

One mechanism for all three would either clutter every signature, hide ordinary failure, or encourage termination. The distinction communicates recovery ownership.

---

**Previous:** [← Why Structured Concurrency?](./02-why-structured-concurrency.md) · **Next:** [Why `match with`? →](./04-why-match-with.md)
