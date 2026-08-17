# Structured Concurrency

Child tasks belong to a lexical scope. The scope cannot finish while child work is abandoned. Failure and cancellation propagate through the structure, making lifetimes inspectable and cleanup deterministic.

```zirk
mut dashboard = task scope {
    mut users = task load_users();
    mut roles = task load_roles();
    return combine(await users, await roles);
};
```

The first unhandled child exception cancels siblings, awaits their cleanup, and
propagates with later failures suppressed. A returned `Result.Error` is an
ordinary completed value. General detach does not exist; long-lived work
transfers explicitly to the application root supervisor.

---

**Previous:** [← await](03-await.md) · **Next:** [ Cancellation](05-cancellation.md)
