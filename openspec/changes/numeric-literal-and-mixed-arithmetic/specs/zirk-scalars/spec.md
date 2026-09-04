# zirk-scalars

## MODIFIED Requirements

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

The checker SHALL allow implicit conversion from one numeric type to another when the destination can represent every value of the source type without loss, and SHALL require an explicit `as` cast for narrowing or cross-family conversions that the compiler cannot prove are safe at compile time. Integer and float literals SHALL adopt the width of their expected type when the value fits, including `1` becoming `1.0` for a `Float` target and `1.0` with zero fractional part becoming `1` for an integer target.

#### Scenario: Unambiguous implicit widening
- **WHEN** an `Int8` is passed where `Int32` is expected
- **THEN** the check succeeds without a written conversion

#### Scenario: Narrowing without an explicit conversion
- **WHEN** an `Int32` is assigned to an `Int8` variable without an explicit conversion
- **THEN** a type diagnostic is emitted unless the compiler can prove the value fits in `Int8`

#### Scenario: Change of signedness without an explicit conversion
- **WHEN** an `Int32` is assigned to a `UInt32` variable without an explicit conversion
- **THEN** a type diagnostic is emitted unless the compiler can prove the value is non-negative

#### Scenario: Integer literal infers its destination width
- **WHEN** `mut a: Int8 = 1;` is written
- **THEN** the literal `1` is typed as `Int8` and the declaration compiles

#### Scenario: Float literal infers its destination width
- **WHEN** `mut a: Float16 = 1.0;` is written
- **THEN** the literal `1.0` is typed as `Float16` and the declaration compiles

#### Scenario: Integer literal becomes float when assigned to float
- **WHEN** `mut a: Float = 1;` is written
- **THEN** the literal `1` is converted to `1.0` and the declaration compiles

#### Scenario: Float literal becomes integer when it has no fractional part
- **WHEN** `mut a: Int32 = 1.0;` is written
- **THEN** the literal `1.0` is converted to `1` and the declaration compiles

#### Scenario: Out-of-range literal is rejected at compile time
- **WHEN** `mut a: Int8 = 1000;` is written
- **THEN** a compile-time diagnostic is emitted because `1000` does not fit in `Int8`

## ADDED Requirements

### Requirement: String repetition count accepts any integer width

The `String * n` operator SHALL accept an integer of any width for `n`, widening the count to `Int32` at the call site when the value fits.

#### Scenario: Repeating with an unsigned count
- **WHEN** `"x" * (3 as UInt32)` is evaluated
- **THEN** the count is converted to `Int32` and the result is the repeated string

#### Scenario: Repeating with a signed narrow count
- **WHEN** `"x" * (3 as Int8)` is evaluated
- **THEN** the count is converted to `Int32` and the result is the repeated string
