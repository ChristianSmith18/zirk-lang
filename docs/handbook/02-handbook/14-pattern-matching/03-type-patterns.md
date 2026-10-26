# Type Patterns

Type patterns select and narrow a value according to its runtime-observable type contract.

```zirk
class Animal { }
class Dog extends Animal { }

fn describe(value: Animal): String {
    if value is Dog { return "dog"; }
    return "animal";
}
```

Inside a successful branch, the tested value has the narrowed type. The
compiler rejects impossible tests and preserves declared types after the
branch. A type test requires a runtime-observable nominal type such as a class,
contract, or enum; scalar types such as `String` do not have runtime type tags.

Use algebraic enums instead when the set of alternatives is closed and owned by the model; they provide stronger exhaustiveness.

---

**Previous:** [← Multiple Patterns](02-multiple-patterns.md) · **Next:** [ Enum Patterns](04-enum-patterns.md)
