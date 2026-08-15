# Class, Record, or Value Class?

Choose based on semantics:

- A **class** models identity, encapsulated state, lifecycle, inheritance, or polymorphic behavior.
- A **record** models a product of named fields with structural value behavior.
- A **value class** creates a distinct domain value around a compact representation.

For example, a mutable session with lifecycle is a class; an immutable address payload is a record; a validated `OrderId` is a value class.

Representation cost is secondary. The compiler may optimize all three, but callers depend on equality, identity, mutation, construction, and compatibility contracts.

```zirk
class Session {
    token: String;
    active: Boolean;
}

record Address {
    street: String;
    city: String;
}

value class OrderId(value: UInt64);
```

| Question | Class | Record | Value class |
| --- | --- | --- | --- |
| Observable identity with `is`? | Yes | No | No |
| Mutable state? | Yes, by field contract | No | No value-level mutation |
| Equality | Usually field/contract-defined | Structural by fields | By wrapped value/contract |
| Assignment | Shares the class reference | Copies value semantics | Copies value semantics |
| Primary use | Stateful entity or service | Immutable data product | Domain-safe scalar/value |
| Inheritance | One class plus contracts | No class identity hierarchy | No identity hierarchy |

Use `Session` when two otherwise equal sessions may still be different live
objects. Use `Address` when only the field values matter. Use `OrderId` when a
raw integer needs a distinct domain meaning without object identity.

---

**Previous:** [← Union Types](./07-union-types.md) · **Next:** [Collections →](../12-collections/README.md)
