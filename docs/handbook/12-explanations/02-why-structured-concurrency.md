# Why Structured Concurrency?

Orphan work makes cancellation, errors, resources, shutdown, and tests difficult to reason about. Zirk binds child tasks and threads to scopes: exit means completion or cooperative cancellation followed by joining.

The runtime may schedule through pools and reactors, but applications reason from lexical ownership. A detached form would require explicit transfer to a supervisor and is not part of 1.x.

---

**Previous:** [← Why Static Types?](./01-why-static-types.md) · **Next:** [Why `Result` and Exceptions? →](./03-why-result-and-exceptions.md)
