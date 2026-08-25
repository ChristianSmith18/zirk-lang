# Cloning

`clone()` exists only through an explicit trait. It may be derived when every field is cloneable.

Cloning must state whether nested identity-bearing values are shared or cloned according to their contracts. It is not an automatic substitute for resource transfer, and external handles may be impossible or unsafe to duplicate.

Prefer immutable sharing when no independent mutation is needed. Clone only when the domain requires a separate value or identity.

Assignment of a class instance shares the same managed reference:

```zirk
mut first = User(name: "Ada");
mut second = first;

second.name = "Grace";
stdout.println(first.name); // "Grace"
first is second;            // true
```

An explicit clone produces an independent logical object when the class
implements `Clone` (derived automatically when every field is itself
`Clone`, or written explicitly as `implements Clone`):

```zirk
mut independent = first.clone();
independent.name = "Linus";

first is independent; // false
```

Destructuring copies the extracted values according to their own semantics. It
does not silently deep-clone nested class references. Reconstruct a new object
from destructured values when a new identity is intended.

`clone()` is the explicit escape from shared-reference behavior. Its contract must specify depth: a deep clone recursively produces independent mutable referents, while an explicitly documented shallow clone may retain nested aliases. `inmut::strict` does not itself clone; clone first, then choose the binding permission for the independent result.

---

**Previous:** [← Object Identity](10-object-identity.md) · **Next:** [ Interfaces and Traits](../09-interfaces-and-traits/README.md)
