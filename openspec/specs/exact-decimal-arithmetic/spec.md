# exact-decimal-arithmetic Specification

## Purpose
TBD - created by archiving change exact-decimal-float. Update Purpose after archive.
## Requirements
### Requirement: `Decimal` is an exact base-ten decimal value

`Decimal` SHALL be an exact base-ten decimal scalar, not a binary floating-point
type and not a member of a width family. Its value SHALL be modeled as a signed
128-bit integer coefficient and a non-negative decimal scale `s` in the range
`0..=38`, denoting the exact rational `coefficient x 10^-s`. Two `Decimal` values
SHALL compare equal when they denote the same rational number, independent of
the scale each carries. Every operation SHALL normalize its result by removing
trailing decimal zeros before the value is observed.

#### Scenario: Bare fractional literal is exact

- **WHEN** `inmut a, b = 0.1, 0.2;` is written and `(a + b) == 0.3` is evaluated
- **THEN** `a` and `b` have type `Decimal`
- **AND** the comparison evaluates to `true`

#### Scenario: Scale does not affect equality

- **WHEN** `1.0 == 1.00` and `2.50 == 2.5` are evaluated
- **THEN** both evaluate to `true`

#### Scenario: Exact representation of a value binary float cannot hold

- **WHEN** `(0.1).to_string()` is evaluated
- **THEN** the result is exactly `"0.1"`, with no trailing binary artifact digits

### Requirement: Exact addition, subtraction, multiplication, and integer power

`Decimal` `+`, `-`, `*`, and `**` with a non-negative integer exponent SHALL
produce the mathematically exact result whenever that result fits within a
128-bit coefficient after normalization. Addition and subtraction SHALL align
the operands to the larger scale before combining coefficients. Multiplication
SHALL add the operand scales. When an intermediate coefficient exceeds 128 bits,
the implementation SHALL use a wider intermediate and, only if the normalized
result still exceeds the 128-bit coefficient budget, SHALL raise a controlled
`ArithmeticOverflowError` at the point of the operation.

#### Scenario: Exact sum with differing scales

- **WHEN** `0.5 + 0.025` is evaluated
- **THEN** the result is exactly `0.525`

#### Scenario: Exact product

- **WHEN** `1.2 * 0.5` is evaluated
- **THEN** the result is exactly `0.6`

#### Scenario: Coefficient overflow is a controlled error

- **WHEN** a multiplication produces a value that needs more than 38 significant
  decimal digits to represent exactly and cannot be represented after rounding
- **THEN** the program raises a controlled `ArithmeticOverflowError` at that
  operation, naming the digit budget, and does not silently switch to binary

#### Scenario: Integer power is exact

- **WHEN** `(1.1) ** 3` is evaluated
- **THEN** the result is exactly `1.331`

### Requirement: Division, modulo, and irrational operations round half-to-even

Inexact `Decimal` operations SHALL produce a rounded result with the default
rounding mode half-to-even. A non-terminating `/` SHALL carry about
`MAX_SIGNIFICANT_DIGITS` significant digits (division is pure integer
arithmetic); `sqrt` and a fractional `pow` carry `f64`-grade precision (about
15 significant digits) — an exact-to-28-digits irrational result is a
documented non-goal. Division SHALL compute at least `MIN_DIV_SCALE` fractional
digits before rounding. `%` SHALL be exact and SHALL preserve the sign of the
dividend. Division or modulo by a zero divisor SHALL raise a controlled
`DivisionByZeroError` at the point of the operation.

#### Scenario: Terminating division is exact

- **WHEN** `1.0 / 4.0` is evaluated
- **THEN** the result is exactly `0.25`

#### Scenario: Non-terminating division is correctly rounded

- **WHEN** `1.0 / 3.0` is evaluated with the default rounding mode
- **THEN** the result is the half-to-even rounding of one third to about
  `MAX_SIGNIFICANT_DIGITS` significant digits

#### Scenario: Explicit place count on division

- **WHEN** `a.div(b, 2)` is evaluated
- **THEN** the quotient is rounded to a scale of 2 (half-to-even; an explicit
  `RoundingMode` argument is a deferred addition)

#### Scenario: Signed remainder

- **WHEN** `-1.0 % 0.3` is evaluated
- **THEN** the result carries the sign of the dividend

#### Scenario: Division by zero is controlled

- **WHEN** a `Decimal` is divided by `0.0`
- **THEN** the program raises a catchable `DivisionByZeroError` at that operation

### Requirement: `Decimal` has no `NaN` and no infinity

The type system SHALL NOT define `NaN` or any infinity value for `Decimal`. `Decimal`
SHALL NOT expose `POSITIVE_INFINITY`, `NEGATIVE_INFINITY`, `EPSILON`,
`is_finite`, or `is_infinite`. Any operation that would require such a value
SHALL instead be a controlled runtime error.

#### Scenario: No infinity member on `Decimal`

- **WHEN** source names `Decimal.POSITIVE_INFINITY` or `Decimal.EPSILON`
- **THEN** an unknown-member diagnostic is emitted

#### Scenario: `sqrt` of a negative value is controlled

- **WHEN** `(-1.0).sqrt()` is evaluated
- **THEN** the program raises the controlled float-domain error, not a `NaN`

### Requirement: Conversions between `Decimal`, integers, and `Float`

Conversion from any integer width to `Decimal` SHALL be implicit and exact.
Conversion from `Decimal` to an integer SHALL require an explicit `as` cast and
SHALL be checked: a non-integer value or an out-of-range value SHALL raise a
controlled error. Conversion between `Decimal` and any `Float` width SHALL
require an explicit cast or constructor in both directions; `Float -> Decimal`
SHALL be checked and SHALL reject a non-finite input. An arithmetic operation
with one `Decimal` operand and one `Float` operand SHALL be a type error that
names the required explicit conversion.

