# Implementing Contracts

`implements` declares that a class satisfies an interface or trait.

```zirk
class User implements Serializable {
    fn serialize(): String { /* ... */ }
}
```

The compiler checks the complete contract. A similar method name with an incompatible return type is not an implementation. Missing requirements should be diagnosed at the declaration with links to the originating contract.

Implementation does not automatically export a private class; file publication still uses `share`.

---

**Previous:** [← Interfaces](./01-interfaces.md) · **Next:** [Traits →](./03-traits.md)
