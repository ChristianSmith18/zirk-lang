# Data modeling and generics

Choose a data form from semantics rather than from a familiar language's
syntax.

| Need | Form | Key property |
| --- | --- | --- |
| Stateful identity | `class` | Reference-backed object with constructors and methods. |
| Immutable named data | `record` | Structural equality and value behavior. |
| Finite named alternatives | `enum` | Domain behavior belongs in external `match` functions. |
| Heterogeneous product | tuple / `Tuple(A, B)` | Fixed components. |
| Alternate compatible types | `A | B` | Narrow before non-common operations. |
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

record Point {
    x: Int32;
    y: Int32;
}
```

Use a `class` when identity, shared state, or mutation through a reference is
meaningful. Use a `record` when the value is immutable data with structural
equality. Use a traditional `enum` for named cases and an algebraic enum when
cases carry typed payloads; behavior belongs in an external exhaustive
`match`, not in enum methods. Use `type Alias = ...` for an alias, and `A | B`
for a normalized union that must be narrowed before non-common operations.

Interfaces contain requirements, traits can provide reusable bodies, and
abstract classes describe nominal requirements without ordinary instance
state. Read the contracts handbook before implementing `implements` or
`#override`.

Generics use `<T>` and constraints use `from A & B`:

```zirk
fn copy_all<T from Clone & Serializable>(values: Iterable<T>): List<T> { ... }
```

Do not invent associated types, higher-kinded types, anonymous classes, or
user-defined enum methods. Check current feature status before presenting an
advanced data-model example as runnable.
