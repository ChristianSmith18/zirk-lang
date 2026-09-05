# Sets

`Set<T>` stores unique values according to the type's equality contract.

```zirk
mut permissions: Set<Permission> = Set();
permissions.add(Permission.Network);
```

Adding an equal value does not create a duplicate. Set membership is clearer than scanning a list when uniqueness is part of the domain.

As with maps, iteration order is not implied unless the specific implementation promises it. Mutable elements whose equality changes while stored can violate invariants and should be rejected or avoided.

The generic constraint on `T` is part of the type's safety contract. Union, intersection, difference, subset, and membership operations preserve `T`; mutation requires a mutable referent. Assignment shares the set, while an explicit clone separates subsequent membership changes.

## API

Unique values under `T`'s equality/hash contract; insertion-order iteration.

> Entirely `specified` — `Set` is ahead of the current compiler.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `Int32` | Element count | specified |
| `is_empty` | `Boolean` | `length == 0` | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Set()` / `Set(e0, e1, …)` | `Set<T>` | Construction | specified |
| `s.add(value)` | `Boolean` | `true` when newly inserted | specified |
| `s.remove(value)` | `Boolean` | `true` when present | specified |
| `s.contains(value)` | `Boolean` | Membership | specified |
| `s.union(other)` | `Set<T>` | Set union | specified |
| `s.intersection(other)` | `Set<T>` | Set intersection | specified |
| `s.difference(other)` | `Set<T>` | Set difference | specified |
| `s.symmetric_difference(other)` | `Set<T>` | Symmetric difference | specified |
| `s.is_subset(other)` / `is_superset(other)` / `is_disjoint(other)` | `Boolean` | Relation queries | specified |
| `s.filter(fn)` | `Set<T>` | Eager selection | specified |
| `s.map(fn)` | `List<U>` | Eager transform (returns `List` because `U` may not be hashable) | specified |
| `s.to_list()` | `List<T>` | Materializes as a list | specified |
| `s.to_array()` | `Array<T>` | Materializes as an array | specified |
| `s.iterator()` | `Iterator<T>` | Insertion-order iteration | specified |
| `s.clone()` | `Set<T>` | Independent set | specified |
| `s.to_string()` | `String` | Rendering | specified |

Mutable elements whose equality changes while stored can violate invariants and
should be rejected or avoided.

### Examples

```zirk
mut permissions: Set<Permission> = Set();
permissions.add(Permission.Network);    // true
permissions.add(Permission.Network);    // false — no duplicate
permissions.is_subset(all_required);
```

---

**Previous:** [← Maps](04-maps.md) · **Next:** [ Ranges](06-ranges.md)
