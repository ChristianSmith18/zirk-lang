# Default Implementations

A trait may supply behavior implementers inherit when they do not override it. The implementation can use only requirements made available by the trait contract.

Defaults reduce repetition while preserving a single semantic expectation. An override must remain substitutable and cannot weaken safety, permissions, or error guarantees promised by the trait.

Prefer a helper function when reuse does not need polymorphic dispatch or conformance.

```zirk
trait Describable {
    type_name(): String;

    describe(): String {
        return "value of {this.type_name()}";
    }
}

class User implements Describable {
    type_name(): String {
        return "User";
    }
    // Inherits `describe`.
}

class VerboseUser implements Describable {
    type_name(): String {
        return "VerboseUser";
    }

    #override
    describe(): String {
        return "a user with verbose diagnostics";
    }
}
```

`User` receives the default; `VerboseUser` replaces it. Replacing a trait
default is an override of an inherited implementation, so it requires the
`#override` marker; satisfying a signature-only requirement such as
`type_name()` takes no marker. If two adopted traits provide the same method,
the class must define its own implementation under `#override` to resolve
the conflict explicitly.

---

**Previous:** [← Traits](03-traits.md) · **Next:** [ Interface, Trait, or Class?](05-interface-vs-trait-vs-class.md)
