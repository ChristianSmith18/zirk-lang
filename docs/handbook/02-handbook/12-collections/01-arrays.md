# Arrays

An `Array<T>` stores a fixed number of ordered values of the same element type.
Its length never changes after construction. Elements can be replaced, but they
cannot be appended or removed.

## Construction

```zirk
inmut names: Array<String> = ["Ada", "Grace", "Linus"];

inmut explicit: Array<Int32> = Array(1, 2, 3, 4);

mut sized: String[8]; // canonical explicit-size form
mut also: Array<String>(8); // supported alternative
```

The literal `[...]` infers the fixed length. `T[n]` and `Array<T>(n)` allocate
`n` default-initialized elements. All forms produce a fixed-size array.

Range elements expand before that length is fixed, preserving source order:

```zirk
inmut values: Array<Int32> = [9, 0 .. 3, 10];
inmut also: Array<Int32> = Array(0 .. 2);
// values is [9, 0, 1, 2, 10]; also is [0, 1]
```

## Indexing and replacement

```zirk
mut values: Int32[4] = [10, 20, 30, 40];

values[0]; // 10
values[1] = 25; // [10, 25, 30, 40]
values[-1]; // 40
values[10]; // error: index out of bounds
```

Bounds are checked in safe code. A negative index counts from the end.

## Sharing and cloning

```zirk
inmut first: String[3] = ["a", "b", "c"];
inmut second = first;

second[0] = "X";

// first and second both contain ["X", "b", "c"]
```

Arrays are managed references. Assignment aliases the same array. To get an
independent copy, use `clone()`:

```zirk
inmut independent = first.clone();
independent[0] = "Y";

// first is ["X", "b", "c"]
// independent is ["Y", "b", "c"]
```

## Slicing

A slice creates a view or a copy depending on the contract. In safe code, slices
copy their elements so that `inmut::strict` sources remain protected:

```zirk
inmut numbers: Int32[5] = [1, 2, 3, 4, 5];
inmut middle = numbers[1: 4]; // [2, 3, 4]

middle[0] = 99;
// numbers is still [1, 2, 3, 4, 5]
```

Only an explicit read-only view shares a subregion. No mutable view may weaken
an `inmut::strict` source.

## Fixed size in the type

A declaration that makes the size part of the type communicates an additional
compile-time contract. It does not select a different resizable collection.

```zirk
fn checksum(bytes: UInt8[16]): UInt32 { ... }

inmut header: UInt8[16] = ...;
checksum(header);         // valid
checksum(some_array);     // error if the array size is not 16
```

## Iteration

```zirk
for value in values {
    process(value);
}
```

Arrays implement `Iterable<T>`. Mutation during iteration is a controlled error.

## Equality

Array equality is element-wise when `T` is `Equatable`:

```zirk
inmut a: Int32[3] = [1, 2, 3];
inmut b: Int32[3] = [1, 2, 3];

a == b; // true
a is b; // false: two different array instances
```

## Transformations

`Array<T>` supports the same eager, chainable transformations as `List<T>`:

```zirk
inmut values: Int32[4] = [1, 2, 3, 4];

inmut doubled = values.map((n) => n * 2);          // Array<Int32>
inmut evens = values.filter((n) => n.is_even());   // Array<Int32>
inmut sum = values.reduce(0, (acc: Int32, n: Int32): Int32 => acc + n); // Int32
```

`map` on a fixed-size `T[N]` produces a same-length array; `filter` produces
an `Array<T>` whose length is determined at runtime. `reverse` mutates the
array in place. `take` and `skip` remain specified. Use `to_list()` when you
need a resizable collection once that conversion is available.

The compiler lowers `Array.map` directly into one exactly-sized contiguous
allocation and indexed writes. It does not use a growable `List` buffer, so it
is the preferred representation when the cardinality is stable.

Sequence pipelines execute eagerly and preserve the array result family for
`map`/`filter`; `collect()` materializes the values as a `List<T>`.

## When to choose an array

