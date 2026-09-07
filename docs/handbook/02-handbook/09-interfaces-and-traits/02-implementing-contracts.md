# Implementing Contracts

`implements` declares that a class satisfies an interface or trait.

```zirk
class User implements Serializable {
    serialize(): String { /* ... */ }
}
```

The compiler checks the complete contract. A similar method name with an incompatible return type is not an implementation. Missing requirements should be diagnosed at the declaration with links to the originating contract.

Implementation does not automatically export a private class; file publication still uses `share`.

## Contract composition

A contract can itself declare `implements`, so one interface or trait extends
the requirements of another:

```zirk
interface A {
    a(): Int32;
}

interface B implements A {
    b(): Int32;
}

class C implements B {
    construct() { }
    a(): Int32 { return 1; }
    b(): Int32 { return 2; }
}
```

Conformance is transitive: `C implements B` must also satisfy every
requirement `B` inherits from `A`. Composition is generic-aware, so
`interface B<T> implements A<T>` carries the type parameter through. Cycles
(`A implements B` while `B implements A`) and incompatible inherited
signatures are rejected with a diagnostic at the contract declaration.

---

**Previous:** [← Interfaces](01-interfaces.md) · **Next:** [ Traits](03-traits.md)
