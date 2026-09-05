# Iteration and Functional Style

Iteration separates producing values from storing them. Zirk combines `Iterable` and iterator contracts with transformations such as `map`, `filter`, and `reduce`, allowing pipelines without making every operation eagerly allocate a collection.

`List<T>` and `Array<T>` also expose eager, chainable `map`, `filter`, `reduce`,
`take`, `skip`, and `reverse` directly on the collection, as well as terminal
conversions such as `to_list`, `to_array`, and `to_set`. Use an `Iterator` when
you need lazy, single-pass evaluation; use collection methods when readability
and eager materialization are preferred.

---

**Previous:** [← Collection Contracts and Complexity](../12-collections/10-collection-contracts-and-complexity.md) · **Next:** [ Iterable](01-iterable.md)
