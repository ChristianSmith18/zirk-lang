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

## API

`UInt` is an exact alias of `UInt32`. The API mirrors the signed family except
unary negation and `abs()`/`sign()`, which do not exist on unsigned values.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `UInt32.MIN` | `UInt32` | Always `0` | implemented |
| `UInt32.MAX` | `UInt32` | `2ⁿ−1` for the width | implemented |
| `UInt32.BITS` | `Int32` | Bit width of the type | implemented |

### Methods

Identical to the signed table above, with these differences:

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `value.is_zero()` … `value.rotate_right(n)` | see signed | Zero/parity tests, bit inspection, rotation | implemented |
| `value.checked_sub(other)` | `Result<UInt32, OverflowError>` | Subtraction reporting underflow | implemented |
| `value.wrapping_*` / `value.saturating_*` | same as `value` | Deliberate underflow/overflow policies | implemented |
| `UInt32.parse(text)` | `Result<UInt32, ParseError>` | Parses text; rejects leading `-` | implemented |
| `UInt32(signed_value)` | `UInt32` | Checked sign/width conversion; range failure is a controlled error | implemented |

> There is no `abs()`, `sign()`, or unary `-` on unsigned integers; apply an
> explicit signed conversion first.

### Examples

```zirk
inmut byte: UInt8 = 255;
byte.bit_count();               // 8
UInt8(0).checked_sub(1);        // Error(OverflowError)
UInt8(0) - 1;                   // controlled underflow error

inmut signed: Int32 = -1;
inmut converted = UInt32(signed); // controlled range failure
```

---

**Previous:** [← Signed Integers](02-signed-integers.md) · **Next:** [ Decimals](04-decimals.md)
