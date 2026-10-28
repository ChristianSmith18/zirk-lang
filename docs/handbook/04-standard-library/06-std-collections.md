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

> **Implementation status:** accepted Zirk 1.x collection contract.
> `Array<T>`, `List<T>` and `Range<T>` (`start..end`, `start..end:step`,
> inferred descending direction, slicing, expansion, and `Iterable<T>` for
> integer `T` and `Duration`)
> are delivered by `array-list-tuple-duration-regex`. `Map<K,V>` and `Set<T>`
> currently use layout-aware key, element and Map value storage for admitted
> scalar and reference types, including 128-bit integers and `Float128`; insertion-ordered
> operations, Set algebra, and typed `Map.keys()`, `Map.values()`,
> `Map.entries()` and Set iteration snapshots. Nullable layouts with a defined
> runtime representation (including text), heterogeneous tuples of
> byte-canonical fields, and payload enums with recursively eligible fields
> are also covered. Tuples with content-based text fields, nullable Decimal,
> record derivation, and union key/element support remain incomplete.
> `Deque<T>`, `PriorityQueue<T>`,
> `Queue<T>` and `Stack<T>` are part of the target contract but are not
> implemented yet.

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
absence raises a typed runtime error; `get` returns `Result`, and nullable
value contexts project that same result to the value or null. Slices use
`[start:end:step]`, Python-style omitted defaults, strict explicit bounds and
equal-length replacement.

```zirk
import { stdout } from std.io;
fn main(): Void {
    mut list: List<Int32> = List();
    list.add(1);
    list.insert(0, 2);
    inmut changed: Boolean = list.remove(2);

    mut map: Map<Int32, Int32> = Map();
    map.set(1, 7); // Void; replacement retains the key's position
    inmut found: Boolean = map.contains_key(1);
    inmut outcome: Result<Int32, LookupError> = map.get(1);
    inmut keys: Array<Int32> = map.keys(); // insertion-order snapshot
    inmut values: Array<Int32> = map.values(); // insertion-order snapshot
    inmut entries: Array<Tuple<Int32, Int32 >> = map.entries();

    mut set: Set<Int32> = Set();
    inmut inserted: Boolean = set.add(8);
    inmut present: Boolean = set.contains(8);
    inmut removed_from_set: Boolean = set.remove(8);

    mut deque: Deque<Int32> = Deque();
    deque.push_front(1);
    deque.push_back(2);
    inmut front: Result<Int32, EmptyCollectionError> = deque.front();
    inmut last: Result<Int32, EmptyCollectionError> = deque.pop_back();

    mut queue: Queue<Int32> = Queue();
    queue.enqueue(3);
    inmut next: Result<Int32, EmptyCollectionError> = queue.dequeue();
    inmut head: Result<Int32, EmptyCollectionError> = queue.peek();

    mut stack: Stack<Int32> = Stack();
    stack.push(4);
    inmut top: Result<Int32, EmptyCollectionError> = stack.pop();
    inmut peeked: Result<Int32, EmptyCollectionError> = stack.peek();

    mut priority: PriorityQueue<Int32> = PriorityQueue();
    priority.enqueue(6);
    priority.enqueue(5);
    inmut least: Result<Int32, EmptyCollectionError> = priority.peek();
    inmut removed_least: Result<Int32, EmptyCollectionError> = priority.dequeue();
    stdout.println((found && inserted && present && removed_from_set).to_string());
}
main();
```

`Array` never resizes; `fill`, `copy_from` and slice replacement preserve its
exact length. `List` grows automatically. Only `length` is public: allocation
capacity, reservation and shrinking remain runtime implementation details.
`clear()` is a logical operation and makes no observable capacity promise.

`Map.length` and `Set.length` have type `UInt64`. `Map.set` returns `Void`;
`Set.add` returns whether it inserted a new value. Updating a Map key preserves
its position, while removing and reinserting it appends it to the end.
Set's `for … in` loop visits an insertion-order snapshot.

Map keys and Set elements require coherent `Hash` and `Equal`; equal values
must hash equally. Mutable class references use object identity. In the current
runtime, accepted keys/elements preserve their complete declared storage
layout. Map values preserve their declared byte layouts. Nullable values,
tuples, and payload enums derive equality when every participating field has a
canonical stored equality representation. Text keys/elements use normalized
content, and floating signed zero is normalized before hashing and comparison.
Tuple text/Char fields, structural records, and unions have no canonical
aggregate equality descriptor yet, so the type checker rejects them as keys or
elements before lowering. Set union, intersection,
difference and symmetric difference preserve deterministic result order
without mutating their inputs.
Sets include union, intersection, difference, symmetric difference and subset,
superset/disjoint queries.

`Deque<T>` provides front/back pushes, pops and inspection, `clear()`,
`length: UInt64`, and `is_empty: Boolean`; empty pops and inspection return
`Result<T, EmptyCollectionError>`. `Queue<T>` exposes only FIFO `enqueue`,
`dequeue`, and `peek`; `Stack<T>` exposes only LIFO `push`, `pop`, and `peek`.
Both also provide `clear`, `length`, `is_empty`, and typed empty results.
`PriorityQueue<T>` accepts Boolean, Char, integer, Float16/32/64/128, Decimal,
and String values with a natural total order; it dequeues the least value and
preserves insertion order for ties. All four collection families invalidate
iterators after structural mutation. Growth failure raises catchable
`AllocationError` without partial mutation.

## Eager and lazy transformation

`List<T>` and `Array<T>` support eager, chainable `map`, `filter`, `flat_map`,
`reduce`, `take`, `skip`, and `reverse` directly. The last transform in the
chain returns the same collection family, and a terminal conversion such as
`to_array()`, `to_list()` or `to_set()` selects the result family:

```zirk
inmut names: List<String> = users
    .filter((user) => user.active)
    .map((user) => user.name);

inmut unique: Set<String> = users
    .filter((user) => user.active)
    .map((user) => user.name)
    .to_set();
```

This is the default idiom for in-memory collections: it is readable, the
intermediate collections are explicit, and the type of the final expression is
clear from the last method.

Iterator transformations are lazy and allocate only at an explicit terminal.
Use them when you want single-pass streaming or to avoid intermediate
allocations:

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
explicit check or the delivered `.parallel` collection adapter. Task-aware
streams remain separate from `Iterator` so iteration never hides suspension. Generator cleanup runs when
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
