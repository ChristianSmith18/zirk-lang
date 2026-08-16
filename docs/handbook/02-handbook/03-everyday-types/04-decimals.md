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

Float widths expose `MIN`, `MAX`, `LOWEST`, `EPSILON`, `INFINITY` constants as
appropriate; `abs()`, `sign()`, `min()`, `max()`, `clamp()`, `is_zero()`,
`floor()`, `ceil()`, `round()`, `truncate()`, `fraction()`, `is_finite()`,
`is_infinite()`, formatting, parsing and explicit numeric conversions.

Comparisons are deterministic because no operand can be NaN. Conversions to an
integer explicitly document truncation and fail if the result is not finite or
representable.

---

**Previous:** [← Unsigned Integers](03-unsigned-integers.md) · **Next:** [ Numeric Literals](05-numeric-literals.md)
