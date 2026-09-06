## MODIFIED Requirements

### Requirement: Explicit and implicit exceptions remain distinct

The language SHALL require every explicit `throw` to be caught or declared in `throws`, SHALL include declared exception sets in callable compatibility, and SHALL allow typed implicit `RuntimeError` failures to be caught without requiring them in a signature. No implicit conversion SHALL occur between `Result.Error` and an exception.

This pass (`native-runtime-errors-catcheable`) makes the implicit-`RuntimeError` half of this requirement real for several compiler-known safety checks: division by zero, an out-of-range shift amount, a negative string-repeat count, and a `BinaryFloat` operation producing `NaN`. Each throws a concrete, catchable `RuntimeError` subclass (`DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError`) instead of aborting the process unconditionally. `FloatNanError` applies to the IEEE 754 binary `BinaryFloat` family only; the exact-decimal `Float` type has no `NaN`. A `Float` operation whose exact result exceeds the 128-bit coefficient budget raises `ArithmeticOverflowError`, and a `Float` division or modulo by zero raises `DivisionByZeroError`. Arithmetic overflow and invalid-cast failures still abort via `fatalError` regardless of any enclosing `try`/`catch`, pending a follow-up pass.

#### Scenario: Explicit exception escapes undeclared

- **WHEN** a function throws a custom `DatabaseUnavailable` without catching or declaring it
- **THEN** compilation fails and identifies the missing `throws DatabaseUnavailable`

#### Scenario: Division failure is caught without declaration

- **WHEN** a function performs integer division without a `throws` clause and a caller catches `DivisionByZeroError`
- **THEN** the catch runs — the division does not abort the process, and `error.message()` describes the division by zero

#### Scenario: Invalid shift is caught without declaration

- **WHEN** a shift amount is negative or at least the operand's bit width, and a caller catches `InvalidShiftError` or `RuntimeError`
- **THEN** the catch runs instead of the process aborting

#### Scenario: Invalid repeat count is caught without declaration

- **WHEN** a `String` is repeated a negative number of times, and a caller catches `InvalidRepeatError`
- **THEN** the catch runs instead of the process aborting

#### Scenario: `NaN`-producing `BinaryFloat` operation is caught without declaration

- **WHEN** a `BinaryFloat` arithmetic expression would produce `NaN`, and a caller catches `FloatNanError` or `Throwable`
- **THEN** the catch runs instead of the process aborting

#### Scenario: Exact-decimal division by zero is caught without declaration

- **WHEN** a `Float` value is divided by zero, and a caller catches `DivisionByZeroError`
- **THEN** the catch runs instead of the process aborting

#### Scenario: Exact-decimal coefficient overflow is a controlled failure

- **WHEN** a `Float` multiplication produces a value that cannot be represented within the 128-bit coefficient budget after rounding
- **THEN** the program raises `ArithmeticOverflowError` at that operation

#### Scenario: Uncaught native failure still aborts

- **WHEN** one of the catchable native failures above is never caught by any enclosing `try` and propagates out of `main`
- **THEN** the process still exits with a nonzero status

#### Scenario: Overflow and invalid cast are unchanged

- **WHEN** an arithmetic operation overflows an integer width, or a cast targets a runtime type the value does not have
- **THEN** the process aborts unconditionally — neither is a catchable exception yet
