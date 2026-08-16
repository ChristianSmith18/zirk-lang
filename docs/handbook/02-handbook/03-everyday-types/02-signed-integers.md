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

All widths expose `MIN`, `MAX`, `BITS`, `abs()`, `sign()`, `min()`, `max()`,
`clamp()`, `is_zero()`, `is_even()`, `is_odd()`, `bit_count()`,
`leading_zeros()`, `trailing_zeros()`, `rotate_left()`, `rotate_right()`,
`to_string()` and typed conversion/parse operations.

Ordinary arithmetic traps on overflow. Algorithms that deliberately need a
different policy call `checked_*`, `wrapping_*`, or `saturating_*` operations.

---

**Previous:** [← Native Operators](01e-native-operators.md) · **Next:** [ Unsigned Integers](03-unsigned-integers.md)
