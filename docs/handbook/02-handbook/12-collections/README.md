# Collections

Collections store multiple typed values. Their contracts differ in size, ordering, uniqueness, lookup, mutation, iteration, indexing, and ownership.

This unit covers arrays, fixed arrays, lists, maps, sets, ranges, indexing, slicing, and safe access.

`Array`, `List`, `Map`, and `Set` are shared native reference types. Assignment creates an alias. `inmut` prevents rebinding while still allowing permitted referent mutation; `inmut::strict` freezes the reachable collection and forbids mutable aliases. Use `clone()` for an independent deep copy. Element and key types must satisfy each operation's generic contracts.

`List<T>` and `Array<T>` also expose eager, chainable transformations such as
`map`, `filter`, `flat_map`, `reduce`, `take`, `skip`, and `reverse` directly on
the collection, as well as terminal conversions like `to_list`, `to_array`, and
`to_set`. The last method in the chain determines the result family. For lazy,
single-pass pipelines, use `.iterator()` and `collect()`.

> **Implementation status:** `Array<T>` (fixed-capacity) and `List<T>` (resizable) are delivered by `array-list-tuple-duration-regex` as collector-tracked native reference collections with bounds-checked indexing, `length`/`is_empty`, and `for ... in` through `Iterable<T>`. `Map<K,V>` and `Set<T>` are partially delivered by `map-set-collections`: empty constructors, `length`/`is_empty`, `Map.set`/`contains_key`/`get_or_null`/`remove`, and `Set.add`/`contains`/`remove` work end-to-end on hashable primitive keys/elements. The runtime implementation is a minimal i64 map/set and does not trace GC references yet; `Range<T>` remains pending.

---

**Previous:** [← Class or Record?](../10-data-types/08-class-or-record.md) · **Next:** [ Arrays](01-arrays.md)
