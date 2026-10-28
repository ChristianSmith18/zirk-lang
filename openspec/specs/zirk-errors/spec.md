# zirk-errors Specification

## Purpose
Defines mandatory expected-failure handling, checked and implicit exceptions,
throwable provenance, and the extensible throwable hierarchy.

## Requirements

### Requirement: Expected failure uses Result
The language SHALL define `Result<T,E>` as `Ok(T)` or `Error(E)`. A direct `match` over a Result expression and a value context whose resolved expected type is explicitly `Result<...>` SHALL preserve both variants. In every other value-demanding context, the language SHALL project `Ok(value)` to `value` and `Error(_)` to `null`, producing `T?`, when `T` has a runtime nullable representation. A Result with a payload that has no nullable representation SHALL require explicit matching in value contexts and SHALL produce a compile-time diagnostic otherwise. Projection SHALL discard the error payload; code that needs the error SHALL preserve the Result and inspect it with `match`. In standalone expression statement position, a Result SHALL discard an `Ok` payload and raise an `Error`: a `Throwable` error SHALL be raised unchanged, and any other error SHALL be wrapped in a catchable `NativeError` whose reason is its string representation (or `Result failed` for `Void`). `_ = expression` SHALL deliberately discard either outcome without raising.

The public Result API SHALL retain `map`, `map_error`, `and_then`, `or_else`, and `or_throw` as composition operations. It SHALL NOT expose status, extraction, fallback, or unwrap helpers (`is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, or `unwrap_error`); callers SHALL use direct `match` for inspection or contextual projection for value use.

#### Scenario: Direct match retains both variants
- **WHEN** `match Env.get("APP_MODE") { Ok(value) => value, Error(error) => error.message() }` is evaluated
- **THEN** the match receives `Result<String, EnvironmentError>` and can inspect either variant

#### Scenario: Expected failure is matched
- **WHEN** a function returns `Result<User,LoadError>` and the caller exhaustively matches `Ok` and `Error`
- **THEN** both outcomes are statically handled and the match retains the original variants

#### Scenario: Inferred binding stores the projected nullable value
- **WHEN** `inmut mode = Env.get("APP_MODE");` is evaluated
- **THEN** `mode` has type `String?`, with the successful string or `null`

#### Scenario: Explicit Result annotation preserves the error
- **WHEN** `inmut result: Result<String, EnvironmentError> = Env.get("APP_MODE");` is evaluated
- **THEN** `result` retains `Ok` or the original `Error`

#### Scenario: Failure projects to null and loses its error payload
- **WHEN** a value-demanding context consumes `Error(permission_error)` as `Result<T,E>`
- **THEN** that context receives `null` of type `T?` and cannot inspect `permission_error`

#### Scenario: Null coalescing supplies a concise fallback
- **WHEN** `inmut port = Env.get<Int32>("PORT") ?? 8080;` is evaluated
- **THEN** `port` is the parsed integer on success or `8080` when projection produces `null`

#### Scenario: Successful Result is used as a statement
- **WHEN** a `Result`-producing expression evaluates to `Ok(value)` in statement position
- **THEN** the expression completes and discards `value`

#### Scenario: Failed Result with a Throwable error is used as a statement
- **WHEN** a `Result`-producing expression evaluates to `Error(error)` in statement position and `error` implements `Throwable`
- **THEN** the same throwable is raised and can be caught by ordinary `try/catch`

#### Scenario: Failed Result with another error is used as a statement
- **WHEN** a `Result`-producing expression evaluates to `Error(error)` in statement position and `error` is not a `Throwable`
- **THEN** a catchable `NativeError` is raised with the error text as its reason

#### Scenario: Result failure is handled without raising
- **WHEN** a caller directly matches a failed Result or assigns it to a value explicitly typed as Result
- **THEN** it can inspect the original `Error` value without exception conversion

#### Scenario: Result failure is deliberately discarded
- **WHEN** a caller writes `_ = operation()`
- **THEN** the Result is evaluated and either outcome is discarded without raising

#### Scenario: Redundant Result helper calls are rejected
- **WHEN** source calls `result.ok_or_null()`, `result.unwrap()`, or `result.is_error()`
- **THEN** compilation reports that Result inspection uses `match` and value use relies on contextual projection

### Requirement: Explicit and implicit exceptions remain distinct

The language SHALL require every explicit `throw` to be caught or declared in `throws`, SHALL include declared exception sets in callable compatibility, and SHALL allow typed implicit `RuntimeError` failures to be caught without requiring them in a signature. Result-to-exception conversion SHALL occur only for a Result expression used as a standalone statement, according to the Expected failure uses Result requirement above.

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

### Requirement: Cancellation and timeout are compiler-known catchable failures

The language SHALL define `CancelledError` and `TimeoutError` as concrete,
compiler-known `RuntimeError` subclasses. A concurrent branch SHALL observe
`CancelledError` only at a defined safe point after cancellation is requested,
and a caller MAY catch either type as itself or as any `RuntimeError` ancestor.
Neither SHALL require a `throws` declaration. `CancelledError` SHALL participate
in ordinary `try` / `catch` / `finally` so a cancelled branch can run cleanup. A
branch that does not catch `CancelledError` SHALL fail with it and its owning
scope SHALL treat it as a cleaned, cancelled branch.

#### Scenario: Cancellation is caught without a declaration

- **WHEN** a branch is cancelled and its body catches `CancelledError` around a safe point
- **THEN** the catch runs at the next safe point and the branch can clean up before finishing

#### Scenario: Timeout is caught without a declaration

- **WHEN** `Concurrent.of(op).within(5s)` expires and the caller catches `TimeoutError`
- **THEN** the catch runs after `op` has been cancelled and cleaned

#### Scenario: Uncaught cancellation still ends the task

- **WHEN** a cancelled branch does not catch `CancelledError`
- **THEN** the branch fails with `CancelledError` and its owning scope treats it as a cleaned, cancelled branch

### Requirement: Task stack exhaustion is a catchable failure

The language SHALL define `StackOverflowError` as a concrete, compiler-known
`RuntimeError` subclass, and a function activation that would exceed the running
task's stack limit SHALL raise it rather than corrupting memory or aborting the
process without a diagnostic. A caller MAY catch it as `StackOverflowError` or any
`RuntimeError` ancestor, and it SHALL NOT require a `throws` declaration.

#### Scenario: Unbounded recursion on a task stack is caught

- **WHEN** a task recurses without a base case inside a `try` that catches `StackOverflowError`
- **THEN** the catch runs instead of the process crashing

#### Scenario: Stack overflow propagates uncaught

- **WHEN** a `StackOverflowError` escapes the root task without a matching catch
- **THEN** the process exits with a nonzero status, as for any uncaught exception

### Requirement: Suppressed failures aggregate during structured cancellation

The language SHALL append to the primary throwable's `suppressed()` list any
exception raised by a sibling or by cleanup while a structured scope cancels
siblings after a primary failure, and SHALL preserve the primary throwable's
identity, origin, cause, and stack trace.

#### Scenario: Sibling cleanup throws during propagation

- **WHEN** a child fails, a sibling's cleanup throws while being cancelled, and the primary failure propagates out of the scope
- **THEN** the sibling's exception is in the primary's `suppressed()` list and the primary is otherwise unchanged

### Requirement: A concurrent branch's failure is distinct from `Result.Error`

An unhandled `Throwable` escaping a concurrent branch body SHALL fail that branch
and drive sibling-failure propagation in its scope. A branch that returns
`Result.Error(e)` SHALL be a completed branch carrying that value; the language
SHALL NOT convert it to a branch failure and SHALL NOT convert a branch failure
to a `Result.Error`.

#### Scenario: Returned error is a completed value
- **WHEN** a branch returns `Error(problem)` and a sibling is running
- **THEN** the sibling is not cancelled and the branch's value is `Error(problem)`

#### Scenario: Unhandled throwable fails the branch
- **WHEN** a branch body lets a custom exception escape
- **THEN** the branch fails, active siblings are cancelled and cleaned, and the exception propagates with secondary failures suppressed

### Requirement: Channel closure is not an error

`channel.close()` SHALL never raise. `send` after close SHALL raise the
compiler-known channel-closed failure, while `try_send` after close SHALL return
`SendOutcome.Closed`. A drained closed `receive` SHALL return `None` and a
drained closed `try_receive` SHALL return `ReceiveOutcome.Closed`. A branch
cancelled while suspended in a channel operation SHALL raise `CancelledError`.

#### Scenario: try send after close is typed
- **WHEN** `try_send` is called after a channel closes
- **THEN** it returns `SendOutcome.Closed` without raising

#### Scenario: Blocking send after close raises

- **WHEN** `send` is called after a channel closes
- **THEN** the compiler-known channel-closed failure is raised

#### Scenario: Suspended receiver observes closure

- **WHEN** a branch is suspended in `receive` and another branch closes the channel
- **THEN** the receiver drains remaining values and then observes `None`

### Requirement: Exception-owned cached references follow exception reachability

Cached stack-trace strings and suppressed-exception references SHALL be traced as edges owned by their exception. They SHALL remain valid while the owner is reachable and SHALL be removed when that owner is reclaimed. A stale entry SHALL NOT be returned for a later object allocated at the same address.

#### Scenario: Reachable exception retains its stack trace
- **WHEN** a reachable exception has requested `stack_trace()` and collection runs
- **THEN** the cached string remains valid and repeated access returns the same cached value

#### Scenario: Dead exception cache entry is pruned
- **WHEN** an exception and its cache become unreachable and its address is later reused
- **THEN** lookup for the new exception cannot return the former exception's metadata

### Requirement: Dynamic object failures are catchable
Missing direct property reads, failed runtime contract conformance, and invalid JSON value conversion SHALL raise typed implicit runtime errors that are catchable through ordinary `try`/`catch`. These failures SHALL not require a declared `throws` clause and SHALL not abort the process. Presence-safe lookup SHALL avoid a missing-property error.

#### Scenario: Missing property error is caught
- **WHEN** direct access reads an absent object key inside `try` and catches the missing-property error
- **THEN** the matching catch branch runs

#### Scenario: Contract violation is caught
- **WHEN** runtime conformance finds a missing or incompatible required member
- **THEN** a catchable contract-violation error is raised

#### Scenario: JSON conversion error is caught
- **WHEN** JSON stringification encounters a non-JSON value or a cycle
- **THEN** a catchable JSON conversion error is raised and no partial string is returned

### Requirement: Collection failures have typed, non-partial outcomes
The standard error hierarchy SHALL provide catchable `AllocationError` for a collection growth allocation that cannot be completed and `EmptyCollectionError` for a removal or inspection that requires an element from an empty collection. APIs specified to return `Result<T, EmptyCollectionError>` SHALL return that error without mutation. Collection growth APIs specified to return `Void` SHALL raise `AllocationError` before mutation. Both error types SHALL be catchable as their concrete type or as `RuntimeError`; they SHALL NOT abort the process, return fabricated values, or leave partially updated collection state.

#### Scenario: Empty removal is handled as a typed result
- **WHEN** a caller removes or inspects an item from an empty Deque, PriorityQueue, Queue, or Stack
- **THEN** the operation returns `Error(EmptyCollectionError)` and can be matched without an exception or process abort

#### Scenario: Collection allocation failure is catchable
- **WHEN** growth of a collection fails to reserve required storage
- **THEN** the operation raises `AllocationError` that a caller can catch, and the collection remains unchanged

#### Scenario: Collection errors share the runtime error hierarchy
- **WHEN** code catches `RuntimeError` around an empty operation or failed collection growth
- **THEN** the corresponding `EmptyCollectionError` or `AllocationError` is caught as that base type
