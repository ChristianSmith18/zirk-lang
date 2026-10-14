# Transfer, Sharing, and Captures

> **Status:** This page describes the intended design for transfer, sharing,
> and concurrent captures. Some mechanisms below, including synchronized
> references and locks, are not currently available as language capabilities;
> examples are illustrative and may not compile today. See the
> [Phase 5 feature status catalog](../../../init/ZIRK_FEATURE_STATUS.md) for
> implementation status.

The design has the compiler derive internal `Transfer` and `Share` properties;
users do not implement or place them in ordinary `Fn` types.

At a concurrent boundary:

- values and projections copy;
- complete `inmut::strict` references may share when the implemented rules
  permit it;
- exclusive mutable references may transfer, disabling sender use until they
  return through a structured result or channel;
- `clone()` creates an independent graph;
- synchronization-aware references may share under the proposed synchronization
  contracts; synchronized references are not currently available;
- resources, pointers, locks, and dependent views are intended to follow
  specialized contracts. Locks and the corresponding `sync` APIs are not yet
  available.

Concurrent branch captures use those same rules in the design. Capturing
`users[0]` captures an independent projected value, while capturing complete
`users` would share or transfer the complete reference only when safe. An
ambiguous mutable alias is intended to be a compile-time error with suggestions
for strict sharing, transfer, synchronization, or clone; those diagnostics and
suggestions may not all be implemented today.

---

**Previous:** [← Data-Race Prevention](14-data-race-prevention.md) · **Next:** [ Modules](../19-modules/README.md)