- Choose `Array<T>` or `T[n]` when the size is fixed and known.
- Choose `List<T>` when the size may grow or shrink.
- Choose fixed-size `T[n]` when the size is part of a protocol, native layout,
  or function contract.

## Relationship to fixed arrays

[Fixed Arrays](02-fixed-arrays.md) explains the same concept with emphasis on
explicit-size types and native interoperation. The distinction is style: use
`Array<T>` for general fixed collections and `T[n]` when the size is a visible
part of the contract.

## API

Fixed-length ordered storage. Elements can be replaced, never appended or
removed. A managed reference: assignment aliases, `clone()` separates.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `UInt64` | Fixed element count | implemented |
| `is_empty` | `Boolean` | `length == 0` | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Array(e0, e1, …)` / `[e0, e1, …]` | `Array<T>` | Literal construction; infers fixed length | implemented |
| `Array(n)` | `Array<T>` | `n` default-initialized elements; `T` comes from the binding annotation | implemented |
| `a[i]` | `T` | Checked indexing | implemented — negative-from-end indices resolve as `length + index` |
| `a.get(i)` | `Result<T, IndexOutOfBoundsError>` (or `T?` in nullable value context) | Safe lookup; preserve or project absence | implemented |
| `a[i] = v` | `Void` | Checked element replacement | implemented |
| `a[start:end:step]` | `Array<T>` | Copying slice (safe code) | implemented |
| `a.fill(value)` | `Void` | Overwrite every element | specified |
| `a.copy_from(source)` | `Void` | Equal-length bulk copy | specified |
| `a.contains(value)` | `Boolean` | Membership | implemented |
| `a.find(predicate)` | `T?` | First matching element | specified |
| `a.map(fn)` | `Array<U>` | Eager transform; result length matches source | implemented |
| `a.filter(fn)` | `Array<T>` | Eager selection; result length is dynamic | implemented |
| `a.for_each(fn)` | `Void` | Eager side-effecting traversal | implemented |
| `a.flat_map(fn)` | `Array<U>` | Map then flatten one level | specified |
| `a.reduce(initial, fn)` | `U` | Left fold over elements | implemented |
| `a.sum()` / `a.count()` | numeric `T` / `Int32` | Numeric sum / element count | implemented |
| `a.take(n)` | `Array<T>` | First `n` elements | specified |
| `a.skip(n)` | `Array<T>` | Elements after the first `n` | specified |
| `a.reverse()` | `Void` | Reverse this array in place | implemented |
| `a.first()` / `a.last()` | `T?` | Read the first or last element | implemented |
| `a.sort()` / `a.sort_by(cmp)` | `Void` | Stable natural or comparator sort in place | implemented |
| `a.collect()` | `List<T>` | Materialize a sequence pipeline as a list | implemented |
| `a.to_list()` | `List<T>` | Materializes as a list | specified |
| `a.to_set()` | `Set<T>` | Materializes as a set | specified |
| `a.iterator()` | `Iterator<T>` | Lazy single-pass iterator | implemented |
| `a.clone()` | `Array<T>` | Independent logical copy | implemented |
| `a.to_string()` | `String` | Rendering | implemented |

`fill`, `copy_from`, and slice replacement preserve exact length. Structural
mutation during iteration is a controlled error (`IteratorInvalidatedError`).

### Examples

```zirk
mut values: Int32[4] = [10, 20, 30, 40];
values[1] = 25;
values[-1];                     // 40
mut middle = values[1:4];       // copy: [25, 30, 40]

// Eager chain on the array itself.
mut doubled = values.map((x) => x * 2);            // Array<Int32>
mut evens = values.filter((x) => x.is_even());    // Array<Int32>, dynamic length
mut sum = values.reduce(0, (acc: Int32, x: Int32): Int32 => acc + x);  // Int32

mut copy = values.clone();
copy is values;                 // false
```

---

**Previous:** [← Collections](README.md) · **Next:** [ Fixed Arrays](02-fixed-arrays.md)
