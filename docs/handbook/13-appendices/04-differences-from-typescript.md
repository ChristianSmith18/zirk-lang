# Differences from TypeScript

Zirk emits standalone native binaries, retains types at compilation, has no JavaScript host or `undefined`, distinguishes tasks/parallelism/threads, uses deterministic resources and manifest capabilities, and distributes portable typed IR.

Familiar unions, classes, interfaces, inference, and nullable syntax do not imply TypeScript structural or Promise semantics.

Zirk classes and value classes are nominal; union operations are limited to the common compatible capability set. `Char` is a grapheme type rather than a one-code-unit String convention. Zirk distinguishes `inmut` binding stability from `inmut::strict` reachable immutability, and its temporal family avoids JavaScript `Date`'s mixture of instant, local calendar, and zone concerns.

---

**Previous:** [← Valid and Invalid Examples](03-valid-and-invalid-examples.md) · **Next:** [ Differences from Python](05-differences-from-python.md)
