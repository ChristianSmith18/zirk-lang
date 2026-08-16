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

Ranges are lazy `Range<T>` values. A step changes the distance between produced
values, and a descending pair walks toward its end:

```zirk
0..10.step(2); // 0, 2, 4, 6, 8
10..0;         // 10 down to 1
10..=0;        // 10 down to 0
```

A computed bound may use `{}` in an inline range expression:

```zirk
inmut number = 10;
for index in 0..{number}.step(1) {
    stdout.println(index);
}
```

The direction follows the bounds. `step` is a positive distance; use
`.reverse()` to invert an already constructed range. Ranges implement
`Iterable<T>` and work with iteration and functional operations without first
allocating an array.

---

**Previous:** [← Pipe Operator](09-pipe-operator.md) · **Next:** [ Operator Overloading](11-operator-overloading.md)
