# Value and Reference Semantics

Every Zirk value belongs semantically to a class, but that does not require uniform heap allocation. The compiler may represent simple values inline and choose stack, heap, move, or reference-counting strategies internally.

Language semantics describe observable behavior: whether assignment copies an independent value, shares identity, or follows a type-defined contract. Representation is an optimization detail unless an ABI boundary makes layout explicit.

Use `==` for structural equality and `is` only for types with observable reference identity:

```zirk
if first == second { /* equal contents */ }
if first is second { /* same observable instance */ }
```

`inmut` prevents rebinding but does not change the underlying type's value/reference behavior. `inmut::strict` adds a transitive mutation guarantee. Zirk 1.x does not expose ownership or reference counting as public syntax.

String is the simplest visible reference example: assigning it shares one
mutable instance, while `clone()` creates independent content. Temporal values
and records are value-semantic and expose no `is` identity.

## Whole references, projections, and places

Whole-reference assignment, argument passing, returning, and closure capture
share the referent. Reading inside a reference—an attribute, index, slice,
destructured component, pattern binding, projected argument, projected return,
or projected capture—creates an independent logical value. A reference-backed
projection is deeply cloned and must satisfy `Clone`.

```zirk
mut users = [User(name: "Ada")]
mut alias = users              // shares the whole list
mut first = users[0]           // independent projected User
users[0].name = "Grace"        // place write changes users and alias
stdout.println(first.name)     // Ada
```

The same expression can be a read or a place. On the left of assignment,
`users[0].name` retains its path and mutates original storage. When read, it is
a projection. The rule is transitive and generic extraction APIs must declare
`Clone` whenever they return an independent reference-backed result. An
explicit documented view is the only way to share a subregion.

---

**Previous:** [← Shadowing](08-shadowing.md) · **Next:** [ Everyday Types](../03-everyday-types/README.md)
