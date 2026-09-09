# Transfer, Sharing, and Captures

The compiler derives internal `Transfer` and `Share` properties; users do not
implement or place them in ordinary `Fn` types.

At a concurrent boundary:

- values and projections copy;
- complete `inmut::strict` references may share;
- exclusive mutable references may transfer, disabling sender use until they
  return through a structured result or channel;
- `clone()` creates an independent graph;
- synchronization-aware references may share;
- resources, pointers, locks, and dependent views follow specialized contracts.

Concurrent branch captures use those same rules. Capturing `users[0]` captures an independent
projected value, while capturing complete `users` would share or transfer the
complete reference only when safe. An ambiguous mutable alias is a compile-time
error with suggestions for strict sharing, transfer, synchronization, or clone.

---

**Previous:** [← Data-Race Prevention](14-data-race-prevention.md) · **Next:** [ Modules](../19-modules/README.md)
