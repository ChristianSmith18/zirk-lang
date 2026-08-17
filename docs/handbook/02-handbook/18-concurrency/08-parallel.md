# `parallel`

`parallel` requests potentially simultaneous finite CPU work.

```zirk
parallel { process_a(); process_b(); }
```

The runtime chooses workers and scheduling. Ordered map-like operations preserve
input order; completion-order variants are explicitly unordered. Unmanaged
blocking I/O and unsafe mutable capture are rejected. Error handling cancels
remaining work cooperatively and awaits cleanup.

---

**Previous:** [← Channels](07-channels.md) · **Next:** [ parallel for](09-parallel-for.md)
