# Duration

Duration literals attach a time unit directly to a numeric value:

```zirk
inmut retry_delay = 500ms;
inmut timeout = 5s;
inmut cache_age = 5m;
inmut retention = 24h;
```

They are typed durations, not plain integers. APIs can therefore reject accidental mixing of milliseconds, counts, and timestamps.

The runtime may normalize units internally, while formatting and comparison preserve duration semantics. Overflow and conversion follow the underlying duration contract.

Use duration values for timeouts, delays, and intervals. Use timestamp types for points on a clock; subtracting timestamps may produce a duration, but the concepts are not interchangeable.

---

**Previous:** [← Void, Never, Null, and Object](./10-void-never-null-object.md) · **Next:** [Nullability →](../04-nullability/README.md)
