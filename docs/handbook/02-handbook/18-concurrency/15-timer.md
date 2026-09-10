# `Timer`

`Timer.sleep(duration)` suspends the current branch for at least `duration`.
It is a cancellation safe point and does not change the function signature.

`Timer.after(duration, callback)` runs a callback once after the deadline.
`Timer.every(duration, callback)` runs it repeatedly with a fixed delay between
completed callbacks. Both return `Job<T>` and are ambient: the nearest
`concurrent` scope cancels them when it closes, so a periodic callback cannot
keep a finite scope alive.

```zirk
concurrent {
    Timer.after(1s, (): Void => stdout.println("saved"));
    Timer.every(5s, (): Void => stdout.println("heartbeat"));
    Timer.sleep(2s);
}
```

Durations must not be negative. Use `Job.cancel()` for explicit early stop;
use `wait()` only when the program deliberately needs the timer result.

---

**Previous:** [← Structured Concurrency](04-structured-concurrency.md) · **Next:** [ `Job<T>`](16-job.md)
