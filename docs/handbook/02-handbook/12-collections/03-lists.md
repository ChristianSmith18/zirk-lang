# Lists

`List<T>` is a dynamically sized, ordered, native reference collection. It keeps
elements in insertion order, grows on demand, and supports indexed access,
insertion, removal, and iteration.

Because a list is a managed reference, assigning one name to another shares the
same list. Clone it when you need an independent copy.

## Construction

```zirk
mut empty: List<Int32> = List();
mut names = List("Ada", "Grace", "Linus");
```

`List<T>()` or `List(...)` creates an empty or pre-populated list. The element
type is usually explicit or inferred from the initial values.

## Growth, insertion, and removal

```zirk
mut numbers = List(10, 20);

numbers.add(30);              // [10, 20, 30]
numbers.insert(0, 5);         // [5, 10, 20, 30]
numbers.remove(1);            // [5, 20, 30]

numbers.length;               // 3
numbers.is_empty();           // false
```

Insertion and removal shift the later elements. Indexing is bounds-checked: an
out-of-range read or write is a controlled error.

```zirk
numbers[0];      // 5
numbers[10];     // error: index out of bounds
```

## Sharing and cloning

```zirk
mut first = List(1, 2, 3);
mut second = first;

second.add(4);

// first and second both contain [1, 2, 3, 4]
```

The assignment makes `second` another name for the same list. To get an
independent list, call `clone()`:

```zirk
mut independent = first.clone();
independent.add(5);

// first is [1, 2, 3, 4]
// independent is [1, 2, 3, 4, 5]
```

## Mutation and binding permissions

A `mut` list can be both rebound and structurally edited. An `inmut` list cannot
be rebound but still permits permitted referent mutation. `inmut::strict`
forbids even that:

```zirk
mut a = List(1, 2, 3);
a.add(4);        // valid
a = List();      // valid

inmut b = List(1, 2, 3);
b.add(4);        // valid: the referent may change
b = List();      // error: binding cannot be reassigned

inmut::strict c = List(1, 2, 3);
c.add(4);        // error: no mutable alias is allowed
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
mut a = List(1, 2, 3);
mut b = List(1, 2, 3);

a == b; // true
a is b; // false: two different list instances
```

## Transformations

`List<T>` supports eager, chainable transformations directly on the collection.
The last transform returns a new `List`; a terminal conversion changes the family:

```zirk
mut numbers = List(1, 2, 3, 4, 5);

inmut doubled = numbers.map((n) => n * 2);           // [2, 4, 6, 8, 10]
inmut evens = numbers.filter((n) => n.is_even());  // [2, 4]
inmut sum = numbers.reduce((acc, n) => acc + n, 0);  // 15

inmut names = users
    .filter((user) => user.active)
    .map((user) => user.name);
```

For lazy, single-pass pipelines, use `.iterator()` and a terminal such as
`.collect()`:

```zirk
inmut lazy_names = users
    .iterator()
    .filter((user) => user.active)
    .map((user) => user.name)
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
| `List()` / `List(e0, e1, …)` | `List<T>` | Empty or pre-populated list | implemented — `List()` empty only; the `List(e0, …)` form is specified |
| `l[i]` / `l[i] = v` | `T` / `Void` | Checked indexing | implemented — negative-from-end indices are specified; today they are bounds errors |
| `l.add(value)` | `Void` | Amortized-constant append | implemented |
| `l.insert(index, value)` | `Void` | Shifting insertion | implemented |
| `l.remove(index)` | `T` | Removes and returns element at `index` | implemented |
| `l.remove_at(index)` | `T` | Named form of index removal | specified |
| `l.remove(value)` | `Boolean` | Removes first equal element | specified |
| `l.splice(start, delete_count, replacement)` | `Void` | General splice | specified |
| `l.clear()` | `Void` | Logical emptying; no capacity promise | specified |
| `l.get(i)` / `l.get_or_null(i)` | `Result<T, BoundsError>` / `T?` | Explicit-absence access | specified |
| `l.contains(v)` / `l.find(pred)` | `Boolean` / `T?` | Search | specified |
| `l.map(fn)` | `List<U>` | Eager transform; `U` from `fn` return | specified |
| `l.filter(fn)` | `List<T>` | Eager selection | specified |
| `l.flat_map(fn)` | `List<U>` | Map then flatten one level | specified |
| `l.reduce(fn, initial)` | `U` | Left fold over elements | specified |
| `l.take(n)` | `List<T>` | First `n` elements, or fewer | specified |
| `l.skip(n)` | `List<T>` | Elements after the first `n` | specified |
| `l.reverse()` | `List<T>` | Reversed copy | specified |
| `l.to_array()` | `Array<T>` | Materializes as a fixed array | specified |
| `l.to_set()` | `Set<T>` | Materializes as a set (`T` must satisfy `Hash` + `Equal`) | specified |
| `l.sort()` / `l.sort_by(fn)` / `l.sort_with(cmp)` / `l.sort_unstable()` | `Void` | Stable sort in place | specified |
| `l.iterator()` | `Iterator<T>` | Lazy single-pass iterator; use `collect()`/`to_list()` to terminate | specified |
| `l.clone()` | `List<T>` | Independent logical copy | specified |
| `l.to_string()` | `String` | Rendering | specified |

Mutation through `inmut::strict` and mutation during iteration are controlled
errors.

### Examples

```zirk
mut numbers = List(10, 20);
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
indexed read/write, `add`, `insert`, `remove`, `length`, `is_empty`, and
>`for ... in`. Eager chainable `map`, `filter`, `flat_map`, `reduce`, `take`,
>`skip`, `reverse`, and family conversions (`to_array`, `to_set`) are specified
>but remain ahead of the current compiler. Use `iterator()` and `collect()` for
>the delivered lazy pipeline today.

---

**Previous:** [← Fixed Arrays](02-fixed-arrays.md) · **Next:** [ Maps](04-maps.md)
