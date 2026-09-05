# Ranges

A range represents a progression between bounds and integrates with iteration and slicing.

```zirk
for index in 0..10 {
    process(index);
}
```

`..` excludes the end and `..=` includes it. A second `..` operand supplies the
step, which may be negative; `.reverse()` inverts a constructed range:

```zirk
0..10;              // 0 through 9
0..=10;             // 0 through 10
0..10..2;           // 0, 2, 4, 6, 8
10..0..-1;          // 10 down to 1
mut r = 0..10;
r.reverse();        // 9 down to 0
```

Bounds and step may be computed, and the element type is `Int32` for integer
ranges or `Duration` for duration ranges (`0s..5s..1s`):

```zirk
inmut number = 10;
for index in 0..number..1 {
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

> Entirely `specified` — `Range<T>` is pending (`Range<T>` is explicitly listed
> as remaining work in the feature status).

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
| `a..b` | `Range<T>` | Exclusive end | specified |
| `a..=b` | `Range<T>` | Inclusive end | specified |
| `r.step(n)` | `Range<T>` | Positive distance between values; zero is invalid | specified |
| `r.reverse()` | `Range<T>` | Reversed direction | specified |
| `r.contains(value)` | `Boolean` | Bounds test | specified |
| `r.iterator()` | `Iterator<T>` | Lazy iteration | specified |
| `r.to_string()` | `String` | Definition rendering | specified |

Direction follows the relative bounds (`10..0` descends). `Range` always has a
finite end — there is no `Range.from(0)`. Equality compares the definition, not
the yielded sequence.

### Examples

```zirk
for index in 0..10.step(2) { process(index); }   // 0, 2, 4, 6, 8
for index in 10..=0 { process(index); }          // descending
(0..10).reverse();                                // 9 down to 0
```

---

**Previous:** [← Sets](05-sets.md) · **Next:** [ Indexing](07-indexing.md)
