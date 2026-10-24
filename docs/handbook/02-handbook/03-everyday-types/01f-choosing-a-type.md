# Choosing a Type

Choose the type whose copying, sharing, and structural rules match the problem.

## Decision table

| Question | Choose | Example |
|---|---|---|
| Do you need a number, truth value, or character? | Primitive value | `Int32`, `Boolean`, `Char` |
| Is this a date, time, or duration value? | Native value | `Date`, `Duration`, `Instant` |
| Do you need shared, mutable text or a collection? | Managed reference | `String`, `Array<T>`, `List<T>` |
| Do you need named open data or a JSON-shaped value? | `Object`, optionally checked by a contract | `{ name: "Ada" }` |
| Do you need identity, inheritance, or encapsulated lifecycle? | Class | `class Session` |
| Do you need one of several named alternatives? | Enum | `enum Status` |
| Do you need a non-owning observation? | `Weak<T>` | `Weak.from(cache)` |
| Do you need a short-lived view? | `Dependent<T>` / `NativeSlice<T>` | `slice[0:10]` |
| Do you need raw memory? | `Pointer<T>` inside `unsafe` | `Pointer<Int32>` |

## Value types for domain meaning

If a value is a date, use `Date`; if it measures elapsed time, use `Duration`.
Use a tuple for a compact positional value:

```zirk
inmut origin: Tuple(Decimal, Decimal) = (0.0, 0.0);
inmut offset: Tuple(Decimal, Decimal) = (1.0, 2.0);
```

For named data, define a contract and use an ordinary object:

```zirk
contract Point {
    x: Decimal;
    y: Decimal;
}

inmut origin: Point = { x: 0.0, y: 0.0 };
```

The object is a reference with identity. A contract describes its shape; it does
not copy the object or infer structural equality.

## Managed references for shared data

Use `String` for text, `List<T>` for a growing collection, and `Array<T>` for
fixed-size storage. Assignment shares the same instance. Call `clone()` when an
independent copy is needed and supported.

Open objects are useful when keys vary at runtime:

```zirk
inmut key = "custom_status";
mut response: Object = { "{key}": "ok" };
response["count"] = 3;
```

Dynamic reads require checked narrowing. Contract conformance validates
required members and seals the current key set across aliases.

## Classes and enums

```zirk
class Session {
    token: String;
    active: Boolean;
}

enum ResultState {
    pending;
    ok(Int32);
    error(String);
}
```

A class owns identity, state, and lifecycle. An enum represents a closed set
of alternatives.

## Views, pointers, and common mistakes

`Weak<T>`, `Dependent<T>`, and `NativeSlice<T>` observe data without owning it.
`Pointer<T>` is for native interoperation and requires `unsafe`.

Prefer primitives for simple values, tuples for compact positional products,
contracts plus `Object` for named structural data, classes for identity, and
enums for alternatives. Do not use `Object` as a universal any-value type; use a
generic type parameter or `Dynamic` according to whether static type identity
must be preserved.

---

**Previous:** [← Native Operators](01e-native-operators.md) · **Next:** [Signed Integers](02-signed-integers.md)
