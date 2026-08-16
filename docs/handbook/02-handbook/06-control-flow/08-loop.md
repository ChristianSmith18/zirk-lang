# `loop`

`loop` repeats unconditionally until control leaves through `break`, `return`, an exception, cancellation, or a never-returning operation.

```zirk
loop {
    inmut event = next_event();
    if event.is_shutdown {
        break;
    }
    handle(event);
}
```

Use it when unconditional repetition is the real model, rather than writing `while true`. Flow analysis can treat a loop with no reachable exit as `Never`.

Long-running loops must still cooperate with cancellation and resource cleanup where their execution context requires it.

---

**Previous:** [← while](07-while.md) · **Next:** [ break and continue](09-break-and-continue.md)
