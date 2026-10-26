# Generic Types

Classes, contracts, enums, and other supported declarations can parameterize stored or associated values.

```zirk
contract Pair<A, B> {
    first: A;
    second: B;
}

inmut pair: Pair<String, Int32> = { first: "Ada", second: 37 };
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

inmut b = Box(7); // T defaults to Int32
inmut s: Box<String> = Box("ok");
inmut d: Box<> = Box(5); // explicit request for the default
```

The default is still checked against any `from` constraint on the parameter.

A generic type satisfies a generic contract end-to-end —
`class Box<T> implements Container<T>` dispatches through
`Container<...>`-typed references. Generic object contracts validate their
substituted property types.
Remaining limits are documented in the [feature status](../../11-reference/12-feature-status.md).

Keep a type parameter only when it represents a relationship consumers need. An unused or purely internal parameter complicates APIs and package compatibility.

---

**Previous:** [← Generic Functions](01-generic-functions.md) · **Next:** [ Constraints with from](03-constraints-with-from.md)
