# Value and Reference Semantics

Every Zirk value belongs semantically to a class, but that does not require uniform heap allocation. The compiler may represent simple values inline and choose stack, heap, move, or reference-counting strategies internally.

Language semantics describe observable behavior: whether assignment copies an independent value, shares identity, or follows a type-defined contract. Representation is an optimization detail unless an ABI boundary makes layout explicit.

Use `==` for structural equality and `is` only for types with observable reference identity:

```zirk
if first == second { /* equal contents */ }
if first is second { /* same observable instance */ }
```

`inmut` prevents rebinding but does not change the underlying type's value/reference behavior. `inmut::strict` adds a transitive mutation guarantee. Zirk 1.x does not expose ownership or reference counting as public syntax.

---

**Previous:** [← Shadowing](./08-shadowing.md) · **Next:** [Everyday Types →](../03-everyday-types/README.md)
