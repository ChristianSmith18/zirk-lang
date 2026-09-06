## ADDED Requirements

### Requirement: `Float` is an exact base-ten decimal value

`Float` SHALL be an exact base-ten decimal scalar, not a binary floating-point
type and not a member of a width family. Its value SHALL be modeled as a signed
128-bit integer coefficient and a non-negative decimal scale `s` in the range
`0..=38`, denoting the exact rational `coefficient x 10^-s`. Two `Float` values
SHALL compare equal when they denote the same rational number, independent of
the scale each carries. Every operation SHALL normalize its result by removing
trailing decimal zeros before the value is observed.

#### Scenario: Bare fractional literal is exact

- **WHEN** `inmut a, b = 0.1, 0.2;` is written and `(a + b) == 0.3` is evaluated
- **THEN** `a` and `b` have type `Float`
- **AND** the comparison evaluates to `true`

#### Scenario: Scale does not affect equality

- **WHEN** `1.0 == 1.00` and `2.50 == 2.5` are evaluated
- **THEN** both evaluate to `true`

#### Scenario: Exact representation of a value binary float cannot hold

- **WHEN** `(0.1).to_string()` is evaluated
- **THEN** the result is exactly `"0.1"`, with no trailing binary artifact digits

### Requirement: Exact addition, subtraction, multiplication, and integer power

`Float` `+`, `-`, `*`, and `**` with a non-negative integer exponent SHALL
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

Inexact `Float` operations SHALL produce a correctly rounded result at a
documented precision budget, and the default rounding mode SHALL be half-to-even.
This covers `/` with a non-terminating quotient, `**` with a negative or
fractional exponent, `sqrt`, and any future transcendental operation. The
maximum significant-digit budget for an inexact result SHALL be a documented
normative constant (`MAX_SIGNIFICANT_DIGITS`), and division SHALL compute at
least a documented minimum fractional scale (`MIN_DIV_SCALE`) before rounding. `%` SHALL be exact and SHALL preserve the sign of the dividend.
Division or modulo by a zero divisor SHALL raise a controlled
`DivisionByZeroError` at the point of the operation.

#### Scenario: Terminating division is exact

- **WHEN** `1.0 / 4.0` is evaluated
- **THEN** the result is exactly `0.25`

#### Scenario: Non-terminating division is correctly rounded

- **WHEN** `1.0 / 3.0` is evaluated with the default rounding mode
- **THEN** the result is the half-to-even rounding of one third at `MAX_SIGNIFICANT_DIGITS`
  significant digits

#### Scenario: Explicit rounding control

- **WHEN** `a.div(b, RoundingMode.HALF_UP, 2)` is evaluated
- **THEN** the quotient is rounded half-up to a scale of 2

#### Scenario: Signed remainder

- **WHEN** `-1.0 % 0.3` is evaluated
- **THEN** the result carries the sign of the dividend

#### Scenario: Division by zero is controlled

- **WHEN** a `Float` is divided by `0.0`
- **THEN** the program raises a catchable `DivisionByZeroError` at that operation

### Requirement: `Float` has no `NaN` and no infinity

The type system SHALL NOT define `NaN` or any infinity value for `Float`. `Float`
SHALL NOT expose `POSITIVE_INFINITY`, `NEGATIVE_INFINITY`, `EPSILON`,
`is_finite`, or `is_infinite`. Any operation that would require such a value
SHALL instead be a controlled runtime error.

#### Scenario: No infinity member on `Float`

- **WHEN** source names `Float.POSITIVE_INFINITY` or `Float.EPSILON`
- **THEN** an unknown-member diagnostic is emitted

#### Scenario: `sqrt` of a negative value is controlled

- **WHEN** `(-1.0).sqrt()` is evaluated
- **THEN** the program raises the controlled float-domain error, not a `NaN`

### Requirement: Conversions between `Float`, integers, and `BinaryFloat`

Conversion from any integer width to `Float` SHALL be implicit and exact.
Conversion from `Float` to an integer SHALL require an explicit `as` cast and
SHALL be checked: a non-integer value or an out-of-range value SHALL raise a
controlled error. Conversion between `Float` and any `BinaryFloat` width SHALL
require an explicit cast or constructor in both directions; `BinaryFloat -> Float`
SHALL be checked and SHALL reject a non-finite input. An arithmetic operation
with one `Float` operand and one `BinaryFloat` operand SHALL be a type error that
names the required explicit conversion.

#### Scenario: Integer widens to `Float` implicitly

- **WHEN** an `Int32` is used where `Float` is expected
- **THEN** it is accepted and represented as that integer at scale 0

#### Scenario: `Float` to integer is a checked cast

- **WHEN** `(2.5) as Int32` is evaluated
- **THEN** the program raises a controlled conversion error because `2.5` has a
  fractional part

