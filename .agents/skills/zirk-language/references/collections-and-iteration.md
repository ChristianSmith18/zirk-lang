# Collections and iteration

Read this whenever a user asks to create, fill, traverse, search, slice, or
transform a sequence. Confirm the desired mutability and whether the result
must have a fixed length before selecting a type.

## Choose the container

| User need | Preferred form | Why |
| --- | --- | --- |
| Fixed known values or fixed length | `Array<T>` / `[values]` | Arrays do not resize. |
| Fixed allocation, then indexed writes | `T[n]` | Allocates exactly `n` default-initialized slots. |
| Size can grow or shrink | `List<T>` / `List(...)` | Lists support structural operations such as `add`. |
| Regular finite numeric sequence | `Range<T>` | Lazy, iterable, and expandable into arrays/lists. |
| Keyed or unique membership | `Map<K, V>` / `Set<T>` | Read the owning handbook API before use. |

An unannotated `[...]` literal defaults to `Array<T>`. An expected
`List<T>` makes the same literal a list:

```zirk
inmut fixed = [1, 2, 3];
inmut growing: List<Int32> = [1, 2, 3];
```

## Ranges: the concise sequence form

| Need | Form |
| --- | --- |
| End excluded | `1..100` |
| End included | `1..=100` |
| Explicit signed step | `0..10:2`, `10..0:-2` |
| Dynamic bound or step | `0..{limit}:{step}` |
| Descending unit sequence | `10..0` |

Ranges accept integer scalar families and `Duration`. A non-literal start,
end, or step must use braces. The step is never zero. Do **not** use legacy
`start..end..step`, `.step(...)`, or `.reverse()`.

## Decision example: “make a list of 1 to 100”

The word *list* means resizable `List<Int32>`. The values form an inclusive
regular range, so expand that range directly instead of adding each element in
a traditional loop:

```zirk
inmut numbers: List<Int32> = List(1..=100);
```

Equivalent context-driven form:

```zirk
inmut numbers: List<Int32> = [1..=100];
```

Use a loop only when construction has per-item side effects or a non-regular
update:

```zirk
mut numbers: List<Int32> = List();
for mut value = 1; value <= 100; value++ {
    numbers.add(value);
}
```

If later growth is forbidden, use `Array<Int32>`/`[1..=100]`. If the purpose is
preallocation for indexed writes, use `Int32[100]`; it is not a list and is
not initialized with a range.

## Access, slices, and iteration

```zirk
for value in numbers {
    stdout.println(value);
}

inmut middle = numbers[10:20];
inmut reversed = numbers[::-1];
```

Indexing and slices yield independent logical values; an indexed expression on
the left side of `=` writes the container. Slices are independent copies,
their end is exclusive, and slice assignment cannot resize a collection.
Structural mutation invalidates an existing iterator. Read the collections
handbook before relying on `map`, `filter`, `reduce`, safe access, or a
specific `List`/`Map` method signature.
