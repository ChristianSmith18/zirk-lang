# Lazy Operations

A lazy transformation records work and performs it as values are requested. This can avoid intermediate collections and support large streams.

Laziness moves the time at which effects and errors occur. APIs must document whether evaluation is repeatable, whether it captures mutable state, and how cancellation or resources are handled.

Do not return a lazy sequence that depends on a resource already closed by `match with`. Materialize safe results inside the resource scope or transfer the resource through a contract that permits it.

---

**Previous:** [← reduce](06-reduce.md) · **Next:** [ Pipelines](08-pipelines.md)
