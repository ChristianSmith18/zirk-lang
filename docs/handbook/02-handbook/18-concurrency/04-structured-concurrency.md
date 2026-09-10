# Structured Concurrency

Concurrent branches belong to a lexical owner. That owner cannot finish while
child work is abandoned. Failure and cancellation propagate through the
structure, making lifetimes inspectable and cleanup deterministic. Use
`concurrent { }` to open that owner and `spawn` for a dynamic child; general
detach does not exist.

```zirk
concurrent {
    inmut profile = load_profile();
    inmut permissions = load_permissions();
}
stdout.println(profile);
```

Direct bindings are branches. Independent bindings start together; a binding
that reads another binding starts only after its dependency completes. The names
hoist after the closing brace. `spawn expr` returns `Job<T>` when a handle is
needed; `job.wait()` consumes that handle, while `job.cancel()` requests a
cooperative stop.

The first unhandled child exception cancels siblings, waits for their cleanup,
and propagates with later failures suppressed. A returned `Result.Error` is an
ordinary completed value. Long-lived work transfers explicitly to the
application root supervisor.

---

**Previous:** [← Concurrency and Parallelism](01-concurrency-vs-parallelism.md) · **Next:** [ `Timer`](15-timer.md)
