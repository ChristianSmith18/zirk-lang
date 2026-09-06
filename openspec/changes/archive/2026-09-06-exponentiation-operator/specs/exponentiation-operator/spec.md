## ADDED Requirements

### Requirement: Infix `**` is available and desugars to `pow`

The compiler SHALL accept the infix operator `**` in every phase that
implements the numeric families. `a ** b` SHALL be defined as `a.pow(b)` and
SHALL lower to the same runtime path the `pow(exponent)` method call already
uses for the operand's type. No new runtime symbol SHALL be introduced.

The compiler SHALL NOT emit a phase diagnostic for `**` or `**=`, and SHALL
NOT reinterpret `**` as two multiplication tokens.

#### Scenario: Power expression compiles and runs

- **WHEN** a program evaluates `2 ** 3` and prints the result
- **THEN** the program compiles and prints `8`

#### Scenario: No phase diagnostic

- **WHEN** source contains `x ** 2`
- **THEN** no "not implemented in this phase" diagnostic is emitted for `**`

### Requirement: `**` precedence and associativity

`**` SHALL bind more tightly than unary `-` and `!` and than the
multiplicative operators, and SHALL be right-associative, matching the
precedence table in `docs/handbook/11-reference/02-operators-and-precedence.md`
and `ZIRK_LANGUAGE_SPEC.md` section 4.

#### Scenario: Right associativity

- **WHEN** `2 ** 3 ** 2` is parsed
- **THEN** the tree represents `2 ** (3 ** 2)` and the value is `512`

#### Scenario: Unary minus applies to the power

- **WHEN** `-2 ** 2` is parsed
- **THEN** the tree represents `-(2 ** 2)` and the value is `-4`

#### Scenario: A negated exponent is an operand

- **WHEN** `2 ** -1` is parsed
- **THEN** the right operand is the negation of `1`

#### Scenario: Tighter than multiplication

- **WHEN** `3 * 2 ** 2` is parsed
- **THEN** the tree represents `3 * (2 ** 2)` and the value is `12`

### Requirement: `**=` compound assignment

The parser SHALL accept `**=` in statement position and expand `a **= b` to
the tree of `a = a ** b`. The left operand SHALL be an assignable, mutable
place; otherwise a diagnostic pointing to the operand SHALL be emitted.

#### Scenario: Compound power assignment

- **WHEN** `mut n = 2; n **= 10;` is evaluated
- **THEN** `n` holds `1024`

#### Scenario: Compound power on a non-assignable place

- **WHEN** the left operand of `**=` is not an assignable, mutable place
- **THEN** a diagnostic pointing to the operand is emitted

### Requirement: Result type of `**` across the numeric families

The result type of `a ** b` SHALL be:

- `Int ** Int` with a statically non-negative exponent, or an exponent whose
  sign is not statically known: `Int` in the common integer type of both
  operands, with overflow checked like every other integer operation.
- `Int ** Int` where the exponent is a statically negative literal: exact
  `Float`, so `2 ** -1` evaluates to `0.5`.
- `Float ** Int` and `Float ** Float`: exact `Float`. A fractional exponent
  uses the documented f64-precision path, as `Float.pow` does.
- `BinaryFloatN ** Int` and `BinaryFloatN ** BinaryFloatN`: `BinaryFloatN`
  (f64-precision result), as `BinaryFloatN.pow` does.
- An operation with one exact `Float` operand and one `BinaryFloat` operand:
  a type error that names the required explicit conversion, identical to the
  other arithmetic operators.

#### Scenario: Integer power stays integer

- **WHEN** `3 ** 4` is evaluated
- **THEN** the result is the `Int` value `81`

#### Scenario: Integer power overflow is controlled

- **WHEN** `Int32` arithmetic evaluates `10 ** 20`
- **THEN** the program raises a controlled `ArithmeticOverflowError`, not a
  wrapped value

#### Scenario: Negative literal exponent widens to exact `Float`

- **WHEN** `2 ** -3` is evaluated
- **THEN** the result is the exact `Float` value `0.125`

#### Scenario: Exact `Float` base

- **WHEN** `(1.5) ** 2` is evaluated
- **THEN** the result is the exact `Float` value `2.25`

#### Scenario: Binary float base

- **WHEN** `(2.0b) ** 10` is evaluated
- **THEN** the result is the `BinaryFloat64` value `1024.0`

#### Scenario: Mixing exact and binary floats is rejected

- **WHEN** `(2.0) ** (3.0b)` is checked
- **THEN** type checking fails and names the explicit conversion required
