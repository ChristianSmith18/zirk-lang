# Unsigned Integers

`UInt8`, `UInt16`, `UInt32`, `UInt64`, and `UInt128` represent fixed-width
non-negative integers with ranges `0 .. 2ⁿ−1`.

```zirk
inmut byte: UInt8 = 255;
inmut request_id: UInt64 = 42;
```

Use unsigned values for bit patterns, protocol fields, explicit non-negative
domains and native interfaces. Do not choose them mechanically for every count:
subtraction can still underflow, and a difference may naturally be negative.

## Operators

Unsigned integers support the signed integer operator set except unary
negation. Arithmetic, increment/decrement, bitwise operations, shifts,
comparison and compound assignment all preserve the concrete width unless an
explicit wider context applies.

```zirk
-byte;       // error: UInt8 is not Negatable
UInt8(0)-1;  // controlled underflow
```

Shift counts must be non-negative and less than the width. Ordinary overflow
and underflow are controlled errors; checked, wrapping and saturating methods
express deliberate alternatives.

## Signed interaction

Zirk does not silently reinterpret sign:

```zirk
inmut signed: Int32 = -1;
inmut invalid: UInt32 = signed; // error
inmut checked = UInt32(signed); // controlled range failure
```

Mixed signed/unsigned arithmetic requires an explicit common type. The API
otherwise mirrors signed integers: constants, zero/parity tests, bit
inspection/rotation, bounds, formatting, parsing and explicit conversions.

---

**Previous:** [← Signed Integers](02-signed-integers.md) · **Next:** [ Floats](04-decimals.md)
