# Data modeling and generics

Choose a data form from semantics rather than from a familiar language's
syntax.

| Need | Form | Key property |
| --- | --- | --- |
| Stateful identity | `class` | Reference-backed object with constructors and methods. |
| Structural API requirements | `contract` | Reusable property and operation requirements without storage. |
| Finite named alternatives | `enum` | Domain behavior belongs in external `match` functions. |
| Heterogeneous product | tuple / `Tuple(A, B)` | Fixed components. |
| Alternate compatible types | `A | B` | Narrow before non-common operations; literal alternatives can preserve exact discriminants. |
| Combined compatible requirements | `A & B` | One value must satisfy every constituent; use class/contract references for runtime views. |
| Reusable abstraction | `interface`, `trait`, `abstract class` | Read contract rules before implementation. |

Classes use `construct`, not `new`; fields use `name: Type;`; type-body methods
omit `fn`. A concrete class extends at most one concrete class. Use `#override`
only when replacing a concrete inherited implementation.

```zirk
class User {
    inmut id: Int32;
    name: String;

    construct(id: Int32, name: String) {
        this.id = id;
        this.name = name;
    }

    display_name(): String {
        return this.name;
    }
}

```

Use a `class` when identity, shared state, or mutation through a reference is
meaningful. Use a `contract` for reusable structural requirements. Use a
traditional `enum` for named cases and an algebraic enum when
cases carry typed payloads; behavior belongs in an external exhaustive
`match`, not in enum methods. Use `type Alias = ...` for a transparent alias,
`A | B` for a normalized union that must be narrowed before non-common
operations, and `A & B` for compatible requirements on the same value.
Anonymous shapes compose with those operators but are not general runtime
values in every backend path; check current feature status before promising
native execution.

Interfaces contain requirements, traits can provide reusable bodies, and
abstract classes describe nominal requirements without ordinary instance
state. Read the contracts handbook before implementing `implements` or
`#override`.

Generics use `<T>` and constraints use `from A & B`:

```zirk
fn copy_all<T from Clone & Serializable>(values: Iterable<T>): List<T> { ... }
```

Generic aliases also support erased `keyof`, indexed access, conditional types
with `infer`, mapped types, finite template types, and standard utility types;
read the [operator reference](../../../../docs/handbook/02-handbook/11-generics/09-type-operators.md)
before authoring them. Do not invent associated types, higher-kinded types,
anonymous classes, or user-defined enum methods. Check current feature status
before presenting an advanced data-model example as runnable.
