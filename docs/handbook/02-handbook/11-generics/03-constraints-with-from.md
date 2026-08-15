# Constraints with `from`

Generic constraints use the language's `from` relationship to restrict acceptable types and expose guaranteed operations to the body.

```zirk
fn render<T from Display>(value: T): String {
    return value.display();
}
```

A call with a type that does not satisfy `Display` fails at the call site with the missing contract. The implementation cannot call operations outside the declared constraint merely because current callers happen to provide them.

> **Specification status:** The historical inventory names `from` constraints. Exact complete grammar must remain aligned with the final generic grammar as it is formalized.

---

**Previous:** [← Generic Types](./02-generic-types.md) · **Next:** [Multiple Type Parameters →](./04-multiple-type-parameters.md)
