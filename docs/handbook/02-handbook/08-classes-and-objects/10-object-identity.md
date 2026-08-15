# Object Identity

Normal class instances have observable identity. `a is b` asks whether two references denote the same instance; `a == b` follows structural equality.

Identity matters for mutable entities, caches, graph nodes, and lifecycle. Do not use it for value concepts better represented as records or value classes.

Compiler optimization must preserve every observable identity comparison.

---

**Previous:** [← Static Members](./09-static-members.md) · **Next:** [Cloning →](./11-cloning.md)
