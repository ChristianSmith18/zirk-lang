# Value Classes

A value class gives a value a distinct domain type while preserving value-oriented behavior.

```zirk
value class UserId(value: UInt64);
```

`UserId` prevents an arbitrary `UInt64` from being accepted where a user identifier is required. Construction and explicit conversion protect the boundary; representation may still be optimized inline.

```zirk
fn load_user(id: UserId): Result<User, LoadError> { /* ... */ }

inmut raw: UInt64 = 42;
load_user(raw);         // Error: `UInt64` is not `UserId`.
load_user(UserId(raw)); // Explicit domain conversion.
```

Unlike `type UserId = UInt64`, which creates only an alias, a value class is a
different type. Unlike a normal class, it has no observable reference identity:
`is` is invalid, and equal contents describe the same value. The compiler may
store it directly inside another value without an intermediate pointer.

Use value classes for identifiers, units, validated strings, and quantities whose raw representation would otherwise be confused. Do not expose representation-dependent ABI assumptions without an explicit native layout contract.

Validation can live at construction boundaries, making value classes useful for
non-empty strings, normalized paths, positive quantities, currency amounts, and
units that must not be mixed accidentally.

Representation conversion is explicit unless the value class declares a safe public projection. Operator support is not inherited automatically from the representation: `value class Meters(Float)` must implement the relevant arithmetic contracts before `+` or comparison is legal.

---

**Previous:** [← Records](01-records.md) · **Next:** [ Traditional Enums](03-traditional-enums.md)
