# Generic Types

Classes, records, enums, and other supported declarations can parameterize stored or associated values.

```zirk
record Pair<A, B> {
    first: A;
    second: B;
}
```

`Pair<String, Int32>` and `Pair<String, UInt64>` are distinct static instantiations. Members preserve their parameterized types without runtime guessing.

A trailing type parameter may declare a default with `=`, used when the
argument is omitted or written as an empty argument list:

```zirk
class Box<T = Int32> {
    value: T;
    construct(value: T) { this.value = value; }
    get(): T { return this.value; }
}

mut b = Box(7);            // T defaults to Int32
mut s: Box<String> = Box("ok");
mut d: Box<> = Box(5);     // explicit request for the default
```

The default is still checked against any `from` constraint on the parameter.

A generic type satisfies a generic contract end-to-end —
`class Box<T> implements Container<T>` and `record Pair<T> implements
Container<T>` both dispatch through `Container<...>`-typed references.
Remaining limits: a trait's default method is not yet substituted under a
generic contract, a nested instantiation in the contract argument
(`implements Container<Box<T>>`) is rejected with a diagnostic, and defaults
on the type parameters of a user-defined generic *function* are specified but
not yet implemented.

Keep a type parameter only when it represents a relationship consumers need. An unused or purely internal parameter complicates APIs and package compatibility.

---

**Previous:** [← Generic Functions](01-generic-functions.md) · **Next:** [ Constraints with from](03-constraints-with-from.md)
