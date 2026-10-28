# Lists

`List<T>` is a dynamically sized, ordered, native reference collection. It keeps
elements in insertion order, grows on demand, and supports indexed access,
insertion, removal, and iteration.

Because a list is a managed reference, assigning one name to another shares the
same list. Clone it when you need an independent copy.

## Construction

```zirk
inmut empty: List<Int32> = List();
inmut names = List("Ada", "Grace", "Linus");
```

`List<T>()` or `List(...)` creates an empty or pre-populated list. The element
type is usually explicit or inferred from the initial values.

A `List<T>` annotation also selects list storage for `[...]`; ranges expand in
place in literals and constructor calls:

```zirk
inmut values: List<Int32> = [0 .. 3];
inmut more: List<Int32> = List(-1, 3 .. 6, 6);
```

## Growth, insertion, and removal

```zirk
inmut numbers: List<Int32> = List(10, 20);

numbers.add(30); // [10, 20, 30]
numbers.insert(0, 5); // [5, 10, 20, 30]
numbers.remove(1); // [5, 20, 30]

numbers.length; // 3
numbers.is_empty(); // false
```

Insertion and removal shift the later elements. Indexing is bounds-checked: an
out-of-range read or write is a controlled error.

`List<T>` manages its capacity internally: it grows geometrically while values
are appended and contracts only after becoming substantially sparse. There is
no user-facing capacity setting; use `Array<T>` when the final size is fixed.

```zirk
numbers[0]; // 5
numbers[10]; // error: index out of bounds
```

## Sharing and cloning

```zirk
inmut first: List<Int32> = List(1, 2, 3);
inmut second = first;

second.add(4);

// first and second both contain [1, 2, 3, 4]
```

The assignment makes `second` another name for the same list. To get an
independent list, call `clone()`:

```zirk
inmut independent = first.clone();
independent.add(5);

// first is [1, 2, 3, 4]
// independent is [1, 2, 3, 4, 5]
```

## Mutation and binding permissions

A `mut` list can be both rebound and structurally edited. An `inmut` list cannot
be rebound but still permits permitted referent mutation. `inmut::strict`
forbids even that:

```zirk
mut a: List<Int32> = List(1, 2, 3); // this example demonstrates rebinding
a.add(4); // valid
a = List(); // valid

inmut b: List<Int32> = List(1, 2, 3);
b.add(4); // valid: the referent may change
b = List(); // error: binding cannot be reassigned

inmut::strict c: List<Int32> = List(1, 2, 3);
c.add(4); // error: no mutable alias is allowed
```

## Iteration

```zirk
mut total = 0;
for value in numbers {
    total += value;
}
```

Lists implement `Iterable<T>`. Modifying the list while iterating over it is a
controlled error because indexes become invalid.

```zirk
for value in numbers {
    numbers.add(value); // error: mutation during iteration
}
```

## Equality

`List` equality is element-wise when `T` is `Equatable`:

```zirk
inmut a: List<Int32> = List(1, 2, 3);
inmut b: List<Int32> = List(1, 2, 3);

a == b; // true
a is b; // false: two different list instances
```

## Transformations

`List<T>` supports eager, chainable transformations directly on the collection.
The last transform returns a new `List`:

```zirk
inmut numbers: List<Int32> = List(1, 2, 3, 4, 5);

inmut doubled = numbers.map((n) => n * 2);           // [2, 4, 6, 8, 10]
inmut evens = numbers.filter((n) => n.is_even());  // [2, 4]
inmut sum = numbers.reduce(0, (acc: Int32, n: Int32): Int32 => acc + n);  // 15

inmut names = users
    .filter((user) => user.active)
    .map((user) => user.name);
```

For custom lazy, single-pass sources that implement `Iterable<T>`, use
`for ... in`. A range can materialize its sequence pipeline with `collect()`:

```zirk
inmut squares: List<Int32> =(1 ..= 3)
.map((n: Int32): Int32 => n * n)
.collect();
```

## When to choose a list

- Choose `List<T>` when the number of elements changes at runtime.
- Choose `Array<T>` or `T[N]` when the size is fixed.
- Choose an iterator or `Range<T>` when consumers should not depend on storage.

## API

