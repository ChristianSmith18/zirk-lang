# Implementing Contracts

`implements` declares that a class or contract includes the requirements of
another contract. An ordinary `Object` may also be checked against a contract.

```zirk
contract Named {
    name: String;
}

class User implements Named {
    name: String;
    construct(name: String) { this.name = name; }
}

inmut user: Named = User("Ada");
```

For a statically known class or literal, the compiler checks conformance at
compile time. When only a dynamic `Object` is available, conformance is checked
at runtime and failure is catchable. A successful check seals the object's
existing key set across aliases. Extra keys remain stored and accessible through
an `Object` view; a contract-typed view exposes only its declared members.
Existing property values can still change when their mutability rules allow it.

Implementation does not automatically export a private class; file publication
still uses `share`.

## Contract composition

A contract can extend the requirements of another contract:

```zirk
contract Identified {
    id: UInt64;
}

contract NamedEntity implements Identified {
    name: String;
}

class User implements NamedEntity {
    id: UInt64;
    name: String;
    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }
}
```

Conformance is transitive, including generic type substitution. Cycles and
incompatible inherited signatures are rejected at the contract declaration.
Traits remain a separate composition mechanism for reusable implementations.

---

**Previous:** [← Contracts](01-interfaces.md) · **Next:** [Traits](03-traits.md)
