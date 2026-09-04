# zirk-scalars

## MODIFIED Requirements

### Requirement: Checked integer arithmetic at every width
Every arithmetic operation on an integer type, of any width and signedness, including prefix and postfix `++` and `--`, SHALL produce a controlled runtime error if the result is not representable in that width, instead of silently wrapping.

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
