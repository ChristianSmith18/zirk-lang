# `mut`

Use `mut` when a binding must be assigned a new value after declaration.

```zirk
mut attempts: Int32 = 0;
attempts += 1;
```

Mutability is local permission, not a request for shared unsynchronized state. A mutable application global accessed from `parallel` or `thread` must use `sync` or `Atomic<T>`; otherwise compilation fails.

`mut` does not erase the type. This is invalid:

```zirk
mut attempts = 0;
attempts = "one";
```

The compiler inferred an integer type from the initializer and should diagnose the incompatible assignment. Use a union only when multiple alternatives are part of the domain model.

Prefer a new `inmut` value when transformation can be expressed without changing identity. Use `mut` for counters, accumulators, state machines, and other genuinely evolving bindings.

---

**Previous:** [← Bindings and Values](./README.md) · **Next:** [`inmut` →](./02-inmut.md)
