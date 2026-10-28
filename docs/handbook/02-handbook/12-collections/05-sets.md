# Sets

`Set<T>` stores unique values according to the type's equality contract. The
runtime stores admitted elements using their complete declared byte layout,
including 128-bit integers and `Float128`; unsupported aggregate element shapes
are still rejected during semantic analysis. Managed references inside
supported elements are traced by the collector. `String` and `Char`
elements are stored as normalized text snapshots so later source-string
mutation cannot invalidate their hash.

```zirk
mut permissions: Set<Permission> = Set();
permissions.add(Permission.Network);
```

Adding an equal value does not create a duplicate. In the current runtime,
class references use object identity; String and Char values use normalized
content snapshots. Set algebra returns values in deterministic order and leaves
its input sets unchanged.

Set `for … in` iterates an insertion-order snapshot. Structural mutation after
iteration starts therefore does not invalidate that snapshot.

The generic constraint on `T` is part of the type's safety contract. Union, intersection, difference, subset, and membership operations preserve `T`; mutation requires a mutable referent. Assignment shares the set, while an explicit clone separates subsequent membership changes.

## API

Unique values under `T`'s equality/hash contract; insertion-order iteration.

> The current compiler implements construction, length/emptiness, add,
> contains, remove, clear, rendering, ordered algebra, relation queries, and
> `for … in` snapshots for all integer widths, binary floats, Decimal, Boolean,
> text, Duration, Date, Time, DateTime, and reference types admitted by semantic
> analysis. Nullable values with a defined runtime layout, including nullable
> text, are supported. Heterogeneous tuples and payload enums derive equality
> from recursively eligible, byte-canonical fields; embedded `String` and
> `Char` fields are not yet eligible because their equality is content-based.
> Nullable Decimal, other structural aggregate elements, and union elements remain
> unsupported.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `UInt64` | Element count | implemented |
| `is_empty` | `Boolean` | Whether the set is empty | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Set()` | `Set<T>` | Empty set construction | implemented |
| `Set(e0, e1, …)` | `Set<T>` | Construction from values | specified |
| `s.add(value)` | `Boolean` | `true` when newly inserted | implemented |
| `s.remove(value)` | `Boolean` | `true` when present | implemented |
| `s.contains(value)` | `Boolean` | Membership | implemented |
| `s.clear()` | `Void` | Removes all values | implemented |
| `s.union(other)` | `Set<T>` | Set union | implemented |
| `s.intersection(other)` | `Set<T>` | Set intersection | implemented |
| `s.difference(other)` | `Set<T>` | Set difference | implemented |
| `s.symmetric_difference(other)` | `Set<T>` | Symmetric difference | implemented |
| `s.is_subset_of(other)` / `is_superset_of(other)` / `is_disjoint(other)` | `Boolean` | Relation queries | implemented |
| `s.filter(fn)` | `Set<T>` | Eager selection | specified |
| `s.map(fn)` | `List<U>` | Eager transform (returns `List` because `U` may not be hashable) | specified |
| `s.to_list()` | `List<T>` | Materializes as a list | specified |
| `s.to_array()` | `Array<T>` | Materializes as an array | specified |
| `for value in s` | `T` per iteration | Insertion-order snapshot | implemented |
| `s.clone()` | `Set<T>` | Independent set | specified |
| `s.to_string()` | `String` | Rendering | implemented |

Mutable class elements use identity, so changing their fields does not change
membership. Composite value shapes outside the eligible tuple and payload-enum
subset are still rejected by semantic analysis.

### Examples

```zirk
mut permissions: Set<Permission> = Set();
permissions.add(Permission.Network); // true
permissions.add(Permission.Network); // false — no duplicate
permissions.is_subset_of(all_required);
```

---

**Previous:** [← Maps](04-maps.md) · **Next:** [ Ranges](06-ranges.md)
