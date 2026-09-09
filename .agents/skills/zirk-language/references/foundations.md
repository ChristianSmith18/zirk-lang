# Foundations: bindings, values, and scalar types

Read this for ordinary variables, literals, type annotations, nullability, and
arithmetic. For classes or collections, continue to their category reference.

## Bindings and mutation

```zirk
mut count: Int32 = 0;
inmut label: String = "Zirk";
inmut::strict config: Config = Config();
```

| Need | Form | Meaning |
| --- | --- | --- |
| Rebind a name | `mut` | The binding can receive another value. |
| Keep the binding, permit ordinary reference mutation | `inmut` | The name cannot be rebound. |
| Freeze the reachable graph | `inmut::strict` | Rejects mutation through it and mutable aliases. |

Use `T?` for a nullable value, `?.` for safe member access, and `??` for a
fallback. There is no `undefined` or truthiness: conditions are `Boolean`.

## Choose a scalar deliberately

- Use `Int32` by default (`Int` and `Integer` alias it); choose a signed or
  unsigned width only when its range matters.
- Use `Boolean`, `Char`, and `String` for their distinct domains; a `Char` is
  one Unicode grapheme.
- Use `Duration` for elapsed amounts (`5s`, `250ms`) and read the temporal
  handbook before choosing `Date`, `Time`, `DateTime`, or `Instant`.
- Before choosing `Decimal` or `Float`, check the current authoritative type
  section and feature status. Never rely on an old spelling or assume an
  implicit decimal/binary-float conversion.

## Expressions and conversions

Use `==`/`!=` for structural equality and `is` for observable identity. Use
`&&`, `||`, and `!` only with Booleans. Explicit conversions use `value as T`
or `<T>value`; memory reinterpretation requires `unsafe`.

Integer overflow, zero division, invalid bounds, and invalid conversions are
controlled failures. Do not encode unchecked behavior in ordinary code.
