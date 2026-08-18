# `std.collections`

The standard collection family combines fixed storage, resizable sequences,
deterministic mappings and lazy iteration:

| Type | Role | Ordering |
| --- | --- | --- |
| `Array<T>` | fixed-length indexed storage | index order |
| `List<T>` | resizable indexed sequence | index order |
| `Map<K,V>` | unique key/value associations | insertion order |
| `Set<T>` | unique values | insertion order |
| `Deque<T>` | efficient operations at both ends | sequence order |
| `PriorityQueue<T>` | priority removal | priority then stable tie order |
| `Queue<T>` | FIFO-restricted deque API | insertion order |
| `Stack<T>` | LIFO-restricted sequence API | insertion order |
| `Range<T>` | finite arithmetic value sequence | range definition |

Specialized multimaps, multisets, trees and linked lists can be implemented as
packages. `Map`/`Set` remain deterministic without separate public hash/ordered
families. Hashing uses a per-process defensive seed while iteration preserves
insertion order.

> **Implementation status:** accepted Zirk 1.x collection contract; current
> compiler support may be partial.

## Reference, mutation and copying

Whole collection assignment shares its reference. Projection reads—index,
slice, iterator item, map entry or materialized view element—produce independent
logical values and require `Clone` when needed. A projection used as a mutation
place updates original storage. `clone()` recursively creates logical
independence.

`inmut` prevents rebinding but does not freeze referenced contents;
`inmut::strict` deeply prevents mutation. Separate `ReadonlyList` families are
therefore unnecessary. Standard collections are not implicitly thread-safe;
shared mutation uses synchronization or channels.

## Access and mutation

Ordered collections accept negative indices and perform checked access. Direct
absence raises a typed runtime error; `get` returns `Result`, and `get_or_null`
deliberately collapses absence with nullability. Slices use
`[start:end:step]`, Python-style omitted defaults, strict explicit bounds and
equal-length replacement.

```zirk
list.add(value);
list.insert(index, value);
inmut removed = list.remove_at(index);
inmut changed = list.remove(value); // Boolean
list.splice(start, delete_count, replacement);

inmut outcome = map.set(key, value);
// SetResult.Inserted or SetResult.Replaced(previous)

inmut inserted = set.add(value); // Boolean
```

`Array` never resizes; `fill`, `copy_from` and slice replacement preserve its
exact length. `List` grows automatically. Only `length` is public: allocation
capacity, reservation and shrinking remain runtime implementation details.
`clear()` is a logical operation and makes no observable capacity promise.

Map/Set keys require coherent `Hash` and `Equal`; equal values must hash equally.
Records, enums and value classes can request explicit compiler derivation.
Sets include union, intersection, difference, symmetric difference and subset,
superset/disjoint queries.

## Eager and lazy transformation

Collection operations are eager and preserve the collection family where the
result permits it:

```zirk
inmut names: List<String> = users.map((user) => user.name);
```

Iterator transformations are lazy and allocate only at an explicit terminal:

```zirk
inmut names: List<String> = users
    .iterator()
    .filter((user) => user.active)
    .map((user) => user.name)
    .collect();
```

Without an expected type, use `collect<List<String>>()`, `to_list`, `to_set` or
`to_map`. Materialization is never implicit because it consumes the iterator,
allocates and may fail.

## Iterator contract

`Iterable<T>` creates `Iterator<T>`; an iterator is single-pass and returns
`Iteration.Item(value)` or permanently `Iteration.Done`. Values are independent
projection copies. Structural source mutation produces deterministic
`IteratorInvalidatedError` rather than stale reads. Read-only views share
storage for a checked lifetime and produce `ViewInvalidatedError` after a
structural change; mutable views are excluded initially.

The standard lazy catalog includes `map`, `filter`, `reduce`, `enumerate`,
`zip`, `zip_exact`, `chain`, `flatten`, `flat_map`, `take`, `skip`, predicate
variants, `chunks`, `chunks_exact`, `windows`, `windows_view`, `inspect`,
`partition`, `group_by`, `find`, `position`, `contains`, `any`, `all`,
`for_each`, `count`, `min`, `max`, `sum`, `first`, `last`, `nth` and `collect`.

`zip` ends with the shorter input; `zip_exact` reports unequal length.
`chunks` retains a final partial chunk; `chunks_exact` exposes its remainder.
`windows` copies independent lists, while `windows_view` explicitly borrows a
checked view. Empty `reduce` without an initial value returns
`Result.Error(EmptyIterationError)`.

Sorting is stable by default. `sort_by` extracts a key; `sort_with` accepts an
advanced comparator; `sort_unstable` explicitly trades stability for an
implementation advantage. `Range` always has a finite end—there is no
`Range.from(0)`—and equality compares its definition.

Pure CPU iteration does not suspend or check cancellation magically. Use an
explicit check or `std.parallel`. Task-aware streams remain separate from
`Iterator` so iteration never hides `await`. Generator cleanup runs when
iteration finishes early, fails or is cancelled.

## Complexity

Array/List indexing is constant-time; List end-add is amortized constant-time;
middle insertion/removal is linear. Map/Set lookup is expected constant-time
with collision defenses. Stable comparison sorting is `O(n log n)`. Lazy
iterator adapters use constant adapter storage unless their documented
operation (such as sorting/grouping) must buffer. Allocation/size limits return
typed failures rather than silently dropping values.

---

**Previous:** [← std.text](05a-std-text.md) · **Next:** [ std.time](07-std-time.md)
