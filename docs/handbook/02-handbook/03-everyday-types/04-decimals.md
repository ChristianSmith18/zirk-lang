# Exact decimals (`Float`)

`Float` is Zirk's everyday fractional type, and it is an **exact base-ten
decimal** — not IEEE 754 binary floating point. A value is a signed 128-bit
integer coefficient with a decimal scale, so it holds up to 38 significant
digits with no rounding artifacts:

```zirk
inmut a, b: Float = 0.1, 0.2;
(a + b) == 0.3;                  // true
inmut ratio = 0.625;            // Float — no annotation needed
```

A suffix-less fractional or scientific literal is a `Float`. A literal that
would need more than 38 significant digits to be exact is a compile error that
points you at `BinaryFloat` (below).

`Float` has **no `NaN` and no infinity**. Coefficient overflow past 38 digits
and division by zero are controlled errors, the same as integer overflow.

## Arithmetic and rounding

`+`, `-`, `*` and integer `**` are **exact**. `/`, `%`, `sqrt` and a fractional
`pow` round **half-to-even** ("banker's rounding") to a 28-significant-digit
budget. Zirk omits `++`/`--` because a fractional unit is not a safe discrete
step.

```zirk
0.5 + 0.025;    // 0.525  — exact
1.2 * 0.5;      // 0.6    — exact
1.0 / 3.0;      // 0.333…3 (28 digits), half-to-even
(2.345).round(2);          // 2.34 — half-to-even at the tie
(10.0).div(3.0, 4);        // 3.3333 — explicit place count
```

Every integer converts to `Float` implicitly and exactly, and mixed
integer/`Float` arithmetic produces `Float`:

```zirk
3 / 4;     // 0: Int32
3 / 4.0;   // 0.75: Float
1 + 0.5;   // 1.5: Float
```

An explicit outer `Float(...)` establishes a deep context before the contained
arithmetic runs — it does not modify the operands or reach into a called
function:

```zirk
Float(3 / 4);   // 0.75
Float((a + 1) / (b * 2));
```

## `BinaryFloat` — IEEE 754 when you need it

For graphics, DSP, machine learning, or C ABI interop, use the binary family:
`BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`, with
`BinaryFloat` aliasing `BinaryFloat64`. Write a binary literal with a `b`
suffix:

```zirk
inmut fast: BinaryFloat64 = 1.5b;
inmut compact = 1.5b32;                 // BinaryFloat32
inmut wide: BinaryFloat128 = 1e-20b128;
```

`BinaryFloat` keeps every IEEE behavior: `BinaryFloat64.POSITIVE_INFINITY` /
`NEGATIVE_INFINITY` are valid values, `NaN` is not (a `NaN`-producing operation
is a controlled error), and `BinaryFloat64 / 0.0b` is an infinity, not an
error. `BinaryFloat128` formatting still truncates to `BinaryFloat64` precision,
and Windows verification is pending.

`Float` and `BinaryFloat` never mix in one operation — convert explicitly with
`BinaryFloat(x)` / `Float(x)` (or `x as BinaryFloat64` / `x as Float`).
`BinaryFloat -> Float` is checked and rejects a non-finite input.

## API — `Float`

`value.to_string()` renders the **exact** decimal (no shortest-round-trip
heuristic). Equality is exact after scale alignment, and trailing zeros are
normalized so `1.0 == 1.00` and `(2.50).scale()` is `1`.

| Signature | Returns | Description |
| --- | --- | --- |
| `value.abs()` | `Float` | Absolute value |
| `value.sign()` | `Int32` | `-1`, `0`, or `1` |
| `value.scale()` | `Int32` | Number of decimal places after normalization |
| `value.min(other)` / `value.max(other)` | `Float` | Smaller / larger |
| `value.clamp(low, high)` | `Float` | Confines `value` to `[low, high]` |
| `value.is_zero()` / `value.is_negative()` / `value.is_integer()` | `Boolean` | Predicates |
| `value.floor()` / `value.ceil()` / `value.truncate()` | `Float` | To an integer, by direction |
| `value.fraction()` | `Float` | Fractional part (`value - truncate()`) |
| `value.round()` / `value.round(places)` | `Float` | Half-to-even to an integer or to `places` |
| `value.pow(exp)` | `Float` | Power (exact for an integer exponent) |
| `value.sqrt()` | `Float` | Square root; a negative input is a controlled error |
| `value.div(other)` / `value.div(other, places)` | `Float` | Division with a chosen place count |
| `Float.parse(text)` | `Result<Float, ParseError>` | Parses decimal / scientific text |
| `value.to_string()` | `String` | Exact decimal rendering |
| `Int32(value)` | `Int32` | Checked conversion; fails on a fractional or out-of-range value |

> `Float.format(spec)` is specified but not yet implemented (as on
> `BinaryFloat`). `RoundingMode` selection for `div` / `round` is deferred;
> both default to half-to-even.

The `BinaryFloat` member surface is the classic IEEE one — it adds `EPSILON`,
`MIN`/`MAX`/`LOWEST`, `POSITIVE_INFINITY`/`NEGATIVE_INFINITY`, `is_finite()` and
`is_infinite()`, none of which apply to the exact type.

## Migration from the old `Float64` family

`Float16`, `Float32`, `Float64` and `Float128` were removed as spellings. The
compiler points each one at its replacement:

| Old | New |
| --- | --- |
| `Float64` (as "the plain float") | `Float` (now exact base-ten) |
| `Float64` (binary) | `BinaryFloat64` |
| `Float16` / `Float32` / `Float128` | `BinaryFloat16` / `BinaryFloat32` / `BinaryFloat128` |
| `1.5f32` literal | `1.5b32` |
| `1.5` where binary was assumed | `1.5b` |

### Examples

```zirk
inmut price: Float = 19.99;
inmut tax = price * 0.08;          // 1.5992 — exact
inmut total = (price + tax).round(2);   // 21.59

inmut near = 1.0b - BinaryFloat64.EPSILON;
stdout.println(near.is_finite());  // true
```

---

**Previous:** [← Unsigned Integers](03-unsigned-integers.md) · **Next:** [ Numeric Literals](05-numeric-literals.md)
