# zirk-errors Specification

## Purpose
Defines mandatory expected-failure handling, checked and implicit exceptions,
throwable provenance, and the extensible throwable hierarchy.
## Requirements
### Requirement: Expected failure uses mandatory Result handling
The language SHALL define `Result<T,E>` as `Ok(T)` or `Error(E)`, SHALL reject a discarded `Result`, and SHALL permit intentional discard only through explicit `_ = expression`. `Result` SHALL provide the accepted inspection, nullable extraction, fallback, mapping, chaining, unwrap, and exception-conversion API. Using the wrong `unwrap` variant SHALL invoke `fatalError`.

#### Scenario: Expected failure is matched
- **WHEN** a function returns `Result<User,LoadError>` and the caller exhaustively matches `Ok` and `Error`
- **THEN** both outcomes are statically handled

#### Scenario: Result is ignored
- **WHEN** a `Result`-producing expression appears as an unused statement
- **THEN** compilation fails and suggests handling it or writing `_ = expression`

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

### Requirement: Patterned exception handling preserves provenance
The language SHALL support pattern-shaped `catch Type(binding)`, subtype/variant patterns without guards, exhaustive handling of declared exceptions, `finally` on every exit, exact `throw;` rethrow, causes, suppressed failures, and lazy structured stack traces. It SHALL reject a control transfer written directly inside `finally` when it could replace the active outcome.

#### Scenario: Exact rethrow
- **WHEN** a catch logs an exception and executes `throw;`
- **THEN** the same throwable identity, original throw point, cause, suppressed list, and stack trace continue propagating

#### Scenario: General handler precedes specific handler
- **WHEN** `catch Throwable(error)` appears before `catch NetworkError(error)`
- **THEN** the specific handler is rejected as unreachable

### Requirement: Throwable hierarchy is immutable and extensible

The language SHALL define nominal `Error`, `Throwable`, and `RuntimeError` requirements with `message`/`code`/`cause`/`stack` behavior. Custom throwable classes SHALL implement `Throwable`; `Result` error payloads SHALL NOT be required to implement `Error`.

#### Scenario: Custom exception

- **WHEN** a class implements every `Throwable` requirement and is thrown from a declared function
- **THEN** it can be caught by its type, `Throwable`, or a compatible requirement ancestor

### Requirement: Arithmetic overflow is catchable
The language SHALL throw `ArithmeticOverflowError` when an integer arithmetic operation overflows, and the caller MAY catch it as `ArithmeticOverflowError` or any `RuntimeError` ancestor.

#### Scenario: Signed overflow is caught
- **WHEN** a function evaluates `Int32.MAX + 1` inside a `try` that catches `ArithmeticOverflowError`
- **THEN** the catch block runs instead of the process aborting

#### Scenario: Unsigned overflow is caught
- **WHEN** a function evaluates `UInt32.MAX + 1u32` inside a `try` that catches `ArithmeticOverflowError`
- **THEN** the catch block runs

#### Scenario: Overflow propagates uncaught
- **WHEN** an overflow escapes `main` without a matching catch
- **THEN** the process exits with a nonzero status, as for any uncaught exception

### Requirement: Invalid cast is catchable
The language SHALL throw `InvalidCastError` when a runtime `as` or `<T>` cast targets a type the value does not actually have, and the caller MAY catch it as `InvalidCastError` or any `RuntimeError` ancestor.

#### Scenario: Invalid class cast is caught
- **WHEN** a value is cast to a `class` it does not implement and the call site catches `InvalidCastError`
- **THEN** the catch block runs and the process continues

#### Scenario: Invalid numeric cast is caught
- **WHEN** a value is cast to a numeric type the value does not fit and the call site catches `InvalidCastError`
- **THEN** the catch block runs

### Requirement: Throwable carries suppressed failures
A `Throwable` object SHALL maintain a list of suppressed failures collected during cleanup unwinding, and the list SHALL be accessible through a `suppressed()` method.

#### Scenario: Close failure is suppressed during exception propagation
- **WHEN** a `finally` or resource close throws an exception while another exception is already propagating
- **THEN** the close exception is appended to the original exception's suppressed list and the original exception continues to propagate

### Requirement: Stack traces are lazily materialized
A `Throwable` SHALL record the throw site and call stack at throw time, and SHALL materialize the human-readable stack trace only when `error.stack()` is first read.

#### Scenario: Stack trace is inspected
- **WHEN** code calls `error.stack()` on a caught exception
- **THEN** a formatted multi-line stack trace is returned

#### Scenario: Stack trace is never read
- **WHEN** an exception is caught and handled without reading `.stack()`
- **THEN** no expensive string building occurs

### Requirement: Thrown objects are deeply immutable
A `Throwable` and every object reachable from it SHALL be immutable after construction; any write through a reference derived from the throwable SHALL be rejected.

#### Scenario: Mutating a caught exception is rejected
- **WHEN** code writes to a field of a caught exception object
- **THEN** compilation fails with a deep-immutability diagnostic

