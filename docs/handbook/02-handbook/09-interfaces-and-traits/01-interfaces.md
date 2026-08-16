# Interfaces

An interface names a behavior contract independent of one implementation class.

```zirk
interface Serializable {
    fn serialize(): String;
}
```

Generic constraints and public APIs can depend on the interface instead of a concrete type. An implementation must provide compatible visibility, parameter, result, effect, and error behavior.

Interfaces should remain cohesive; unrelated methods force implementers to claim capabilities they do not possess.

---

**Previous:** [← Interfaces and Traits](README.md) · **Next:** [ Implementing Contracts](02-implementing-contracts.md)
