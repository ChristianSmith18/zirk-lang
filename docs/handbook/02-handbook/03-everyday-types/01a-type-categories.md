# Type Categories

Zirk classifies types by who defines their semantics and whether assignment
copies a value or shares a reference. These categories help predict mutation,
identity, conversion, and which operations the compiler knows intrinsically.

## Compiler primitives

Primitives have literals or compiler-recognized operators, may be represented
inline, and cannot be reopened by application code:

- signed and unsigned integers;
- `Float16`, `Float32`, `Float64`, and `Float128`;
- `Boolean`;
- `Char`;
- exact `Duration`.

A primitive still has properties, methods, and contracts. Calling
`value.abs()` does not imply observable boxing.

## Native value types

The language or standard library defines these immutable semantic values, but
they are not all lexical primitives: `Date`, `Time`, `DateTime`, `Instant`,
`ZonedDateTime`, `TimeZone`, `Period`, and supporting temporal enums. Their
transformations return new values.

## Native reference types

`String`, `Array<T>`, `List<T>`, `Map<K,V>`, and `Set<T>` are managed references.
Assignment normally aliases the same instance. Binding qualifiers decide
whether a name can be rebound or the referent mutated.

## User-defined types

- A `class` is nominal, referenced, stateful, and has observable identity.
- A `record` is nominal, immutable, and structurally equal by its fields.
- A value class is nominal, has no observable identity, and may be inline.
- An enum is a closed nominal set, optionally with associated values.
- An alias names an existing type; a union lists explicit alternatives.

## Special types

`Null`, `Void`, and `Never` describe absence, normal no-value completion, and
non-returning control flow. They are not ordinary data containers.

## Choosing a category

Use the narrowest type that expresses the domain. A user ID should be a value
class rather than a loose integer; a birthday is a `Date`, not an `Instant`; a
timeout is a `Duration`, not an integer count of milliseconds; growing text is
a `String`, while binary protocol data is bytes.

---

**Previous:** [← Object and the Type Tree](01-object-and-type-hierarchy.md) · **Next:** [ Value and Reference Behavior](01b-value-and-reference-behavior.md)
