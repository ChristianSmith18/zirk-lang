## MODIFIED Requirements

### Requirement: Throwable hierarchy is immutable and extensible

The language SHALL define nominal `Error`, `Throwable`, and `RuntimeError` requirements with `message`/`code`/`cause`/`stack` behavior. Custom throwable classes SHALL implement `Throwable`; `Result` error payloads SHALL NOT be required to implement `Error`.

This pass (`fase-4b-excepciones`) narrows three things the fuller requirement above still describes as the target: `suppressed(): List<Error>` is not part of `Error`'s method set yet — `List<T>` is a Phase 7 collection that does not exist yet, and nothing in this pass populates a suppressed list regardless. `stack_trace(): StackTrace` is real and callable, but `StackTrace` carries no frames — a concrete exception class's own override is free to build one any way it likes, commonly `return StackTrace();`, an empty stub. Deep immutability of a thrown instance is not enforced by the checker; a thrown object behaves like any other object reference for now.

#### Scenario: Custom exception

- **WHEN** a class implements every `Throwable` requirement and is thrown from a declared function
- **THEN** it can be caught by its type, `Throwable`, or a compatible requirement ancestor

#### Scenario: Stack trace is an empty stub

- **WHEN** a thrown exception's `stack_trace()` is called
- **THEN** it returns a `StackTrace` value with no frames, not a diagnostic error
