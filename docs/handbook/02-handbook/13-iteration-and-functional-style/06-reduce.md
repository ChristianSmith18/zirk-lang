# `reduce`

`reduce` combines a sequence into one accumulator value.

```zirk
inmut total = values.reduce(
    0,
    (sum: Int32, value: Int32): Int32 => sum + value,
);
```

An explicit initial value defines empty-input behavior and helps infer accumulator type. The operation's order matters for non-associative functions and cannot be parallelized freely unless the contract permits it.

Use a dedicated aggregate such as `sum` when it communicates intent and numeric policy more directly.

---

**Previous:** [← filter](05-filter.md) · **Next:** [ Lazy Operations](07-lazy-operations.md)
