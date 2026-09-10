# `Job<T>`

`Job<T>` is the handle returned by `spawn` and by `Timer.after` or
`Timer.every`. It represents one runtime branch owned by its lexical scope.

```zirk
concurrent {
    inmut report: Job<String> = spawn build_report();
    inmut text: String = report.wait();
    stdout.println(text);
}
```

`wait()` returns `T` and consumes the handle; a second wait or later use is a
compile-time error. `cancel()` requests cooperative cancellation. `done` is a
Boolean property for observation without consumption. A bound handle must be
waited, cancelled, or explicitly discharged with `_ = handle`.

---

**Previous:** [← `Timer`](15-timer.md) · **Next:** [ Cancellation](05-cancellation.md)
