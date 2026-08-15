# Sets

`Set<T>` stores unique values according to the type's equality contract.

```zirk
mut permissions: Set<Permission> = Set();
permissions.add(Permission.Network);
```

Adding an equal value does not create a duplicate. Set membership is clearer than scanning a list when uniqueness is part of the domain.

As with maps, iteration order is not implied unless the specific implementation promises it. Mutable elements whose equality changes while stored can violate invariants and should be rejected or avoided.

---

**Previous:** [← Maps](./04-maps.md) · **Next:** [Ranges →](./06-ranges.md)
