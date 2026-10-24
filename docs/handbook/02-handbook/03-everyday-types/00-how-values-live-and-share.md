# How Zirk Values Live and Share

Every Zirk value has a type and observable memory behavior: it either copies as
a value or shares a reference. The compiler may choose a different physical
layout when that does not change program behavior.

## The main positions

| Position | Copy/share rule | Representative types |
|---|---|---|
| Primitive value | Assignment copies the value | `Int32`, `Boolean`, `Char`, `Decimal` |
| Aggregate value | Assignment copies the aggregate | `Date`, `Duration`, tuples, enums |
| Managed reference | Assignment aliases the same instance | `String`, collections, classes, `Object` |
| Borrowed view | A short-lived, non-owning view | `Weak<T>`, `Dependent<T>`, `NativeSlice<T>` |
| Unsafe pointer | Raw memory without a lifetime guarantee | `Pointer<T>` |

`Dynamic` is a tagged value that preserves the runtime type of its contents; it
is not a memory category or a universal root type.

## Primitive and aggregate values

```zirk
mut first: Int32 = 3;
mut second = first;
second += 1; // first is still 3

inmut point: Tuple(Decimal, Decimal) = (1.0, 2.0);
inmut copy = point; // independent value
```

Scalars, tuples, enums, and temporal values do not share identity. Their
component-wise equality and cloning depend on the capabilities of their
components.

## Managed references

```zirk
mut text = "hello";
mut alias = text;
alias[0] = 'H'; // both names see "Hello"

mut child: Object = { value: 1 };
mut parent: Object = { child: child };
mut copied: Object = { ...parent };
inmut original_child: Object = parent.child as Object;
inmut copied_child: Object = copied.child as Object;
original_child is copied_child; // true: object spread is shallow
```

`String`, collections, classes, and `Object` are managed references. Dynamic
object property reads preserve the stored runtime tag and nested reference
identity. Object spread creates a new top-level object and preserves nested
references. `inmut` prevents rebinding while allowing referent mutation;
`inmut::strict` freezes the reachable graph. Use explicit `clone()` when an
independent graph is required and supported.

A `contract` describes required properties and operations without creating a
new object. Successful conformance seals an object's existing key set across
aliases while preserving its identity.

## Borrowed and unsafe views

`Weak<T>` does not keep its referent alive. `Dependent<T>` and `NativeSlice<T>`
are views whose validity is tied to a larger owner. They cannot outlive the
value they observe.

`Pointer<T>` is a raw address. It may be null or dangling and carries no runtime
ownership; use it only within the documented `unsafe` boundaries or when
interoperating with native code.

---

**Previous:** [← Everyday Types](README.md) · **Next:** [Object and Zirk's Type Families](01-object-and-type-hierarchy.md)
