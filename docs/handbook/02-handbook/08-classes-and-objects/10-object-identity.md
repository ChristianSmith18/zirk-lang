# Object Identity

Normal class instances have observable identity. `a is b` asks whether two references denote the same instance; `a == b` follows the type's equality contract.

Identity matters for mutable entities, caches, graph nodes, and lifecycle. Do not use it for value concepts better represented as records.

Compiler optimization must preserve every observable identity comparison.

Assignment copies the managed reference, not the object. Consequently aliases observe the same field mutations. `mut` permits rebinding and referent mutation; `inmut` prevents rebinding but may still permit referent mutation; `inmut::strict` prevents both and imposes a strict-alias invariant. A mutable alias cannot be created from a strict reference.

Classes are nominal: two independently declared classes are never compatible merely because they expose the same fields. Equality is separate from identity and exists only when the class implements the equality capability. A class may define domain equality while two equal objects still have different identity.

---

**Previous:** [← Static Members](09-static-members.md) · **Next:** [ Cloning](11-cloning.md)
