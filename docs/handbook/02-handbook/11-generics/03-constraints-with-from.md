# Constraints with `from`

Generic constraints use the language's `from` relationship to restrict acceptable types and expose guaranteed operations to the body.

```zirk
fn render<T from Display>(value: T): String {
    return value.display();
}
```

Join multiple constraints with `&`:

```zirk
fn stable_key<T from Hashable & Equatable & Clone>(value: T): T {
    return value;
}
```

A call with a type that does not satisfy `Display` fails at the call site with the missing contract. The implementation cannot call operations outside the declared constraint merely because current callers happen to provide them.

A constraint may be an interface, trait, abstract requirement class, concrete
base, or native contract. Failure diagnostics identify the chosen argument and
every missing requirement at the call site.

---

**Previous:** [← Generic Types](02-generic-types.md) · **Next:** [ Multiple Type Parameters](04-multiple-type-parameters.md)
