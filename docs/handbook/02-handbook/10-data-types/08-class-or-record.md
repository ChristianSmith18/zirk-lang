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

## Anonymous structural types

A type alias may also name an anonymous property shape. Required and optional
members compose with unions and intersections:

```zirk
type Contact = { email: String; phone?: String };
type IdentifiedContact = Contact & { inmut id: String };
type PreferredContact = IdentifiedContact | { active: false; reason: String };
```

These are static structural types, not open `Object` values. When a fresh
object literal is checked against a union of shapes, the compiler checks
required properties, nested values, and literal discriminants. It accepts one
viable alternative and reports an error for zero or multiple candidates.
Conflicting property types or mutability requirements make an intersection
invalid.

---

**Previous:** [← Union Types](07-union-types.md) · **Next:** [Intersection Types →](09-intersection-types.md)
