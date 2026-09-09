## MODIFIED Requirements

### Requirement: Cancellation and timeout are compiler-known catchable failures

The language SHALL define `CancelledError` and `TimeoutError` as concrete,
compiler-known `RuntimeError` subclasses. A task SHALL observe `CancelledError`
only at a defined safe point after cancellation is requested, and a caller MAY
catch either type as itself or as any `RuntimeError` ancestor. Neither SHALL
require a `throws` declaration. `CancelledError` SHALL participate in ordinary
`try` / `catch` / `finally` so a cancelled task can run cleanup. A
`CancelledError` that becomes pending while the task is inside a shielded await
(an `await` of a `Task.Final<T>`) SHALL be held, not discarded, and SHALL be
delivered at the first safe point after that await returns. A `Task.Final<T>`
task SHALL never observe `CancelledError` from a parent- or sibling-cancellation
sweep.

#### Scenario: Cancellation is caught without a declaration

- **WHEN** a task is cancelled and its body catches `CancelledError` around an `await`
- **THEN** the catch runs at the next safe point and the task can clean up before finishing

#### Scenario: Timeout is caught without a declaration

- **WHEN** `await operation timeout 5s` expires and the caller catches `TimeoutError`
- **THEN** the catch runs after the operation has been cancelled and cleaned

#### Scenario: Shielded cancellation is delivered afterward

- **WHEN** cancellation is requested while a task is awaiting a `final task { ... }` cleanup block
- **THEN** the cleanup block completes and `CancelledError` is delivered at the first safe point after the await returns

#### Scenario: A final task ignores sweep cancellation

- **WHEN** a parent scope is cancelled while a `final task` child is running
- **THEN** the child never observes `CancelledError` from the sweep and runs to completion

#### Scenario: Uncaught cancellation still ends the task

- **WHEN** a cancelled task does not catch `CancelledError`
- **THEN** the task fails with `CancelledError` and its owning scope treats it as a cleaned, cancelled child
