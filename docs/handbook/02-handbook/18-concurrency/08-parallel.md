# `parallel`

`parallel` requests potentially simultaneous CPU work.

```zirk
parallel { process_a(); process_b(); }
```

The runtime chooses workers and scheduling. Unsafe mutable capture is rejected; error handling cancels remaining work cooperatively and waits for closure.

---

**Previous:** [← Channels](./07-channels.md) · **Next:** [`parallel for` →](./09-parallel-for.md)
