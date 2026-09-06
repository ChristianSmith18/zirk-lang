# zirk-scalar-members Specification

## Purpose
TBD - created by archiving change native-type-member-surface. Update Purpose after archive.
## Requirements
### Requirement: Integer static members

Each integer width type SHALL expose `MIN`, `MAX`, `BITS`, and `parse` as
static members resolved through the type name in expression position.

#### Scenario: Width constants are readable

- **WHEN** a program reads `Int8.MIN`, `Int32.MAX`, or `UInt64.BITS`
- **THEN** the value is the width's own bound or bit count, typed at that
  width (`BITS` as `Int32`)

#### Scenario: Parse accepts valid decimal text

- **WHEN** a program calls `Int32.parse("42")`
- **THEN** the result is `Ok(42)` of `Result<Int32, ParseError>`

#### Scenario: Parse rejects invalid text

- **WHEN** a program calls `Int32.parse("nope")`
- **THEN** the result is `Error` with a `ParseError`

#### Scenario: Unsigned parse rejects a leading minus

- **WHEN** a program calls `UInt32.parse("-1")`
- **THEN** the result is `Error` with a `ParseError`

### Requirement: Integer value methods

An integer value SHALL expose `abs`, `sign`, `min`, `max`, `clamp`,
`is_zero`, `is_even`, `is_odd`, `bit_count`, `leading_zeros`,
`trailing_zeros`, `rotate_left`, and `rotate_right`, all preserving the
receiver's width unless documented otherwise.

#### Scenario: Absolute value and sign

- **WHEN** `(-7: Int32).abs()` and `(-7: Int32).sign()` are evaluated
- **THEN** they produce `7` and `-1`

#### Scenario: Width-correct bit inspection

- **WHEN** `(255: UInt8).bit_count()` and `(0: UInt8).leading_zeros()` run
- **THEN** they produce `8` and `8` (width-scoped, not 64-bit-scoped)

#### Scenario: Absolute value traps at MIN

- **WHEN** `Int8.MIN.abs()` is evaluated
- **THEN** a controlled overflow error is raised

#### Scenario: Rotation wraps bits within the width

- **WHEN** `(255: UInt8).rotate_left(4)` is evaluated
- **THEN** the result is `255` at `UInt8` width

### Requirement: Deliberate integer overflow policies

Integers SHALL expose `checked_*` (add, sub, mul, div, rem),
`wrapping_*` (add, sub, mul), and `saturating_*` (add, sub, mul) families
at every width.

#### Scenario: Checked operation reports overflow

- **WHEN** `Int8.MAX.checked_add(1)` is evaluated
- **THEN** the result is `Error` carrying an `OverflowError`

#### Scenario: Checked operation succeeds in range

- **WHEN** `(2: Int32).checked_add(3)` is evaluated
- **THEN** the result is `Ok(5)`

#### Scenario: Wrapping addition wraps around

- **WHEN** `UInt8.MAX.wrapping_add(1)` is evaluated
- **THEN** the result is `0`

#### Scenario: Saturating addition clamps

- **WHEN** `Int8.MAX.saturating_add(1)` is evaluated
- **THEN** the result is `Int8.MAX`

#### Scenario: Checked division reports zero division

- **WHEN** `(1: Int32).checked_div(0)` is evaluated
- **THEN** the result is `Error`

### Requirement: Float static members

Each float width SHALL expose `MIN`, `MAX`, `LOWEST`, `EPSILON`,
`POSITIVE_INFINITY`, `NEGATIVE_INFINITY`, and `parse`.

#### Scenario: Infinity constants

- **WHEN** `Float64.POSITIVE_INFINITY` is read
- **THEN** it compares `is_infinite() == true`

#### Scenario: Parse rejects non-numeric text

- **WHEN** `Float64.parse("nope")` is evaluated
- **THEN** the result is `Error` with a `ParseError`

### Requirement: Float value methods

A float value SHALL expose `abs`, `sign`, `min`, `max`, `clamp`,
`is_zero`, `floor`, `ceil`, `round`, `truncate`, `fraction`, `is_finite`,
`is_infinite`, `is_negative`, `pow`, `sqrt`, and `to_string`.

#### Scenario: Rounding family

- **WHEN** `(1.7: Float64).floor()`, `.ceil()`, `.round()`, `.truncate()`
  run
- **THEN** they produce `1.0`, `2.0`, `2.0`, `1.0`

#### Scenario: Classification

- **WHEN** `Float64.POSITIVE_INFINITY.is_finite()` and `.is_infinite()` run
- **THEN** they produce `false` and `true`

#### Scenario: Square root of a negative is a controlled error

- **WHEN** `(-1.0: Float64).sqrt()` is evaluated
- **THEN** a controlled error is raised (no `NaN` is produced)

#### Scenario: Power

- **WHEN** `(2.0: Float64).pow(10)` is evaluated
- **THEN** the result is `1024.0`

