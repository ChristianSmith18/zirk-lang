# `Iterable`

An iterable can produce an iterator over values of a known element type. `for ... in` consumes this contract rather than depending on a specific collection representation.

```zirk
for user in users {
    stdout.println(user.name);
}
```

The iterable contract documents ordering, repeatability, mutation behavior, errors, blocking, and cancellation. A one-shot stream and an in-memory list may both be iterable without being interchangeable in cost or lifetime.

Arrays, lists, sets, maps, ranges, generators, and `String` implement iterable
contracts. Iterating a string produces its public character units in text order:

```zirk
for character in "Zirk" {
    stdout.println(character);
}
```

A user-defined class becomes iterable by implementing `Iterable<T>` and
returning an `Iterator<T>` from `iterator()`.

The contract is covariant and yields independent projections:

```zirk
interface Iterable<out T> {
    fn iterator(): Iterator<T>;
}
```

Consequently, mutating a loop variable that contains a reference-backed value
does not mutate the collection element. Use an indexed place or another
explicit collection mutation operation to change storage.

---

**Previous:** [← Iteration and Functional Style](README.md) · **Next:** [ Iterator](02-iterator.md)
