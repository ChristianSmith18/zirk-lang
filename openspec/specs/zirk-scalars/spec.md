# zirk-scalars Specification

## Purpose
Defines integer and floating families, Boolean and grapheme Char semantics,
checked arithmetic, contextual conversion, and scalar formatting contracts.
## Requirements
### Requirement: Complete family of integer widths

The type system SHALL recognize `Int8`, `Int16`, `Int32`, `Int64`, `Int128` signed and `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128` unsigned, with `Int`/`Integer` as an alias for `Int32` and `UInt`/`UInteger` as an alias for `UInt32`.

#### Scenario: Literal in an explicit width
- **WHEN** a variable is annotated `Int8` and an integer literal within its range is assigned to it
- **THEN** the check succeeds and the value is represented as 8 signed bits

#### Scenario: Literal out of range for its width
- **WHEN** a literal exceeds the representable range of the annotated width
- **THEN** a diagnostic is emitted at compile time, without waiting for runtime

#### Scenario: Unsigned alias resolves to `UInt32`
- **WHEN** a variable is annotated `UInt` or `UInteger` and assigned `42`
- **THEN** the check succeeds and the value is represented as `UInt32`, with the same range, overflow rules and member surface

### Requirement: Checked integer arithmetic at every width

Every arithmetic operation on a numeric type, of any width and signedness, including prefix and postfix `++` and `--`, SHALL produce a controlled runtime error if the result is not representable in the operation's result type, instead of silently wrapping. When operands have different widths or families, the operation SHALL be performed in the smallest common numeric type that can represent every value of both operand types, with each operand widened losslessly to that common type. If no such common type exists, the program SHALL be rejected at compile time.

#### Scenario: Overflow in a narrow width
- **WHEN** an addition on `Int8` produces a result greater than 127
- **THEN** the program terminates with a controlled error that identifies the operation

#### Scenario: Unsigned arithmetic below zero
- **WHEN** a subtraction on `UInt32` would produce a negative result
- **THEN** the program terminates with a controlled error, not with a value wrapped to the top of the range

#### Scenario: Postfix increment overflows
- **WHEN** an `Int32` variable holds `2147483647` and `x = i++` is evaluated
- **THEN** the program terminates with a controlled `ArithmeticOverflowError` instead of wrapping to `-2147483648`

#### Scenario: Prefix decrement underflows
- **WHEN** a `UInt8` variable holds `0` and `--i` is evaluated
- **THEN** the program terminates with a controlled `ArithmeticOverflowError` instead of wrapping to `255`

#### Scenario: Mixed-width integer addition
- **WHEN** `Int8` and `Int32` values are added
- **THEN** the `Int8` operand is widened to `Int32`, the result is `Int32`, and overflow is checked at `Int32`

#### Scenario: Mixed-family addition chooses a common float type
- **WHEN** an `Int32` and a `Float64` are added
- **THEN** the `Int32` operand is widened to `Float64` and the result is `Float64`

#### Scenario: Unsafe mixed-family addition is rejected
- **WHEN** an `Int32` and a `Float16` are added
- **THEN** compilation fails because `Float16` cannot represent every `Int32` value

#### Scenario: Postfix increment on a narrow signed integer
- **WHEN** an `Int8` variable holds `127` and `i++` is evaluated
- **THEN** the program terminates with a controlled `ArithmeticOverflowError` instead of wrapping to `-128`

### Requirement: Conversion between integer widths

The checker SHALL allow implicit conversion from one numeric type to another when the destination can represent every value of the source type without loss, and SHALL require an explicit `as` cast for narrowing or cross-family conversions that the compiler cannot prove are safe at compile time. An integer literal SHALL adopt the width of its expected type when the value fits. An integer value or literal SHALL convert implicitly and exactly to `Decimal`, and a `Decimal` literal with zero fractional part SHALL become an integer for an integer target. Conversion between `Decimal` and any `Float` width SHALL require an explicit cast or constructor in both directions.

#### Scenario: Unambiguous implicit widening

- **WHEN** an `Int8` is passed where `Int32` is expected
- **THEN** the conversion is implicit and no cast is required

#### Scenario: Integer widens to exact `Decimal` implicitly

- **WHEN** an `Int32` is passed where `Decimal` is expected
- **THEN** the conversion is implicit and the value is exact at scale 0

#### Scenario: Literal that does not fit is rejected

- **WHEN** `mut a: Int8 = 1000;` is written
- **THEN** a compile-time diagnostic is emitted because `1000` does not fit in `Int8`

#### Scenario: Exact and binary floats do not mix implicitly

