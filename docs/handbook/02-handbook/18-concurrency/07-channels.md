# Channels

`Channel<T>` is a typed, shareable queue across concurrent branches. It is
available in bounded, rendezvous (`capacity: 0`) and growable forms.

```zirk
mut messages: Channel<String> = Channel();
mut jobs: Channel<Job> = Channel(8);

messages.send("ok");
mut message = messages.receive();
```

Bounded forms apply backpressure by suspending senders without blocking an OS
thread. `try_send` and `try_receive` distinguish success, temporary
fullness/absence, and closure. `close()` wakes waiters; queued values remain
receivable, then receive reports closed. A channel endpoint is shareable, and
the runtime traces managed references in queued values.

---

**Previous:** [← Timeouts](06-timeouts.md) · **Next:** [ parallel](08-parallel.md)
