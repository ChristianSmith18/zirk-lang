# Collection Contracts and Complexity

Choose a collection by its invariant, not only its literal syntax.

| Family | Shape | Order | Structural mutation | Equality | Typical access |
|---|---|---|---|---|---|
| `Array<T>` | contiguous dynamic reference | insertion order | supported | ordered elements | index O(1) |
| `T[N]` | fixed-size contiguous reference | index order | element only | ordered elements | index O(1) |
| `List<T>` | growable reference sequence | insertion order | supported | ordered elements | index O(1) |
| `Map<K,V>` | key/value reference container | stable documented iteration | supported | mappings | average lookup O(1) |
| `Set<T>` | unique reference container | stable documented iteration | supported | membership | average lookup O(1) |
| `Deque<T>` | growable double-ended sequence | front to back | both ends | ordered elements | end operations amortized O(1) |
| `Queue<T>` | FIFO sequence | enqueue order | enqueue/dequeue | ordered elements | operations amortized O(1) |
| `Stack<T>` | LIFO sequence | top to bottom | push/pop | ordered elements | operations amortized O(1) |
| `PriorityQueue<T>` | stable minimum heap | dequeue priority order | enqueue/dequeue | natural total order | enqueue/dequeue O(log n) |
| `Range<T>` | lazy value progression | progression order | none | definition | iteration O(n) |

Complexities describe ordinary operation, excluding the cost of deep-cloning a
projected result. Allocation overflow, capacity overflow, invalid bounds,
missing direct access, and iterator invalidation are controlled errors rather
than undefined behavior.

## Core APIs

`Array` and `List` provide `length`, `capacity` where meaningful, `get`,
`append`, `insert`, `remove_at`, `contains`, `clear`, `clone`,
iteration, and sequence conversion. Fixed arrays expose `length`, checked
indexing, element replacement, `clone`, iteration, and conversion, but reject
append, removal, and resizing.

`Map` provides checked `[]`, `get`, `get_or_insert_with`,
`contains_key`, `insert`, `remove`, `keys`, `values`, `entries`, `length`,
`clear`, and `clone`. Keys satisfy equality and hashing contracts. A default or
factory is evaluated only when the key is absent.

`Set` provides `contains`, `insert`, `remove`, `union`, `intersection`,
`difference`, `symmetric_difference`, subset/superset tests, `length`, `clear`,
iteration, and `clone`. Mathematical operations require compatible element
types and their equality/hash contracts.

`Deque<T>()` provides `push_front(T): Void`, `push_back(T): Void`,
`pop_front(): Result<T, EmptyCollectionError>`,
`pop_back(): Result<T, EmptyCollectionError>`, `front()` and `back()` with the
same result type, `clear(): Void`, `length: UInt64`, and `is_empty: Boolean`.
Iteration is from front to back. `Queue<T>()` exposes `enqueue(T): Void`,
`dequeue(): Result<T, EmptyCollectionError>`, `peek()` with that result type,
`clear(): Void`, `length`, and `is_empty`; it iterates in FIFO order. `Stack<T>()`
exposes `push(T): Void`, `pop(): Result<T, EmptyCollectionError>`, `peek()` with
that result type, `clear(): Void`, `length`, and `is_empty`; it iterates from
top to bottom. These restricted APIs reject operations from the other sequence
families.

`PriorityQueue<T>()` requires a supported total-order type. It provides
`enqueue(T): Void`, `peek(): Result<T, EmptyCollectionError>`,
`dequeue(): Result<T, EmptyCollectionError>`, `length: UInt64`, and
`is_empty: Boolean`. The least value is removed first; equal values preserve
enqueue order. Supported element types are Boolean, Char, signed and unsigned
integers through 128 bits, Float16/32/64/128, Decimal, and String. Nullable and
user-defined types without an explicit total-order implementation are rejected.

## Access and slicing

`String`, tuple, array, and list indexes accept negatives. Direct missing
access reports a typed error. `get` returns `Result`; nullable value contexts
project that same result to the value or null.

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

Deque end operations, Queue operations, and Stack operations are amortized
O(1). PriorityQueue uses a stable binary min-heap: enqueue and dequeue are
O(log n), peek is O(1), and its lazily cached ordered iteration view costs
O(n log n) time and O(n) temporary storage. Map and Set lookup/mutation are
expected O(1); iteration is O(n), and set algebra is expected O(n + m).
These bounds assume constant-time element comparison, hashing, and equality.

The focused runtime benchmark can be run with
`cargo test -p zirk-runtime --lib collection_operation_costs -- --ignored --nocapture`.
One local debug-profile run measured these combined operations:

| Elements | Deque back-push/front-pop | Queue/Stack FIFO/LIFO | PriorityQueue enqueue/dequeue | Map/Set insert and lookup |
|---:|---:|---:|---:|---:|
| 1,000 | 1.19 ms | 0.25 ms | 2.83 ms | 4.54 ms |
| 10,000 | 1.92 ms | 1.87 ms | 27.14 ms | 25.39 ms |
| 100,000 | 11.54 ms | 11.32 ms | 240.16 ms | 222.54 ms |

These timings are an implementation sanity check, not a cross-machine
performance promise. The priority heap trades constant-time sorted-vector
insertion for logarithmic updates; the first ordered traversal builds one
O(n)-space cache and subsequent element reads reuse it until mutation.

---

**Previous:** [← Safe Collection Access](09-safe-access.md) · **Next:** [Iteration and Functional Style →](../13-iteration-and-functional-style/README.md)