- **WHEN** a `Decimal` value is used where `Float64` is expected without a cast
- **THEN** a diagnostic is emitted naming the explicit conversion required

### Requirement: Fractional scalar family names

The type system SHALL recognize `Decimal` and its alias `Dec` as the exact base-ten decimal scalar. It SHALL recognize `Float16`, `Float32`, `Float64`, and `Float128` as the IEEE 754 binary family, with `Float` as an alias for `Float64`. The spellings `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`, and `BinaryFloat` SHALL NOT resolve. The type `Decimal` SHALL have neither `NaN` nor `Infinity`; a `Decimal` operation that would require either SHALL be a controlled runtime error. For a `Float` value, `NaN` SHALL NOT be valid, but infinities are valid; an operation that would produce `NaN` under IEEE 754 SHALL be a controlled runtime error.

#### Scenario: Exact-decimal division by zero

- **WHEN** a `Decimal` is divided by `0.0`
- **THEN** the program raises a controlled `DivisionByZeroError` at that operation

#### Scenario: Binary division by zero

- **WHEN** a non-zero `Float64` is divided by `0.0f` or `0.0f64`
- **THEN** the result is infinite, not an error

#### Scenario: Binary indeterminate operation

- **WHEN** a `Float` operation would produce `NaN` under standard IEEE 754 semantics (for example, `0.0f / 0.0f`)
- **THEN** the program terminates with a controlled error at the point of that operation; it does not propagate a `NaN` value

#### Scenario: Fractional literal without context

- **WHEN** a fractional literal appears without an annotation or context that fixes its type
- **THEN** its type is `Decimal` (exact base-ten decimal)

#### Scenario: Former binary spelling is redirected

- **WHEN** an annotation names `BinaryFloat64`
- **THEN** a diagnostic states that the binary type is now `Float64` and the exact base-ten type is `Decimal`

#### Scenario: Former exact spelling is redirected

- **WHEN** an annotation names `Float` in a context where `Decimal` is expected after the rename
- **THEN** a diagnostic states that the exact base-ten type is now `Decimal` and the binary type is `FloatN`

### Requirement: `Char` as a Unicode grapheme

The type system SHALL recognize `Char` as exactly one extended Unicode grapheme, even when it spans more than one code point.

#### Scenario: Literal of a simple character
- **WHEN** a `Char` literal contains a single ASCII code point
- **THEN** its value is that grapheme

#### Scenario: Literal of a composite grapheme
- **WHEN** a `Char` literal contains an extended grapheme composed of several code points (for example, a base with combining marks)
- **THEN** the check succeeds and the value retains the complete grapheme as a single unit

#### Scenario: Iterating a `String` by grapheme
- **WHEN** a `for ... in` traverses a `String`
- **THEN** each bound element is a `Char`, one per grapheme, not per byte or per code point

### Requirement: Deep contextual conversion

An explicit constructor of a scalar type (`Decimal(expr)`, `Float(expr)`, `String(expr)`, etc.) SHALL establish a conversion domain for the compatible operator tree it directly contains, converting each operand before the operation is evaluated. The context SHALL NOT mutate the original operands or cross into the body of a function called within the expression.

#### Scenario: Division converted before operating
- **WHEN** `Decimal(3 / 4)` is written
- **THEN** the result is `0.75`, not `0` truncated and then converted

#### Scenario: The context does not cross a function call
- **WHEN** an expression within a contextual constructor calls a function that internally performs an integer division
- **THEN** that internal division does not adopt the context of the outer constructor

### Requirement: Bitwise and shift operators

The checker SHALL accept `&`, `|`, `^`, `~`, `<<`, `>>` on integer operands, of any width and signedness compatible with each other, at the precedence levels that `ZIRK_LANGUAGE_SPEC.md` fixes for them.

#### Scenario: Bitwise operation between different widths
- **WHEN** `&` is applied between an `Int8` and an `Int32` without an explicit conversion
- **THEN** the same type diagnostic as any other operation between incompatible widths is emitted

#### Scenario: Shift by a negative amount
- **WHEN** the right operand of `<<` or `>>` is negative at runtime
- **THEN** the program terminates with a controlled error

### Requirement: String repetition count accepts any integer width

The `String * n` operator SHALL accept an integer of any width for `n`, widening the count to `Int32` at the call site when the value fits.

#### Scenario: Repeating with an unsigned count
- **WHEN** `"x" * (3 as UInt32)` is evaluated
- **THEN** the count is converted to `Int32` and the result is the repeated string

#### Scenario: Repeating with a signed narrow count
- **WHEN** `"x" * (3 as Int8)` is evaluated
- **THEN** the count is converted to `Int32` and the result is the repeated string

