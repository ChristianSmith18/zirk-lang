# Ranges

A range represents a progression between bounds and integrates with iteration and slicing.

```zirk
for index in 0..10 {
    process(index);
}
```

`..` excludes the end and `..=` includes it. A colon supplies an explicit step;
without one, the step is inferred from the bounds:

```zirk
0..10;              // 0 through 9
0..=10;             // 0 through 10
0..10:2;            // 0, 2, 4, 6, 8
10..0;              // 10 down to 1
9..0:-1;            // 9 down to 1
```

Bounds and step may be computed, and the element type is `Int32` for integer
ranges or `Duration` for duration ranges (`0s..5s:1s`). Non-literal operands
must be enclosed in braces:

```zirk
inmut number = 10;
for index in 0..{number}:1 {
    process(index);
}
```

`start`, `end` and `step` are readable members, and a range slices like a
sequence — `r[1:4]` is the range of `r`'s second through fourth elements:

```zirk
mut r: Range<Int32> = 0..10;
r[2:6];   // 2, 3, 4, 5
r[::2];   // 0, 2, 4, 6, 8
```

A zero step throws `InvalidStepError`. A range is lazy and implements
`Iterable<T>`, so functional operations do not require first materializing an
array.

## API

A lazy, finite arithmetic progression; value-like, implements `Iterable<T>`.

> **Implementation status:** delivered for integer and `Duration` ranges,
> including collection expansion and fixed-array allocation.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `start` | `T` | First bound | specified |
| `end` | `T` | Last bound | specified |
| `is_inclusive` | `Boolean` | `..=` vs `..` | specified |
| `is_empty` | `Boolean` | No values to yield | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `a..b` | `Range<T>` | Exclusive end; direction inferred when step is omitted | delivered |
| `a..=b` | `Range<T>` | Inclusive end | delivered |
| `a..b:step` | `Range<T>` | Explicit non-zero signed step | delivered |
| `r.contains(value)` | `Boolean` | Bounds test | specified |
| `r.iterator()` | `Iterator<T>` | Lazy iteration | specified |
| `r.to_string()` | `String` | Definition rendering | specified |

Direction follows the relative bounds (`10..0` descends). `Range` always has a
finite end — there is no `Range.from(0)`. Equality compares the definition, not
the yielded sequence.

### Examples

```zirk
for index in 0..10:2 { process(index); }   // 0, 2, 4, 6, 8
for index in 10..=0 { process(index); }     // descending
for index in 9..0:-1 { process(index); }    // 9 down to 1
```

---

**Previous:** [← Sets](05-sets.md) · **Next:** [ Indexing](07-indexing.md)
