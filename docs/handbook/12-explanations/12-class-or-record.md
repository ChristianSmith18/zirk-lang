# Class, Contract, or Object?

Use a class when identity, lifecycle, inheritance, or encapsulated behavior
matters. Use a contract to state which properties and operations a value must
provide. Use `Object` for open data whose keys may vary, including JSON-shaped
data.

```zirk
contract AddressLike {
    street: String;
    city: String;
}

mut address: Object = { street: "Main", city: "Santiago" };
inmut view: AddressLike = address;

class Session {
    token: String;
    active: Boolean;
}
```

Contracts do not allocate wrappers or infer equality. A successful check
preserves object identity and seals its existing keys across aliases. Tuples
remain useful for compact positional values; enums model closed alternatives.

---

**Previous:** [← Why Portable IR?](11-why-portable-ir.md) · **Next:** [Appendices →](../13-appendices/README.md)
