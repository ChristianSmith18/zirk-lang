# Generic Types

Classes, records, enums, and other supported declarations can parameterize stored or associated values.

```zirk
record Pair<A, B> {
    first: A;
    second: B;
}
```

`Pair<String, Int32>` and `Pair<String, UInt64>` are distinct static instantiations. Members preserve their parameterized types without runtime guessing.

Keep a type parameter only when it represents a relationship consumers need. An unused or purely internal parameter complicates APIs and package compatibility.

---

**Previous:** [← Generic Functions](./01-generic-functions.md) · **Next:** [Constraints with `from` →](./03-constraints-with-from.md)
