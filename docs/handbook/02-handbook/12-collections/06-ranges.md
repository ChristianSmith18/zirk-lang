# Ranges

A range represents a progression between bounds and integrates with iteration and slicing.

```zirk
for index in 0..10 {
    process(index);
}
```

`..` excludes the end and `..=` includes it:

```zirk
0..10;              // 0 through 9
0..=10;             // 0 through 10
0..10.step(2);      // 0, 2, 4, 6, 8
10..0;              // 10 down to 1
10..=0;             // 10 down to 0
(0..10).reverse();  // 9 down to 0
```

Bounds may be computed:

```zirk
inmut number = 10;
for index in 0..{number}.step(1) {
    process(index);
}
```

Direction follows the relative bounds and `step` supplies the positive distance
between values. A zero step is invalid. A range is lazy and implements
`Iterable<T>`, so functional operations do not require first materializing an
array. Ranges are independent from slicing.

---

**Previous:** [← Sets](05-sets.md) · **Next:** [ Indexing](07-indexing.md)
