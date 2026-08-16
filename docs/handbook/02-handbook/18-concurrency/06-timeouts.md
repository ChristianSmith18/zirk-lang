# Timeouts

```zirk
await load_data() timeout 5s;
```

A timeout requests cancellation and produces a recoverable typed timeout exception. It does not prove remote work stopped instantly; cleanup and underlying protocol semantics still matter.

---

**Previous:** [← Cancellation](05-cancellation.md) · **Next:** [ Channels](07-channels.md)
