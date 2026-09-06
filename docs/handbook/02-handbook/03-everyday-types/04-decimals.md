# Exact decimals (`Decimal`)

`Decimal` is Zirk's everyday fractional type, and it is an **exact base-ten
decimal** — not IEEE 754 binary floating point. A value is a signed 128-bit
integer coefficient with a decimal scale, so it holds up to 38 significant
digits with no rounding artifacts:

```zirk
inmut a, b: Decimal = 0.1, 0.2;
(a + b) == 0.3;                  // true
inmut ratio = 0.625;            // Decimal — no annotation needed
```

A suffix-less fractional or scientific literal is a `Decimal`. A literal that
would need more than 38 significant digits to be exact is a compile error that
points you at `Float` (below).

`Decimal` has **no `NaN` and no infinity**. Coefficient overflow past 38 digits
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

Every integer converts to `Decimal` implicitly and exactly, and mixed
integer/`Decimal` arithmetic produces `Decimal`:

```zirk
3 / 4;     // 0: Int32
3 / 4.0;   // 0.75: Decimal
1 + 0.5;   // 1.5: Decimal
```

An explicit outer `Decimal(...)` establishes a deep context before the contained
arithmetic runs — it does not modify the operands or reach into a called
function:

```zirk
Decimal(3 / 4);   // 0.75
Decimal((a + 1) / (b * 2));
```

## `Float` — IEEE 754 when you need it

For graphics, DSP, machine learning, or C ABI interop, use the binary family:
`Float16`, `Float32`, `Float64`, `Float128`, with `Float` aliasing `Float64`.
Write a binary literal with a `b` suffix (the new `f` suffix is also accepted
and preferred in new code):

```zirk
inmut fast: Float64 = 1.5b;
inmut compact = 1.5b32;                 // Float32
inmut wide: Float128 = 1e-20b128;
```

`Float` keeps every IEEE behavior: `Float64.POSITIVE_INFINITY` /
`NEGATIVE_INFINITY` are valid values, `NaN` is not (a `NaN`-producing operation
is a controlled error), and `Float64 / 0.0b` is an infinity, not an error.
`Float128` formatting still truncates to `Float64` precision, and Windows
verification is pending.

`Decimal` and `Float` never mix in one operation — convert explicitly with
`Float(x)` / `Decimal(x)` (or `x as Float64` / `x as Decimal`).
`Float -> Decimal` is checked and rejects a non-finite input.

## API — `Decimal`

`value.to_string()` renders the **exact** decimal (no shortest-round-trip
heuristic). Equality is exact after scale alignment, and trailing zeros are
normalized so `1.0 == 1.00` and `(2.50).scale()` is `1`.

| Signature | Returns | Description |
| --- | --- | --- |
| `value.abs()` | `Decimal` | Absolute value |
| `value.sign()` | `Int32` | `-1`, `0`, or `1` |
| `value.scale()` | `Int32` | Number of decimal places after normalization |
| `value.min(other)` / `value.max(other)` | `Decimal` | Smaller / larger |
| `value.clamp(low, high)` | `Decimal` | Confines `value` to `[low, high]` |
| `value.is_zero()` / `value.is_negative()` / `value.is_integer()` | `Boolean` | Predicates |
| `value.floor()` / `value.ceil()` / `value.truncate()` | `Decimal` | To an integer, by direction |
| `value.fraction()` | `Decimal` | Fractional part (`value - truncate()`) |
| `value.round()` / `value.round(places)` | `Decimal` | Half-to-even to an integer or to `places` |
| `value.pow(exp)` | `Decimal` | Power (exact for an integer exponent) |
| `value.sqrt()` | `Decimal` | Square root; a negative input is a controlled error |
| `value.div(other)` / `value.div(other, places)` | `Decimal` | Division with a chosen place count |
| `Decimal.parse(text)` | `Result<Decimal, ParseError>` | Parses decimal / scientific text |
| `value.to_string()` | `String` | Exact decimal rendering |
| `Int32(value)` | `Int32` | Checked conversion; fails on a fractional or out-of-range value |

> `Decimal.format(spec)` is specified but not yet implemented (as on `Float`).
> `RoundingMode` selection for `div` / `round` is deferred; both default to
> half-to-even.

The `Float` member surface is the classic IEEE one — it adds `EPSILON`,
`MIN`/`MAX`/`LOWEST`, `POSITIVE_INFINITY`/`NEGATIVE_INFINITY`, `is_finite()` and
`is_infinite()`, none of which apply to the exact type.

## Migration from the old `Float64` family

The old `Float64` family has been renamed. The compiler points each one at its
replacement:

| Old | New |
| --- | --- |
| `Float` (exact base-ten) | `Decimal` |
| `Float` (binary alias) | `Float64` |
| `BinaryFloat` (binary alias) | `Float64` |
| `BinaryFloat16` / `BinaryFloat32` / `BinaryFloat64` / `BinaryFloat128` | `Float16` / `Float32` / `Float64` / `Float128` |
| `1.5b32` literal | `1.5b32` or `1.5f32` |
| `1.5b` literal | `1.5b` or `1.5f` |
| `1.5` where `Float` was exact | `1.5` (now `Decimal`) or `1.5d` |

### Examples

```zirk
inmut price: Decimal = 19.99;
inmut tax = price * 0.08;          // 1.5992 — exact
inmut total = (price + tax).round(2);   // 21.59

inmut near = 1.0b - Float64.EPSILON;
stdout.println(near.is_finite());  // true
```

---

**Previous:** [← Unsigned Integers](03-unsigned-integers.md) · **Next:** [ Numeric Literals](05-numeric-literals.md)