#### Scenario: Integer widens to `Decimal` implicitly

- **WHEN** an `Int32` is used where `Decimal` is expected
- **THEN** it is accepted and represented as that integer at scale 0

#### Scenario: `Decimal` to integer is a checked cast

- **WHEN** `(2.5) as Int32` is evaluated
- **THEN** the program raises a controlled conversion error because `2.5` has a
  fractional part

#### Scenario: Mixing `Decimal` and `Float` is rejected

- **WHEN** a `Decimal` value is added to a `Float64` value
- **THEN** type checking fails and names the explicit conversion required

#### Scenario: Explicit binary-to-exact conversion

- **WHEN** `Decimal(x)` is written where `x` is a finite `Float64`
- **THEN** the result is the exact decimal of the binary value, rounded to
  `MAX_SIGNIFICANT_DIGITS`, and a non-finite `x` raises a controlled error

### Requirement: `Decimal` member surface

`Decimal` SHALL expose: `abs()`, `sign()`, `min(other)`, `max(other)`,
`clamp(low, high)`, `is_zero()`, `is_negative()`, `is_integer()`, `floor()`,
`ceil()`, `truncate()`, `fraction()`, `round()` / `round(places)`,
`pow(exponent)`, `sqrt()`, `div(other)` / `div(other, places)`, `scale()` (the
decimal scale as an integer), `to_string()`, and the static
`Decimal.parse(text) -> Result<Decimal, ParseError>`. `to_string` SHALL render the
exact decimal value. `format(spec)` and an explicit `RoundingMode` argument
SHALL remain specified but not implemented, as `format` is for the binary
family; both round half-to-even by default.

#### Scenario: Exact `to_string`

- **WHEN** `(123.400).to_string()` is evaluated
- **THEN** the result is `"123.4"`

#### Scenario: Rounding to a scale

- **WHEN** `(2.345).round(2)` is evaluated with the default mode
- **THEN** the result is `2.34` under half-to-even

#### Scenario: `scale()` reports the decimal scale

- **WHEN** `(1.50).scale()` is evaluated
- **THEN** the result is `1` after trailing-zero normalization

#### Scenario: Parse round-trips

- **WHEN** `Decimal.parse("0.1")` succeeds and its value is printed
- **THEN** the output is `"0.1"`

### Requirement: `Float` family carries the IEEE 754 binary semantics

The type system SHALL recognize `Float16`, `Float32`,
`Float64`, and `Float128`, with `Float` aliasing
`Float64`. This family SHALL have exactly the semantics the former
`BinaryFloat16`/`BinaryFloat32`/`BinaryFloat64`/`BinaryFloat128` family had: IEEE 754 binary values at
each width, explicit `POSITIVE_INFINITY` and `NEGATIVE_INFINITY`, no valid
`NaN`, an operation that would produce `NaN` as a controlled runtime error at
its point of occurrence, `Float128` text rendering truncated to
`Float64` precision, and the pending Windows verification for
`Float128`. A `Float` literal SHALL be written with an `f` suffix
(`1.5f`), optionally with a width (`1.5f32`, `0.1f128`). The former `b`/`bN`
literal suffixes and the `BinaryFloat*` spellings SHALL NOT resolve; the
lexer SHALL emit a diagnostic pointing to `f`/`fN` and the checker SHALL emit
a diagnostic naming the `Float` replacement.

#### Scenario: Binary literal suffix

- **WHEN** `inmut x = 1.5f32;` is written
- **THEN** `x` has type `Float32`

#### Scenario: Old `b` literal suffix is rejected

- **WHEN** `inmut x = 1.5b32;` is written
- **THEN** a diagnostic states that the binary-float suffix is now `f`/`fN`

#### Scenario: Binary division by zero yields infinity

- **WHEN** a non-zero `Float64` is divided by `0.0f`
- **THEN** the result is an infinity, not an error

#### Scenario: Binary indeterminate operation is controlled

- **WHEN** a `Float` operation would produce `NaN` under IEEE 754
- **THEN** the program raises a catchable `FloatNanError` at that operation

#### Scenario: Former `BinaryFloat64` spelling is redirected

- **WHEN** an annotation names `BinaryFloat64`
- **THEN** a diagnostic states that the binary type is `Float64` and the
  exact base-ten type is `Decimal`

### Requirement: Runtime representation and helpers for exact `Decimal`

The runtime SHALL represent a `Decimal` as a C-compatible value of a signed
128-bit coefficient and an 8-bit scale, passed across the native boundary by
pointer. The runtime SHALL provide helpers for addition, subtraction,
multiplication, division, remainder, integer power, negation, absolute value,
comparison, square root, general power, rounding, explicit-mode division,
decimal-to-text formatting, text-to-decimal parsing, integer/decimal
conversion, and binary/decimal conversion. Exact arithmetic that an integer
instruction can perform directly MAY be lowered inline; everything requiring
scale alignment, normalization, or rounding SHALL be delegated to these
helpers. `Decimal` values SHALL NOT be heap-allocated and SHALL NOT participate in
garbage collection.

#### Scenario: Formatting does not go through binary

- **WHEN** the runtime formats a `Decimal` for `to_string` or interpolation
- **THEN** it renders the coefficient and scale directly, without converting to
  any binary floating type

#### Scenario: Divide-by-zero guard precedes the helper call

- **WHEN** `a / b` on `Decimal` is lowered
- **THEN** a guard that raises `DivisionByZeroError` is emitted before the
  runtime division helper is called

