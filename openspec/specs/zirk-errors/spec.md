# zirk-errors Specification

## Purpose
TBD - created by archiving change document-errors-resources-and-permissions. Update Purpose after archive.
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

#### Scenario: Explicit exception escapes undeclared
- **WHEN** a function throws a custom `DatabaseUnavailable` without catching or declaring it
- **THEN** compilation fails and identifies the missing `throws DatabaseUnavailable`

#### Scenario: Division failure is caught without declaration
- **WHEN** a function performs integer division without a `throws` clause and a caller catches `DivisionByZeroError`
- **THEN** the catch is valid because the implicit runtime exception is compiler-known but signature-optional

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

This pass (`fase-4b-excepciones`) narrows three things the fuller requirement above still describes as the target: `suppressed(): List<Error>` is not part of `Error`'s method set yet — `List<T>` is a Phase 7 collection that does not exist yet, and nothing in this pass populates a suppressed list regardless. `stack_trace(): StackTrace` is real and callable, but `StackTrace` carries no frames — a concrete exception class's own override is free to build one any way it likes, commonly `return StackTrace();`, an empty stub. Deep immutability of a thrown instance is not enforced by the checker; a thrown object behaves like any other object reference for now.

#### Scenario: Custom exception

- **WHEN** a class implements every `Throwable` requirement and is thrown from a declared function
- **THEN** it can be caught by its type, `Throwable`, or a compatible requirement ancestor

#### Scenario: Stack trace is an empty stub

- **WHEN** a thrown exception's `stack_trace()` is called
- **THEN** it returns a `StackTrace` value with no frames, not a diagnostic error

