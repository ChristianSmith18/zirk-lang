# Contracts

A `contract` describes the properties and operations a value must provide.
It defines no storage and no reusable method bodies.

```zirk
contract Serializable {
    serialize(): String;
}

contract Named {
    name: String;
}
```

A contract can combine properties and operations. Classes and ordinary objects
may satisfy it structurally. Property types, mutability, operation parameters,
results, and effects must be compatible. Generic constraints can name contracts
so an API depends on the capability it needs rather than on one concrete class.

Use a `trait` when several types should share reusable method bodies. Traits do
not declare data properties; `contract` owns the structural shape requirement.

---

**Previous:** [← Contracts and Traits](README.md) · **Next:** [Implementing Contracts](02-implementing-contracts.md)
