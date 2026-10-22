# `fatalError`

`fatalError(message)` marks an irreparable process state and does not return normally.

```zirk
if !runtime_invariant {
    fatalError("runtime invariant violated");
}
```

The runtime attempts a diagnostic and only cleanup that remains safe. It does not promise ordinary unwinding or recovery. Never use fatal termination for expected user input or I/O failure.

Result no longer exposes `unwrap()` or `unwrap_error()`. Use direct `match` to
inspect either variant without converting an expected failure into an
unrecoverable process termination. Uncaught recoverable exceptions are not
`fatalError`: a task reports failure to its structured scope, while `main`
prints the typed exception and trace and exits nonzero.

---

**Previous:** [← finally](06-finally.md) · **Next:** [ Assertions](08-assertions.md)
