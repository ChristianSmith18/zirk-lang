# `Iterator`

An iterator tracks traversal state and yields successive elements until completion. It hides the storage strategy while exposing one typed sequence.

Consumers must respect whether iteration can fail, block, or be cancelled. Concurrent use requires an explicit thread-safe contract; ordinary iterators are not assumed safe for shared mutation.

Prefer an iterator-returning API when callers need streaming consumption or composition. Return a collection when callers require stable repeated access or indexing.

---

**Previous:** [← Iterable](01-iterable.md) · **Next:** [ Generators](03-generators.md)
