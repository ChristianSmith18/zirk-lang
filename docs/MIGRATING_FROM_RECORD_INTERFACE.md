# Migrating from `record` and `interface`

Zirk replaces the two overlapping shape constructs with one structural
`contract` and identity-bearing `Object` values. The migration is intentional:
records no longer copy by value or receive structural equality, and interfaces
are no longer a separate declaration kind.

## Choose the replacement by intent

| Previous intent | Replacement |
| --- | --- |
| A named value aggregate with a small fixed number of components | Tuple, or an enum when the alternatives are closed |
| Mutable named data with open keys, including JSON objects | `Object` literal; use `Dynamic` when a read has no known concrete type |
| A fixed structural API shared by classes or ordinary objects | `contract` with required properties and operation signatures |
| Nominal identity, methods, construction invariants, or inheritance | `class` |
| Reusable method bodies or conflict-selectable defaults | `trait` |
| A generic function that accepts any type but preserves its static type | Generic type parameter |
| A heterogeneous runtime value whose concrete type is checked later | `Dynamic` |

## Interface to contract

Replace the keyword and add property requirements where the shape includes
state:

```zirk
contract Named {
    name: String;
    label(): String;
}
```

Contracts describe shape only. Move reusable method bodies to a `trait` or a
class. Class conformance is checked against required property types, property
mutability, and operation signatures. An ordinary `Object` can be checked by
shape; successful conformance seals its existing key set across aliases while
preserving extra keys and identity.

## Record to Object, class, tuple, or enum

Use an object for open mutable data:

```zirk
inmut user: Object = {
    name: "Ada",
    "{localized_key}": "value"
};
```

The reference binding may be `inmut` while its properties remain mutable;
`inmut::strict` freezes the reachable graph. Bare identifier keys are literal
strings. Quoted interpolated keys use ordinary string interpolation, and
computed access uses `object[key]`.

Use a class instead when the old record had important identity, invariants,
construction behavior, or methods. Use a tuple for compact positional values,
and an enum for a closed set of alternatives. Do not replace record equality
with implicit object equality: objects have identity unless an explicit
capability defines otherwise.

## Former universal `Object` parameters

`Object` now accepts reference objects only. If a function should preserve the
input's type, make it generic. If it intentionally stores or reads values with
different runtime types, use `Dynamic` and narrow before concrete use.

## JSON

`JSON.parse` returns `Dynamic`: JSON objects are ordinary `Object` values,
arrays are `List<Dynamic>`, and scalar nodes remain ordinary Zirk values.
`JSON.stringify` serializes those values. Use a contract-based codec when a
fixed object shape is required; there is no public `JSONValue` tree.

## Compiler migration diagnostics

The parser reports an error for both `interface` and `record`. For
`interface`, it suggests replacing the keyword with `contract`; for `record`,
it suggests a `contract` plus an `Object` literal for structural data, or a
class, tuple, or enum when identity or value semantics are intended. The
parser may retain a recovery node so it can continue reporting diagnostics,
but source containing either removed declaration is rejected and cannot be
compiled or run.
