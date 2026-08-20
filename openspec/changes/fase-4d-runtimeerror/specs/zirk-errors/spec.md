## MODIFIED Requirements

### Requirement: Explicit and implicit exceptions remain distinct

The language SHALL require every explicit `throw` to be caught or declared in `throws`, SHALL include declared exception sets in callable compatibility, and SHALL allow typed implicit `RuntimeError` failures to be caught without requiring them in a signature. No implicit conversion SHALL occur between `Result.Error` and an exception.

This pass (`fase-4d-runtimeerror`) makes the implicit-`RuntimeError` half of this requirement real for four of the compiler-known safety checks: division by zero, an out-of-range shift amount, a negative string-repeat count, and a `Float` operation producing `NaN`. Each now throws a concrete, catchable `RuntimeError` subclass (`DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError`) instead of aborting the process unconditionally. A fifth compiler-known check, arithmetic overflow, and invalid-cast failures are unchanged — they still abort via `fatalError` regardless of any enclosing `try`/`catch`, pending a follow-up pass (documented in `design.md`).

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

#### Scenario: `NaN`-producing `Float` operation is caught without declaration

- **WHEN** a `Float` arithmetic expression would produce `NaN`, and a caller catches `FloatNanError` or `Throwable`
- **THEN** the catch runs instead of the process aborting

#### Scenario: Uncaught native failure still aborts

- **WHEN** one of the four native failures above is never caught by any enclosing `try` and propagates out of `main`
- **THEN** the process still exits with a nonzero status, unchanged from before this pass

#### Scenario: Overflow and invalid cast are unchanged

- **WHEN** an arithmetic operation overflows, or a cast targets a runtime type the value does not have
- **THEN** the process aborts unconditionally, exactly as before this pass — neither is a catchable exception yet
