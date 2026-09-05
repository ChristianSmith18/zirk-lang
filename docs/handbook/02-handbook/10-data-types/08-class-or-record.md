# Class or Record?

Choose based on semantics:

- A **class** models identity, encapsulated state, lifecycle, inheritance, or polymorphic behavior.
- A **record** models a product of named fields with structural value behavior.

For example, a mutable session with lifecycle is a class; an immutable address payload is a record.

Representation cost is secondary. The compiler may optimize both, but callers depend on equality, identity, mutation, construction, and compatibility contracts.

```zirk
class Session {
    token: String;
    active: Boolean;
}

record Address {
    street: String;
    city: String;
}
```

| Question | Class | Record |
| --- | --- | --- |
| Observable identity with `is`? | Yes | No |
| Mutable state? | Yes, by field contract | No |
| Equality | Usually field/contract-defined | Structural by fields |
| Assignment | Shares the class reference | Copies value semantics |
| Primary use | Stateful entity or service | Immutable data product |
| Inheritance | One class plus contracts | No class identity hierarchy |

Use `Session` when two otherwise equal sessions may still be different live
objects. Use `Address` when only the field values matter.

---

**Previous:** [← Union Types](07-union-types.md) · **Next:** [ Collections](../12-collections/README.md)
