# Constructors

The constructor is named `construct` and establishes a valid instance.

```zirk
construct(id: UInt64, name: String) {
    this.id = id;
    this.name = name;
}
```

Every required field must be available before construction completes. Validation that can fail should use an explicit factory returning `Result` when constructor failure cannot be expressed safely.

Zirk has no `new` keyword and does not use destructors as its general resource-cleanup model.

A class may declare multiple `construct` members when their parameter lists are
not identical. Resolution considers arity, parameter types, and named
arguments:

```zirk
class User {
    id: UInt64;
    name: String;

    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }

    construct(name: String) {
        this.id = generate_user_id();
        this.name = name;
    }
}
```

Two constructors with the same effective parameter signature are invalid.
Constructor selection must remain unambiguous after optional and named
arguments are considered. This constructor-specific facility does not enable
general function overloading.

---

**Previous:** [← Fields and Properties](./02-fields-and-properties.md) · **Next:** [Instantiation →](./04-instantiation.md)