#### Scenario: Mixing `Float` and `BinaryFloat` is rejected

- **WHEN** a `Float` value is added to a `BinaryFloat64` value
- **THEN** type checking fails and names the explicit conversion required

#### Scenario: Explicit binary-to-exact conversion

- **WHEN** `Float(x)` is written where `x` is a finite `BinaryFloat64`
- **THEN** the result is the exact decimal of the binary value, rounded to
  `MAX_SIGNIFICANT_DIGITS`, and a non-finite `x` raises a controlled error

### Requirement: `Float` member surface

`Float` SHALL expose: `abs`, `sign`, `min`, `max`, `clamp`, `is_zero`,
`is_negative`, `is_integer`, `floor`, `ceil`, `truncate`, `fraction`,
`round(places[, mode])`, `pow(exponent)`, `sqrt`, `div(other, mode, places)`,
`scale` (the decimal scale as an integer), `to_string`, and the static
`Float.parse(text) -> Result<Float, ParseError>`. `to_string` SHALL render the
exact decimal value. `format(spec)` SHALL remain specified but not implemented,
as it is for the binary family, and is out of scope for this capability.

#### Scenario: Exact `to_string`

- **WHEN** `(123.400).to_string()` is evaluated
- **THEN** the result is `"123.4"`

#### Scenario: Rounding to a scale

- **WHEN** `(2.345).round(2)` is evaluated with the default mode
- **THEN** the result is `2.34` under half-to-even

#### Scenario: `scale` reports the decimal scale

- **WHEN** `(1.50).scale` is evaluated
- **THEN** the result is `1` after trailing-zero normalization

#### Scenario: Parse round-trips

- **WHEN** `Float.parse("0.1")` succeeds and its value is printed
- **THEN** the output is `"0.1"`

### Requirement: `BinaryFloat` family carries the IEEE 754 binary semantics

The type system SHALL recognize `BinaryFloat16`, `BinaryFloat32`,
`BinaryFloat64`, and `BinaryFloat128`, with `BinaryFloat` aliasing
`BinaryFloat64`. This family SHALL have exactly the semantics the former
`Float16`/`Float32`/`Float64`/`Float128` family had: IEEE 754 binary values at
each width, explicit `POSITIVE_INFINITY` and `NEGATIVE_INFINITY`, no valid
`NaN`, an operation that would produce `NaN` as a controlled runtime error at
its point of occurrence, `BinaryFloat128` text rendering truncated to
`BinaryFloat64` precision, and the pending Windows verification for
`BinaryFloat128`. A `BinaryFloat` literal SHALL be written with a `b` suffix
(`1.5b`), optionally with a width (`1.5b32`, `0.1b128`). The former spellings
`Float16`, `Float32`, `Float64`, and `Float128` SHALL NOT resolve; the checker
SHALL emit a diagnostic naming the `BinaryFloat` replacement.

#### Scenario: Binary literal suffix

- **WHEN** `inmut x = 1.5b32;` is written
- **THEN** `x` has type `BinaryFloat32`

#### Scenario: Binary division by zero yields infinity

- **WHEN** a non-zero `BinaryFloat64` is divided by `0.0b`
- **THEN** the result is an infinity, not an error

#### Scenario: Binary indeterminate operation is controlled

- **WHEN** a `BinaryFloat` operation would produce `NaN` under IEEE 754
- **THEN** the program raises a catchable `FloatNanError` at that operation

#### Scenario: Former `Float64` spelling is redirected

- **WHEN** an annotation names `Float64`
- **THEN** a diagnostic states that the binary type is `BinaryFloat64` and the
  exact base-ten type is `Float`

### Requirement: Runtime representation and helpers for exact `Float`

The runtime SHALL represent a `Float` as a C-compatible value of a signed
128-bit coefficient and an 8-bit scale, passed across the native boundary by
pointer. The runtime SHALL provide helpers for addition, subtraction,
multiplication, division, remainder, integer power, negation, absolute value,
comparison, square root, general power, rounding, explicit-mode division,
decimal-to-text formatting, text-to-decimal parsing, integer/decimal
conversion, and binary/decimal conversion. Exact arithmetic that an integer
instruction can perform directly MAY be lowered inline; everything requiring
scale alignment, normalization, or rounding SHALL be delegated to these
helpers. `Float` values SHALL NOT be heap-allocated and SHALL NOT participate in
garbage collection.

#### Scenario: Formatting does not go through binary

- **WHEN** the runtime formats a `Float` for `to_string` or interpolation
- **THEN** it renders the coefficient and scale directly, without converting to
  any binary floating type

#### Scenario: Divide-by-zero guard precedes the helper call

- **WHEN** `a / b` on `Float` is lowered
- **THEN** a guard that raises `DivisionByZeroError` is emitted before the
  runtime division helper is called
