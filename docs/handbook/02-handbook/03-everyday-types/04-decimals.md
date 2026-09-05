# Floats

Zirk's binary floating family is `Float16`, `Float32`, `Float64`, and
`Float128`. `Float` is an exact alias of `Float64`, and a fractional or
scientific literal defaults to `Float64` without another context.

```zirk
inmut ratio = 0.625;             // Float64
inmut compact: Float32 = 1.5;
inmut explicit = Float128(1e-20);
```

Use a width according to required range, precision, ABI and target support.
Binary Float does not exactly represent every base-ten fraction. A future
stdlib `Decimal` can serve money and exact base-ten domains; it is not an alias
of Float.

## Arithmetic and conversion

Float supports unary sign, `+ - * / % **`, equality, order and compound
assignment. Zirk deliberately omits `++`/`--` because a floating unit is not a
safe discrete step. Mixed integer/Float operations produce Float.

```zirk
3 / 4;   // 0: Int32
3 / 4.0; // 0.75: Float64
```

An explicit outer constructor establishes deep context before contained
arithmetic executes:

```zirk
Float(3 / 4);                 // 0.75
Float((a + 1) / (b * 2));
```

It does not modify `a` or `b`, affect surrounding expressions, or enter a
called function's body.

## Infinity without NaN

`Float.POSITIVE_INFINITY` and `Float.NEGATIVE_INFINITY` exist for explicit
algorithms and interoperability. `NaN` is not a valid Zirk value. Zero division,
overflow and indeterminate operations such as positive infinity minus itself
produce controlled errors instead of silently creating special results.

## API

`Float` is an exact alias of `Float64`. `NaN` is not a valid Zirk value, so
comparisons are deterministic. `Float128` is delivered except for Windows
verification, and its formatting currently truncates to `Float64` precision.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `Float64.MIN` | `Float64` | Smallest positive normalized value | specified |
| `Float64.MAX` | `Float64` | Largest finite value | specified |
| `Float64.LOWEST` | `Float64` | Most negative finite value (`−MAX`) | specified |
| `Float64.EPSILON` | `Float64` | Difference between `1.0` and the next representable value | specified |
| `Float64.POSITIVE_INFINITY` | `Float64` | Explicit infinity for algorithms and interop | specified |
| `Float64.NEGATIVE_INFINITY` | `Float64` | Explicit negative infinity | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `value.abs()` | same as `value` | Absolute value | specified |
| `value.sign()` | `Int32` | `-1`, `0`, or `1` | specified |
| `value.min(other)` / `value.max(other)` | same as `value` | Smaller/larger of two values | specified |
| `value.clamp(low, high)` | same as `value` | Confines `value` to `[low, high]` | specified |
| `value.is_zero()` | `Boolean` | `true` when `value == 0.0` | specified |
| `value.floor()` | same as `value` | Largest integer ≤ `value` | specified |
| `value.ceil()` | same as `value` | Smallest integer ≥ `value` | specified |
| `value.round()` | same as `value` | Nearest integer, half away from zero | specified |
| `value.truncate()` | same as `value` | Integer part, toward zero | specified |
| `value.fraction()` | same as `value` | Fractional part (`value - truncate()`) | specified |
| `value.is_finite()` | `Boolean` | `true` when not an infinity | specified |
| `value.is_infinite()` | `Boolean` | `true` for either infinity | specified |
| `value.is_negative()` | `Boolean` | `true` when `value < 0.0` (includes `−0.0` sign) | specified |
| `value.pow(exp)` | same as `value` | Floating power | specified |
| `value.sqrt()` | same as `value` | Square root; negative input is a controlled error | specified |
| `Float64.parse(text)` | `Result<Float64, ParseError>` | Parses decimal/scientific text | specified |
| `value.to_string()` | `String` | Shortest round-trip decimal rendering | implemented |
| `value.format(spec)` | `String` | Contract-driven presentation formatting | specified |
| `Int32(value)` / `Float64(value)` | target type | Explicit checked conversion; fails on non-finite or unrepresentable results | implemented |

> `Float` omits `++`/`--` deliberately: a floating unit is not a safe discrete
> step. Zero division, overflow, and indeterminate operations are controlled
> errors rather than `NaN`.
>
> `Float128` formatting is **partial**: it currently truncates to `Float64`
> precision (lossy), and Windows verification is pending.

### Examples

```zirk
inmut ratio = 0.625;            // Float64
ratio.floor();                  // 0.0
(3 / 4.0).to_string();          // "0.75"
Float(3 / 4);                   // 0.75 — deep Float context

inmut near = 1.0 - Float64.EPSILON;
stdout.println(near.is_finite()); // true
```

---

**Previous:** [← Unsigned Integers](03-unsigned-integers.md) · **Next:** [ Numeric Literals](05-numeric-literals.md)
