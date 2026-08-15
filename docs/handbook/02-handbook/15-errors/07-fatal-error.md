# `fatalError`

`fatalError(message)` marks an irreparable process state and does not return normally.

```zirk
if !runtime_invariant {
    fatalError("runtime invariant violated");
}
```

The runtime attempts a diagnostic and only cleanup that remains safe. It does not promise ordinary unwinding or recovery. Never use fatal termination for expected user input or I/O failure.

---

**Previous:** [← `finally`](./06-finally.md) · **Next:** [Assertions →](./08-assertions.md)
