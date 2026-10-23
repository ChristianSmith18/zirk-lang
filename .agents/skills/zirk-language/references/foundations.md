# Foundations: bindings, values, and scalar types

Read this for ordinary variables, literals, type annotations, nullability, and
arithmetic. For classes or collections, continue to their category reference.

## Bindings and mutation

```zirk
mut count: Int32 = 0;
inmut label: String = "Zirk";
inmut::strict config: Config = Config();
count += 1;
```

| Need | Form | Meaning |
| --- | --- | --- |
| Rebind a name | `mut` | The binding can receive another value. |
| Keep the binding, permit ordinary reference mutation | `inmut` | The name cannot be rebound. |
| Freeze the reachable graph | `inmut::strict` | Rejects mutation through it and mutable aliases. |

For initialized local bindings, omit the type when the initializer makes the
intended type clear:

```zirk
inmut greeting = "Hello";
mut retries = 0;
retries += 1;
```

Annotate when the type is part of a public signature, the initializer is
ambiguous or empty, a specific scalar width is intentional, or an explicit
`Result<T, E>` binding is needed to inspect its error later. Do not write both
an inferred type and an equivalent cast just to repeat the initializer's
type.

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
`&&`, `||`, and `!` only with Booleans. `as` and `<T>value` are checked casts
between related types, not general value conversions or type assertions. Use a
documented `T(value)` conversion or typed conversion method when converting the
value itself; check the owning type/API chapter because conversions may be
fallible. Memory reinterpretation requires `unsafe`.

Integer overflow, zero division, invalid bounds, and invalid conversions are
controlled failures. Do not encode unchecked behavior in ordinary code.
