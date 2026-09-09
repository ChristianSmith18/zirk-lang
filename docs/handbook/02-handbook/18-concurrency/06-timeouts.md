# Timeouts

The planned `Concurrent.of(operation).within(5s)` timeout requests cancellation,
waits for local structured cleanup, and then throws `TimeoutError`. It never leaves the timed operation running as accidental
background work. It cannot retract an external effect a remote peer already
observed; underlying protocol semantics still matter.

---

**Previous:** [← Cancellation](05-cancellation.md) · **Next:** [ Channels](07-channels.md)
