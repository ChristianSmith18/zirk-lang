# Objects and Contracts

Zirk uses `Object` for open, identity-bearing data and `contract` for a named
structural requirement. These roles replace the former `record` and
`interface` split.

```zirk
contract PointLike {
    x: Decimal;
    y: Decimal;
}

mut point: Object = { x: 10.0, y: 20.0 };
point.x = 11.0;

inmut view: PointLike = point;
```

A bare identifier key is its literal string name. Quoted keys allow arbitrary
strings, and quoted interpolation computes a string key using ordinary Zirk
interpolation:

```zirk
inmut key = "custom_status";
inmut response: Object = {
    status: "ok",
    "{key}": null
};
response["count"] = 3;
```

Unknown reads through `Object` have type `Dynamic` and require checked narrowing
before concrete use. Missing direct reads raise a catchable missing-property
error; use the presence-safe lookup operation when absence is expected.
Object assignment and contract views preserve identity. A successful contract
check validates required members and seals the complete existing key set across
aliases. Extra properties remain on the same object, but contract views expose
only the declared members. Mutability rules still govern changes to values.

Use a class when construction invariants, nominal identity, inheritance, or
encapsulated behavior matter. Use tuples for compact positional value groups
and enums for closed alternatives. Dynamic objects do not gain automatic deep
copy or structural equality; use explicit clone/equality behavior when needed.

JSON objects parse to ordinary `Object` values and arrays to `List<Dynamic>`.
Use `JSON.stringify` to serialize JSON-compatible values, or implement the
`JSONCodec<T>` contract for explicit typed conversion. See [`std.json`](../../04-standard-library/13-std-json.md)
and the [migration guide](../../../MIGRATING_FROM_RECORD_INTERFACE.md).

---

**Previous:** [← Tuples](00-tuples.md) · **Next:** [Traditional Enums →](03-traditional-enums.md)
