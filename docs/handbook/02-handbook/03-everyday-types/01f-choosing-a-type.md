# Choosing a Type

The right Zirk type is the one whose copying and sharing rules match the
problem. This page turns common questions into concrete choices.

## Decision table

| Question | Choose | Example |
|---|---|---|
| Do you need a number, truth value, or single character? | An immediate primitive | `Int32`, `Boolean`, `Char` |
| Is the value a date, time, duration, or other semantic value that should not share state? | A native value | `Date`, `Duration`, `Instant` |
| Do you need shared, mutable text or a collection? | A managed reference | `String`, `Array<T>`, `List<T>` |
| Do you need a fixed product of fields with no identity? | A record | `record Point` |
| Do you need identity, inheritance, or encapsulated lifecycle? | A class | `class Session` |
| Do you need one of several named alternatives? | An enum | `enum Status` |
| Do you need a non-owning observation? | `Weak<T>` | `Weak.from(cache)` |
| Do you need a short-lived view into native or managed storage? | `Dependent<T>` / `NativeSlice<T>` | `slice[0:10]` |
| Do you need raw memory? | `Pointer<T>` inside `unsafe` | `Pointer<Int32>` |

## Immediate values first

If a value is a plain number, flag, or character, use a primitive. Primitives
are cheap to copy and never share hidden state.

```zirk
inmut count: Int32 = 0;
inmut active: Boolean = true;
inmut marker: Char = 'X';
```

Use explicit widths only at boundaries: ABI, serialization, protocol fields, or
careful numeric ranges.

```zirk
inmut header: UInt16 = 65_535;   // protocol field
inmut total: Int64 = 1_000_000;  // 64-bit counter
```

## Native values for domain meaning

If the number is not really a number, give it a type. A birthday is a `Date`.
A timeout is a `Duration`. A coordinate is a `record`.

```zirk
record Point { x: Decimal; y: Decimal; }
inmut origin = Point(x: 0.0, y: 0.0);
inmut offset = origin; // copies the whole record
```

Native values and records are independent. Modifying one does not modify the
other.

## Managed references for shared or growing data

Use `String` for text that may grow, `List<T>` for a collection that changes
size, and `Array<T>` or `String[N]` for fixed-size storage.

```zirk
mut names: List<String> = List("Ada", "Grace");
names.add("Linus");      // shared growth

mut fixed: String[4];    // four elements, fixed
```

Remember that assignment shares the same instance. If you need an independent
copy, call `clone()`.

```zirk
mut first = List(1, 2, 3);
mut second = first;
second.add(4);

// first and second both contain [1, 2, 3, 4]

mut independent = first.clone();
independent.add(5);

// first still contains [1, 2, 3, 4], independent contains [1, 2, 3, 4, 5]
```

## Records for messages, classes for lifecycles

```zirk
record Address {
    street: String;
    city: String;
}

class Session {
    token: String;
    active: Boolean;
}
```

A `record` is the right choice when only the fields matter. A `class` is the
right choice when the object has a lifecycle, identity, or internal state that
must not be duplicated.

## Enums for alternatives

When a value is one of a fixed set of cases, use an enum.

```zirk
enum ResultState {
    pending;
    ok(Int32);
    error(String);
}
```

## Views and pointers for unsafe or non-owning access

`Weak<T>`, `Dependent<T>`, and `NativeSlice<T>` are for observing data you do
not own. `Pointer<T>` is for native interoperation and lives inside `unsafe`.

```zirk
mut weak = Weak.from(large_cache);
mut slice = native_buffer[0:64]; // NativeSlice<Byte>

unsafe {
    mut raw = slice.base();
    raw.write(0);
}
```

Do not reach for a pointer when a managed collection is enough. Pointers
require the programmer to prove alignment, lifetime, and validity.

## Common mistakes

**Using a primitive for a domain concept**

```zirk
// Avoid
inmut birthday: Int64 = 20260905;

// Prefer
inmut birthday: Date = Date(2026, 9, 5);
```

**Using a class when a record is enough**

```zirk
// Avoid: two different "empty" sessions are never equal by default
class PointClass { x: Decimal; y: Decimal; }

// Prefer: value equality when only the data matters
record Point { x: Decimal; y: Decimal; }
```

**Sharing a collection and being surprised by mutation**

```zirk
mut first = List(1, 2, 3);
mut second = first;
second.add(4);

// first also contains 4: they are the same List
```

If this is not what you want, clone before sharing.

## Summary

- Prefer primitives for small, copyable values.
- Prefer records and native values for domain data.
- Prefer classes when identity and lifecycle matter.
- Prefer `String`, `Array`, and `List` for shared or growing collections.
- Prefer `Weak`, `Dependent`, and `NativeSlice` for non-owning views.
- Use `Pointer<T>` only inside `unsafe` or native interop.

---

**Previous:** [← Native Operators](01e-native-operators.md) · **Next:** [ Signed Integers](02-signed-integers.md)
