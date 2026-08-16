# Built-in Types

This catalog is the quick lookup for Zirk's compiler-known types. For semantics and examples, begin with [Everyday Types](../02-handbook/03-everyday-types/README.md).

## Type catalog

| Category | Types | Storage/behavior | Default |
|---|---|---|---|
| Signed integer | `Int8`, `Int16`, `Int32`, `Int64`, `Int128`; `Int` = `Int32` | value, fixed-width, checked arithmetic | `0` |
| Unsigned integer | `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128`; `UInt` = `UInt32` | value, fixed-width, checked arithmetic | `0` |
| Floating point | `Float16`, `Float32`, `Float64`, `Float128`; `Float` = `Float64` | IEEE-style binary value without valid `NaN` | `0.0` |
| Logic | `Boolean` | value; only `true` or `false` | `false` |
| Text unit | `Char` | one Unicode grapheme value | invalid without an explicit value |
| Text | `String` | mutable, shared reference; grapheme-indexed | `""` |
| Temporal values | `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, `Period` | immutable value types; nanosecond precision where applicable | type-specific or explicit |
| Collections | `Array<T>`, `Array<T, N>`, `List<T>`, `Map<K, V>`, `Set<T>`, `Range<T>` | native reference containers except value-like `Range` | empty where meaningful |
| Callable | `Function(P...) => R`; `Fn(P...) => R` | signature-compatible callable identity with compiler-managed environment | no implicit default |
| Product | `Tuple(T...)` | immutable heterogeneous value with constant `[]` access | component defaults where explicitly constructed |
| Special | `Null`, `Void`, `Never`, `Object` | absence, no result, no return, and semantic root | varies |

`Decimal*` is not a Zirk type family. Exact base-ten arithmetic may later be supplied as a distinct standard-library type; it must not be confused with `Float`.

## Domain types

User programs add nominal reference `class` types, immutable structural `record` values, nominal inline `value class` values, traditional and algebraic `enum` values, aliases, and unions. These are not compiler primitives, but all belong under the conceptual `Object` root.

Complete reference variables alias when moved. Reads through an attribute,
index, slice, destructuring, pattern, iterator, argument, return, or closure
capture are independent projections and require `Clone` when reference-backed.

## Universal and conditional members

Every inhabited value exposes `type` and `to_string()`. Equality, hashing, ordering, iteration, indexing, arithmetic, and cloning exist only when the type satisfies the corresponding capability contract. Native behavior is closed: programs cannot reopen a built-in type or replace its operator meaning.

## Choosing a type

- Use `Int` for ordinary whole-number counters; pick an explicit width at storage, ABI, or protocol boundaries.
- Use `UInt*` only when the domain and boundary genuinely require a non-negative binary integer.
- Use `Float` for approximate real-number computation, never for exact money.
- Use `Char` for one user-perceived Unicode grapheme and `String` for text.
- Choose temporal types by meaning, not formatting: a birthday is a `Date`, an elapsed timeout is a `Duration`, and a globally scheduled event is a `ZonedDateTime` or `Instant`.

See also [Type Member Index](./13-type-member-index.md) and [Temporal Reference](./14-temporal-reference.md).

---

**Previous:** [← Operators and Precedence](02-operators-and-precedence.md) · **Next:** [ Literals](04-literals.md)
