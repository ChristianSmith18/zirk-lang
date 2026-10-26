# Generic Type Operators

Type aliases can describe reusable transformations over structural types and
finite unions. The operators are evaluated during semantic analysis and have
no runtime value.

## Structural queries

`keyof T` produces the finite union of visible property names. Indexed access
`T[K]` produces the property type for each valid key in a finite key union:

```zirk
type Profile = { name: String; enabled: Boolean };
type ProfileKey = keyof Profile;
type ProfileValue = Profile[ProfileKey];
```

Every indexed key must exist on every possible receiver alternative. Optional
properties contribute their nullable value type. Private members are excluded
under ordinary visibility rules.

## Conditional types and `infer`

Use `T extends Constraint ? Yes : No` to select a type by assignability. A
conditional on a naked type parameter distributes across union members;
parentheses suppress that distribution. `infer` binds a matched component only
inside the true branch:

```zirk
type Element<T> = T extends Array<infer Item> ? Item : Boolean;
type Found = Element<Array<String >>;
```

`Found` is `String`; an `infer` name cannot escape its true branch. Distribution
and inference are deterministic and subject to the type-computation budget.

## Mapped types

Mapped types transform finite structural keys. `?` and `!` add or remove
optionality; `inmut` and `mut` control mutability; `as` remaps keys:

```zirk
type Optional<T> = { [K in keyof T]?: T[K] };
type Frozen<T> = { inmut [K in keyof T]: T[K] };
type RequiredMutable<T> = { mut [K in keyof T] !: T[K] };
type Renamed<T> = { [K in keyof T as K]: T[K] };
```

Remapped keys must remain unique. A duplicate key is a type error rather than
an order-dependent overwrite.

## Finite template types

Interpolated type strings expand finite singleton alternatives into a
normalized union of string types:

```zirk
type Color = "red" | "blue";
type ColorName = "color-{Color}";
```

String, numeric, Boolean, and other supported string-convertible singleton
unions can be interpolated. Expansion has a fixed budget; exceeding it reports
a diagnostic instead of truncating the result.

## Standard utilities

The standard type prelude includes `Partial<T>`, `Required<T>`, `Readonly<T>`,
`Pick<T, K>`, `Omit<T, K>`, `Record<K, V>`, `Extract<T, U>`,
`NonNullable<T>`, `Parameters<F>`, and `ReturnType<F>`. These preserve ordinary
constraints, visibility, and property modifiers. Type operators cannot expose
members that are private in the current context.

The semantic regression suite covers these examples and edge cases with
`cargo test -p zirk-sema --test typing`.

---

**Previous:** [← Variance, Recursion, and Runtime Identity](08-variance-recursion-and-runtime.md) · **Next:** [Data Types →](../10-data-types/README.md)
