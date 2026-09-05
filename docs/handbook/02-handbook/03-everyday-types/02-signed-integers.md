# Signed Integers

Zirk provides `Int8`, `Int16`, `Int32`, `Int64`, and `Int128`. `Int` and
`Integer` are exact aliases of the default `Int32`; aliases do not create new
types.

| Type | Minimum | Maximum |
| --- | ---: | ---: |
| `Int8` | −2⁷ | 2⁷−1 |
| `Int16` | −2¹⁵ | 2¹⁵−1 |
| `Int32` | −2³¹ | 2³¹−1 |
| `Int64` | −2⁶³ | 2⁶³−1 |
| `Int128` | −2¹²⁷ | 2¹²⁷−1 |

An unconstrained whole-number literal is `Int32` when it fits. Context may
select another width:

```zirk
inmut ordinary = 42;                 // Int32
inmut distance: Int64 = 4_000_000_000;
inmut explicit = Int8(10);
```

Use explicit widths for ABI boundaries, serialization, persistent data and
domain ranges. Values have value semantics and may be stored inline.

## Operators

Signed integers support unary `+`/`-`, arithmetic `+ - * / % **`, equality,
order, prefix/postfix `++`/`--`, compound assignment, and bitwise
`& | ^ ~ << >>` with corresponding compounds.

Division truncates toward zero and remainder keeps the dividend sign:

```zirk
3 / 4;    // 0: Int32
-10 / 3;  // -3: Int32
-10 % 3;  // -1: Int32
```

Integer negative powers stay in the integer domain and truncate fractional
results; request Float context for a fractional result:

```zirk
2 ** -1;        // 0: Int32
Float(2 ** -1); // 0.5: Float64
```

Division by zero, a negative/oversized shift, or an unrepresentable result is a
controlled error. Constant errors can be diagnosed during compilation.

## Conversion

Widening that preserves every source value may be implicit when the target is
unambiguous. Narrowing, signed/unsigned conversion and Float-to-integer
conversion are explicit and checked. Parsing text returns `Result`.

## API

`Int` and `Integer` are exact aliases of `Int32`. All widths share the same API;
signatures below use `Int32` as the example width. Methods that take another
integer parameter (`min`, `max`, `clamp`, `rotate_*`, shifts) accept the same
width unless noted.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `Int32.MIN` | `Int32` | Smallest representable value | specified |
| `Int32.MAX` | `Int32` | Largest representable value | specified |
| `Int32.BITS` | `Int32` | Bit width of the type (`8`, `16`, `32`, `64`, `128`) | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `value.abs()` | same as `value` | Absolute value; traps on `MIN` | specified |
| `value.sign()` | `Int32` | `-1`, `0`, or `1` | specified |
| `value.min(other)` | same as `value` | Smaller of the two values | specified |
| `value.max(other)` | same as `value` | Larger of the two values | specified |
| `value.clamp(low, high)` | same as `value` | Confines `value` to `[low, high]` | specified |
| `value.is_zero()` | `Boolean` | `true` when `value == 0` | specified |
| `value.is_even()` | `Boolean` | `true` when divisible by 2 | specified |
| `value.is_odd()` | `Boolean` | `true` when not divisible by 2 | specified |
| `value.bit_count()` | `Int32` | Number of set bits (population count) | specified |
| `value.leading_zeros()` | `Int32` | Zero bits above the highest set bit | specified |
| `value.trailing_zeros()` | `Int32` | Zero bits below the lowest set bit | specified |
| `value.rotate_left(n)` | same as `value` | Circular left shift by `n` | specified |
| `value.rotate_right(n)` | same as `value` | Circular right shift by `n` | specified |
| `value.checked_add(other)` | `Result<Int32, OverflowError>` | Addition that reports overflow | specified |
| `value.checked_sub(other)` | `Result<Int32, OverflowError>` | Subtraction that reports overflow | specified |
| `value.checked_mul(other)` | `Result<Int32, OverflowError>` | Multiplication that reports overflow | specified |
| `value.checked_div(other)` | `Result<Int32, OverflowError>` | Division reporting overflow/zero division | specified |
| `value.checked_rem(other)` | `Result<Int32, OverflowError>` | Remainder reporting zero division | specified |
| `value.checked_pow(exp)` | `Result<Int32, OverflowError>` | Power reporting overflow | specified |
| `value.wrapping_add(other)` | same as `value` | Two's-complement wraparound addition | specified |
| `value.wrapping_sub(other)` | same as `value` | Wraparound subtraction | specified |
| `value.wrapping_mul(other)` | same as `value` | Wraparound multiplication | specified |
| `value.saturating_add(other)` | same as `value` | Addition clamped to `MIN`/`MAX` | specified |
| `value.saturating_sub(other)` | same as `value` | Subtraction clamped to `MIN`/`MAX` | specified |
| `value.saturating_mul(other)` | same as `value` | Multiplication clamped to `MIN`/`MAX` | specified |
| `Int32.parse(text)` | `Result<Int32, ParseError>` | Parses decimal text | specified |
| `Int32.parse(text, radix:)` | `Result<Int32, ParseError>` | Parses text in radix 2–36 | specified |
| `value.to_string()` | `String` | Decimal rendering | implemented |
| `value.to_string(radix:)` | `String` | Rendering in radix 2–36 | specified |
| `Int32(value)` | `Int32` | Checked explicit conversion from any numeric type | implemented |

> **Overflow policy:** ordinary `+ - * / % **` trap on overflow. The
> `checked_*` family reports failure through `Result`; `wrapping_*` and
> `saturating_*` express deliberate alternatives. The `checked_*`/`wrapping_*`/
> `saturating_*` families are specified; ordinary trapping arithmetic is what
> the compiler delivers today.

### Examples

```zirk
inmut value: Int32 = -7;
value.abs();                    // 7
value.sign();                   // -1
(255).rotate_left(4);           // 4080
stdout.println(Int32.MAX);      // 2147483647

mut total: Int64 = 0;
total += value;                 // widening through explicit context

match Int32.parse("42") {
    Ok(n) => stdout.println(n),
    Error(e) => stdout.println("invalid: {e}"),
}
```

---

**Previous:** [← Choosing a Type](01f-choosing-a-type.md) · **Next:** [ Unsigned Integers](03-unsigned-integers.md)
