# Collections

Collections store multiple typed values. Their contracts differ in size, ordering, uniqueness, lookup, mutation, iteration, indexing, and ownership.

This unit covers arrays, fixed arrays, lists, maps, sets, ranges, indexing, slicing, and safe access.

`Array`, `List`, `Map`, and `Set` are shared native reference types. Assignment creates an alias. `inmut` prevents rebinding while still allowing permitted referent mutation; `inmut::strict` freezes the reachable collection and forbids mutable aliases. Use `clone()` for an independent deep copy. Element and key types must satisfy each operation's generic contracts.

`List<T>` and `Array<T>` expose eager `map`, `filter`, `for_each`, `reduce`,
`sum`, and `count`, plus membership, sorting, reversing, and nullable first/last
access. `Range<T>` supports the sequence pipeline and can materialize as a
`List<T>` with `collect()`. Additional operations and cross-family conversions
remain specified; see the per-type API tables.

> **Implementation status:** `Array<T>` (fixed-capacity), `List<T>` (resizable), and `Range<T>` are delivered as compiler-known sequence surfaces. Arrays and lists provide collector-tracked storage, checked indexing, `get`, and iteration; eager sequence operations are listed in their API tables. `Map<K,V>` and `Set<T>` provide lookup, insertion, membership, and removal for supported hashable keys/elements. These surfaces do not imply an importable `std.collections` package.

---

**Previous:** [← Intersection Types](../10-data-types/09-intersection-types.md) · **Next:** [Arrays →](01-arrays.md)
