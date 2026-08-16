# Parameters and Return Types

Each parameter associates a local name with an accepted type. The return annotation constrains every reachable `return` and value-producing path.

```zirk
fn describe(id: UInt64, active: Boolean): String {
    return active ? "active:{id}" : "inactive:{id}";
}
```

Arguments do not undergo silent lossy conversion. Returning `null` from `String` is invalid; use `String?` only when absence belongs to the contract.

Parameter bindings are local to the function. Mutation of a binding, mutation of an object, and transfer of a resource are separate questions governed by the parameter and type contracts.

---

**Previous:** [← Declaring Functions](01-declaring-functions.md) · **Next:** [ Optional Parameters](03-optional-parameters.md)