Dynamically sized ordered sequence; keeps insertion order and supports eager chainable transformations.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `UInt64` | Element count | implemented |
| `is_empty` | `Boolean` | `length == 0` | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `List()` / `List(e0, e1, …)` | `List<T>` | Empty or pre-populated list | implemented |
| `l[i]` / `l[i] = v` | `T` / `Void` | Checked indexing | implemented — negative-from-end indices resolve as `length + index` |
| `l.add(value)` | `Void` | Amortized-constant append | implemented |
| `l.insert(index, value)` | `Void` | Shifting insertion | implemented |
| `l.remove(index)` | `T` | Removes and returns element at `index` | implemented |
| `l.remove_at(index)` | `T` | Named form of index removal | specified |
| `l.remove(value)` | `Boolean` | Removes first equal element | implemented |
| `l.splice(start, delete_count, replacement)` | `Void` | General splice | specified |
| `l.clear()` | `Void` | Logical emptying; no capacity promise | specified |
| `l.get(i)` | `Result<T, IndexOutOfBoundsError>` (or `T?` in nullable value context) | Safe lookup; preserve or project absence | implemented |
| `l.contains(v)` | `Boolean` | Membership | implemented |
| `l.find(pred)` | `T?` | First matching element | specified |
| `l.map(fn)` | `List<U>` | Eager transform; `U` from `fn` return | implemented |
| `l.filter(fn)` | `List<T>` | Eager selection | implemented |
| `l.for_each(fn)` | `Void` | Eager side-effecting traversal | implemented |
| `l.flat_map(fn)` | `List<U>` | Map then flatten one level | specified |
| `l.reduce(initial, fn)` | `U` | Left fold over elements | implemented |
| `l.sum()` / `l.count()` | numeric `T` / `Int32` | Numeric sum / element count | implemented |
| `l.take(n)` | `List<T>` | First `n` elements, or fewer | specified |
| `l.skip(n)` | `List<T>` | Elements after the first `n` | specified |
| `l.reverse()` | `Void` | Reverse this list in place | implemented |
| `l.first()` / `l.last()` / `l.pop()` | `T?` | Read endpoints or remove the last item | implemented |
| `l.to_array()` | `Array<T>` | Materializes as a fixed array | specified |
| `l.to_set()` | `Set<T>` | Materializes as a set (`T` must satisfy `Hash` + `Equal`) | specified |
| `l.sort()` / `l.sort_by(cmp)` | `Void` | Stable natural or comparator sort in place | implemented |
| `l.collect()` | `List<T>` | Materialize a sequence pipeline as a list | implemented |
| `l.iterator()` | `Iterator<T>` | Lazy single-pass iterator | implemented |
| `l.clone()` | `List<T>` | Independent logical copy | implemented |
| `l.to_string()` | `String` | Rendering | implemented |

Mutation through `inmut::strict` and mutation during iteration are controlled
errors.

### Examples

```zirk
mut numbers: List<Int32> = List(10, 20);
numbers.add(30);                // [10, 20, 30]
numbers.insert(0, 5);           // [5, 10, 20, 30]
numbers.remove(1);              // [5, 20, 30]

// Eager chain on the collection itself; the last transform returns a List.
inmut names: List<String> = users
    .filter((user) => user.active)
    .map((user) => user.name);

// Lazy alternative when you want to avoid intermediate allocations or stream data.
inmut eager_from_lazy: List<String> = users
    .iterator()
    .filter((user) => user.active)
    .map((user) => user.name)
    .collect();
```

## Implementation status

> `List<T>` is delivered as a resizable native reference collection with
> indexed read/write (including negative-from-end indices), `add`, `insert`,
> `remove(index)`, `remove(value)`, `clone()`, `to_string()`, `length`,
> `is_empty`, and `for ... in`. Pre-populated `List(e0, …)` literals are also
> implemented. Eager `map`, `filter`, `for_each`, `reduce`, `sum`, `count`,
> in-place `reverse`/`sort`, `first`, `last`, `pop`, and sequence `collect()`
> are delivered. `flat_map`, `take`, `skip`, and family conversions
> (`to_array`, `to_set`) remain specified.

---

**Previous:** [← Fixed Arrays](02-fixed-arrays.md) · **Next:** [ Maps](04-maps.md)
