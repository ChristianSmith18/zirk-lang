# Ranges

Ranges describe an ordered interval and are commonly consumed by loops, slicing, and collection APIs.

```zirk
for index in 0..10 {
    stdout.println("{index}");
}
```

`..` excludes the upper bound and `..=` includes it:

```zirk
0..10;  // 0 through 9
0..=10; // 0 through 10
```

Ranges are `Range<T>` values: `start`, `end` and `step` are readable, and the
element type `T` is numeric or `Duration`. A third `..` operand supplies the
step, which may be negative to walk down:

```zirk
0..10..2;   // 0, 2, 4, 6, 8
5..0..-1;   // 5, 4, 3, 2, 1
0s..5s..1s; // five Duration values: 0s, 1s, 2s, 3s, 4s
```

A range is a value, so it can be stored, passed and sliced (`r[lo:hi:step]`
slices the element sequence, returning a new `Range`); `.reverse()` returns
the same elements produced last to first:

```zirk
mut r: Range<Int32> = 2..9;
for i in r.reverse() {
    // 8 down to 2
}
for i in r[2:5] {
    // 4, 5, 6
}
```

A step of `0` can never advance and throws `InvalidStepError`. Ranges implement
`Iterable<T>`: `for x in range` iterates them directly, whether the range is a
literal or a stored value.

---

**Previous:** [← Pipe Operator](09-pipe-operator.md) · **Next:** [ Operator Overloading](11-operator-overloading.md)
