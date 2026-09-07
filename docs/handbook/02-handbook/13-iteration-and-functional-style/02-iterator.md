# `Iterator`

An iterator tracks traversal state and yields successive elements until completion. It hides the storage strategy while exposing one typed sequence.

Consumers must respect whether iteration can fail, block, or be cancelled. Concurrent use requires an explicit thread-safe contract; ordinary iterators are not assumed safe for shared mutation.

Completion is explicit and cannot be confused with a nullable item:

```zirk
enum Iteration<T> { Item(T), Done }

interface Iterator<out T> {
    next(): Iteration<T>;
}
```

An item is an independent projection. Expected iteration failure appears in a
declared `Result`-shaped item or completion contract, never as a hidden third
meaning of `Done`. Structural mutation of a source invalidates existing
iterators; the next operation reports a deterministic controlled error.

Prefer an iterator-returning API when callers need streaming consumption or composition. Return a collection when callers require stable repeated access or indexing.

---

**Previous:** [← Iterable](01-iterable.md) · **Next:** [ Generators](03-generators.md)
