# Channels

`Channel<T>` is a typed thread-safe queue across tasks, parallel work, and threads.

```zirk
mut messages = Channel<String>();
mut jobs = Channel<Job>(capacity: 8);

await messages.send("ok");
mut message = await messages.receive();
```

Bounded forms apply backpressure by suspending senders without blocking an OS
thread. `try_send` and `try_receive` distinguish success, temporary
fullness/absence, and closure. `close()` wakes waiters; queued values remain
receivable, then receive reports closed. Channel values follow the derived
copy/transfer/share rules rather than silently creating mutable aliases.

---

**Previous:** [← Timeouts](06-timeouts.md) · **Next:** [ parallel](08-parallel.md)
