# Sets

`Set<T>` stores unique values according to the type's equality contract.

```zirk
mut permissions: Set<Permission> = Set();
permissions.add(Permission.Network);
```

Adding an equal value does not create a duplicate. Set membership is clearer than scanning a list when uniqueness is part of the domain.

As with maps, iteration order is not implied unless the specific implementation promises it. Mutable elements whose equality changes while stored can violate invariants and should be rejected or avoided.

The generic constraint on `T` is part of the type's safety contract. Union, intersection, difference, subset, and membership operations preserve `T`; mutation requires a mutable referent. Assignment shares the set, while an explicit clone separates subsequent membership changes.

---

**Previous:** [← Maps](04-maps.md) · **Next:** [ Ranges](06-ranges.md)
