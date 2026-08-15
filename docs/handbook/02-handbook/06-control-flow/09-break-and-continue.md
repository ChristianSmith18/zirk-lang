# `break` and `continue`

`break` exits the nearest applicable loop. `continue` skips the remainder of the current iteration and begins the next one.

```zirk
for item in items {
    if item.is_invalid { continue; }
    if item.is_terminal { break; }
    process(item);
}
```

Both statements must occur in a loop context. They do not bypass resource guarantees: leaving a `match with` scope still closes its resource before control continues outside.

Deeply nested control transfers are often clearer when extracted into a function with an explicit return type.

---

**Previous:** [← `loop`](./08-loop.md) · **Next:** [Pattern Matching →](../14-pattern-matching/README.md)
