# Default Implementations

A trait may supply behavior implementers inherit when they do not override it. The implementation can use only requirements made available by the trait contract.

Defaults reduce repetition while preserving a single semantic expectation. An override must remain substitutable and cannot weaken safety, permissions, or error guarantees promised by the trait.

Prefer a helper function when reuse does not need polymorphic dispatch or conformance.

```zirk
trait Describable {
    fn type_name(): String;

    fn describe(): String {
        return "value of {this.type_name()}";
    }
}

class User implements Describable {
    fn type_name(): String {
        return "User";
    }
    // Inherits `describe`.
}

class VerboseUser implements Describable {
    fn type_name(): String {
        return "VerboseUser";
    }

    fn describe(): String {
        return "a user with verbose diagnostics";
    }
}
```

`User` receives the default; `VerboseUser` replaces it. If two adopted traits
provide the same method, the class must define its own implementation to resolve
the conflict explicitly.

---

**Previous:** [← Traits](03-traits.md) · **Next:** [ Interface, Trait, or Class?](05-interface-vs-trait-vs-class.md)
