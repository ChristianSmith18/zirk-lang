# Collection Contracts and Complexity

Choose a collection by its invariant, not only its literal syntax.

| Family | Shape | Order | Structural mutation | Equality | Typical access |
|---|---|---|---|---|---|
| `Array<T>` | contiguous dynamic reference | insertion order | supported | ordered elements | index O(1) |
| `T[N]` | fixed-size contiguous reference | index order | element only | ordered elements | index O(1) |
| `List<T>` | growable reference sequence | insertion order | supported | ordered elements | index O(1) |
| `Map<K,V>` | key/value reference container | stable documented iteration | supported | mappings | average lookup O(1) |
| `Set<T>` | unique reference container | stable documented iteration | supported | membership | average lookup O(1) |
| `Range<T>` | lazy value progression | progression order | none | definition | iteration O(n) |

Complexities describe ordinary operation, excluding the cost of deep-cloning a
projected result. Allocation overflow, capacity overflow, invalid bounds,
missing direct access, and iterator invalidation are controlled errors rather
than undefined behavior.

## Core APIs

`Array` and `List` provide `length`, `capacity` where meaningful, `get`,
`get_or_null`, `append`, `insert`, `remove_at`, `contains`, `clear`, `clone`,
iteration, and sequence conversion. Fixed arrays expose `length`, checked
indexing, element replacement, `clone`, iteration, and conversion, but reject
append, removal, and resizing.

`Map` provides checked `[]`, `get`, `get_or_null`, `get_or_insert_with`,
`contains_key`, `insert`, `remove`, `keys`, `values`, `entries`, `length`,
`clear`, and `clone`. Keys satisfy equality and hashing contracts. A default or
factory is evaluated only when the key is absent.

`Set` provides `contains`, `insert`, `remove`, `union`, `intersection`,
`difference`, `symmetric_difference`, subset/superset tests, `length`, `clear`,
iteration, and `clone`. Mathematical operations require compatible element
types and their equality/hash contracts.

## Access and slicing

`String`, tuple, array, and list indexes accept negatives. Direct missing
access reports a typed error. `get` returns `Result`; `get_or_null` is the
explicit nullable alternative.

Only `String`, arrays, and lists slice. End is exclusive. `[::]` means first to
last by one, `[n:w]` uses step one, and `[::-1]` reverses. With a negative step,
omitted bounds mean last element through just before the first. Step zero and
explicit out-of-range bounds fail; explicit bounds are not Python-clamped.

Every slice is a deep independent collection. Slice assignment requires an
equal number of replacement elements. Use `splice` or another named structural
operation when a list must change length.

## Iteration and invalidation

Iteration yields independent projection copies. Structural mutation after an
iterator is created invalidates it; its next operation deterministically
reports an invalidation error. Explicit read-only views are the only shared
subcollection form. Mutable iteration is not in the initial language.

---

**Previous:** [← Safe Collection Access](09-safe-access.md) · **Next:** [Iteration and Functional Style →](../13-iteration-and-functional-style/README.md)
