## ADDED Requirements

### Requirement: Unsafe journal rolls back on non-exceptional exits
An `unsafe { ... }` block whose journal has not been committed by `commit { ... }` SHALL roll back all recorded writes when control leaves the block through `return`, `break`, or `continue`.

#### Scenario: return inside unsafe rolls back
- **WHEN** a `return` statement appears inside an `unsafe { ... }` block that has recorded slot or field writes
- **THEN** the compiler emits a `JournalRollback` for that block before the function returns

#### Scenario: break inside unsafe rolls back
- **WHEN** a `break` statement appears inside an `unsafe { ... }` block
- **THEN** the compiler emits a `JournalRollback` for that block before jumping to the loop's `break_to` target

#### Scenario: continue inside unsafe rolls back
- **WHEN** a `continue` statement appears inside an `unsafe { ... }` block
- **THEN** the compiler emits a `JournalRollback` for that block before jumping to the loop's `continue_to` target

#### Scenario: nested try and unsafe cleanup order is lexical
- **WHEN** `try { unsafe { ... return ... } }` or `unsafe { try { ... return ... } }` is compiled
- **THEN** `finally` and `JournalRollback` are emitted in the order they were lexically entered, from innermost to outermost

#### Scenario: commit prevents rollback
- **WHEN** `commit { ... }` is executed inside `unsafe { ... }` before a `return`/`break`/`continue`
- **THEN** the journal is committed and no `JournalRollback` is emitted for that `unsafe` frame
