# Timeouts

```zirk
await load_data() timeout 5s;
```

A timeout requests cancellation, awaits local structured cleanup, and then
throws `TimeoutError`. It never leaves the timed task running as accidental
background work. It cannot retract an external effect a remote peer already
observed; underlying protocol semantics still matter.

---

**Previous:** [← Cancellation](05-cancellation.md) · **Next:** [ Channels](07-channels.md)
