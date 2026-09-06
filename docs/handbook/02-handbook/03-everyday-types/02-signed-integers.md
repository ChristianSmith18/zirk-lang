# Signed integers

Zirk's signed integer family covers `Int8`, `Int16`, `Int32`, `Int64` and
`Int128`. The plain name `Int` is an alias for `Int32`.

Signed integers use two's-complement representation. Arithmetic is checked by
default: ordinary `+`, `-`, `*`, `/`, `%` and `**` trap on overflow or division
by zero. The `checked_*`, `wrapping_*` and `saturating_*` families provide
explicit alternatives.

## Literals

Decimal literals are written as plain numbers. A negative literal is a unary
`-` applied to a positive literal.

```zirk
mut zero: Int32 = 0;
mut one: Int64 = 1;
mut small: Int8 = -128;
mut large: Int128 = 170141183460469231731687303715884105727;
```

## Construction and conversion

`IntN(value)` converts another numeric value to `IntN`, checking the value fits
at run time.

```zirk
mut a: Int32 = Int32(42.0);   // ok
mut b: Int32 = Int32(300.0);  // error: does not fit in Int32
```

## Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `Int32.MIN` | `Int32` | Smallest representable value | implemented |
| `Int32.MAX` | `Int32` | Largest representable value | implemented |
| `Int32.BITS` | `Int32` | Bit width of the type (`8`, `16`, `32`, `64`, `128`) | implemented |

## Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `value.abs()` | same as `value` | Absolute value; traps on `MIN` | implemented |
| `value.sign()` | `Int32` | `-1`, `0`, or `1` | implemented |
| `value.min(other)` | same as `value` | Smaller of the two values | implemented |
| `value.max(other)` | same as `value` | Larger of the two values | implemented |
| `value.clamp(low, high)` | same as `value` | Confines `value` to `[low, high]` | implemented |
| `value.is_zero()` | `Boolean` | `true` when `value == 0` | implemented |
| `value.is_even()` | `Boolean` | `true` when divisible by 2 | implemented |
| `value.is_odd()` | `Boolean` | `true` when not divisible by 2 | implemented |
| `value.bit_count()` | `Int32` | Number of set bits (population count) | implemented |
| `value.leading_zeros()` | `Int32` | Zero bits above the highest set bit | implemented |
| `value.trailing_zeros()` | `Int32` | Zero bits below the lowest set bit | implemented |
| `value.rotate_left(n)` | same as `value` | Circular left shift by `n` | implemented |
| `value.rotate_right(n)` | same as `value` | Circular right shift by `n` | implemented |
| `value.checked_add(other)` | `Result<Int32, OverflowError>` | Addition that reports overflow | implemented |
| `value.checked_sub(other)` | `Result<Int32, OverflowError>` | Subtraction that reports overflow | implemented |
| `value.checked_mul(other)` | `Result<Int32, OverflowError>` | Multiplication that reports overflow | implemented |
| `value.checked_div(other)` | `Result<Int32, OverflowError>` | Division reporting overflow/zero division | implemented |
| `value.checked_rem(other)` | `Result<Int32, OverflowError>` | Remainder reporting zero division | implemented |
| `value.checked_pow(exp)` | `Result<Int32, OverflowError>` | Power reporting overflow | implemented |
| `value.wrapping_add(other)` | same as `value` | Two's-complement wraparound addition | implemented |
| `value.wrapping_sub(other)` | same as `value` | Wraparound subtraction | implemented |
| `value.wrapping_mul(other)` | same as `value` | Wraparound multiplication | implemented |
| `value.saturating_add(other)` | same as `value` | Addition clamped to `MIN`/`MAX` | implemented |
| `value.saturating_sub(other)` | same as `value` | Subtraction clamped to `MIN`/`MAX` | implemented |
| `value.saturating_mul(other)` | same as `value` | Multiplication clamped to `MIN`/`MAX` | implemented |
| `Int32.parse(text)` | `Result<Int32, ParseError>` | Parses decimal text | implemented |
| `Int32.parse(text, radix:)` | `Result<Int32, ParseError>` | Parses text in radix 2–36 | implemented |
| `value.to_string()` | `String` | Decimal rendering | implemented |
| `value.to_string(radix:)` | `String` | Rendering in radix 2–36 | specified |
| `Int32(value)` | `Int32` | Checked explicit conversion from any numeric type | implemented |

> **Overflow policy:** ordinary `+ - * / % **` trap on overflow. The
> `checked_*` family reports failure through `Result`; `wrapping_*` and
> `saturating_*` fold the result instead. `abs()` traps on the minimum value,
> because `|MIN|` is one larger than `MAX` in two's complement.

## Pitfalls

- `Int8` and `Int128` may not be natively aligned on every target, but their
  values are always exact.
- Division and remainder trap on zero.
- Mixed-width arithmetic requires an explicit cast; there is no implicit
  widening.
