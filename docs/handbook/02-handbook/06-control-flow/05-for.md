# `for`

Zirk provides both the traditional counter form and `for ... in` iteration.

```zirk
for mut index = 0; index < 10; index++ {
    process(index);
}
```

The initializer runs once, the Boolean condition is checked before each
iteration, and the update runs after each completed body. Use it when a loop
needs explicit state or a non-range update.

For an ordinary progression, prefer a range:

```zirk
for index in 0..10.step(2) {
    process(index);
}
```

Any loop must define progress. A counter loop that never changes its condition is better expressed as `loop` if nontermination is intentional.

`for ... in` accepts every `Iterable<T>`, including arrays, lists, strings,
ranges, and user-defined iterable types.

---

**Previous:** [← Exhaustiveness](04-exhaustiveness.md) · **Next:** [ for ... in](06-for-in.md)
