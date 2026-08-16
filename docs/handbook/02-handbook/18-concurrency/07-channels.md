# Channels

`Channel<T>` is a typed thread-safe queue across tasks, parallel work, and threads.

```zirk
mut messages: Channel<String> = Channel();
messages.send("ok");
mut message = await messages.receive();
```

Channels distinguish closure from temporary absence and bounded forms apply backpressure.

---

**Previous:** [← Timeouts](06-timeouts.md) · **Next:** [ parallel](08-parallel.md)
