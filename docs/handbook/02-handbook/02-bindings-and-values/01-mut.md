# `mut`

Use `mut` when a binding must be assigned a new value after declaration. For a
reference, it also permits mutation through that reference.

```zirk
mut attempts: Int32 = 0;
attempts += 1;
```

Mutability is local permission, not a request for shared unsynchronized state.
At the implemented `parallel` boundary, mutable reference captures are rejected.
This does not establish a general mutable-global race check. Managed `thread`
and the `Mutex`, `RwLock`, `Semaphore`, `Barrier`, and `Once` APIs remain
unavailable; scalar `Atomic<T>` is available for supported target widths.

`mut` does not erase the type. This is invalid:

```zirk
mut attempts = 0;
attempts = "one";
```

The compiler inferred an integer type from the initializer and should diagnose the incompatible assignment. Use a union only when multiple alternatives are part of the domain model.

Prefer a new `inmut` value when transformation can be expressed without changing identity. Use `mut` for counters, accumulators, state machines, and other genuinely evolving bindings.

```zirk
mut text = "hello";
text[0] = 'H'; // referent mutation
text = "bye"; // binding reassignment
```

---

**Previous:** [← Bindings and Values](README.md) · **Next:** [ inmut](02-inmut.md)
