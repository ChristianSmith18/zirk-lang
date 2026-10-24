# Class, Contract, or Object?

Choose the concept that matches the semantics:

- A **class** models identity, encapsulated state, lifecycle, inheritance, or
  polymorphic behavior.
- A **contract** names required properties and operations for values that may
  have different concrete types.
- An **Object** holds open, dynamically extensible data, including parsed JSON.
- A **tuple** models a small positional value group.
- An **enum** models a closed set of alternatives.

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

| Question | Class | Contract | Object |
| --- | --- | --- | --- |
| Defines nominal identity? | Yes | No | Yes |
| Defines required members? | Concrete members | Yes | No |
| Allows open dynamic keys? | Through its object view | No | Yes |
| Assignment aliases identity? | Yes | Same underlying value | Yes |
| Main use | Stateful entity or service | Shared structural API | JSON and flexible data |

A contract does not create an immutable projection or copy. Successful
conformance validates the shape and seals the object's existing key set across
aliases. Extra keys remain stored but are hidden from the contract-typed view.

---

**Previous:** [← Union Types](07-union-types.md) · **Next:** [Collections](../12-collections/README.md)
