## ADDED Requirements

### Requirement: Cancellation and timeout are compiler-known catchable failures

The language SHALL define `CancelledError` and `TimeoutError` as concrete,
compiler-known `RuntimeError` subclasses. A task SHALL observe `CancelledError`
only at a defined safe point after cancellation is requested, and a caller MAY
catch either type as itself or as any `RuntimeError` ancestor. Neither SHALL
require a `throws` declaration. `CancelledError` SHALL participate in ordinary
`try` / `catch` / `finally` so a cancelled task can run cleanup. A
`CancelledError` that becomes pending while a `cancellation shield` is active
SHALL be delivered immediately after the shield rather than discarded.

#### Scenario: Cancellation is caught without a declaration

- **WHEN** a task is cancelled and its body catches `CancelledError` around an `await`
- **THEN** the catch runs at the next safe point and the task can clean up before finishing

#### Scenario: Timeout is caught without a declaration

- **WHEN** `await operation timeout 5s` expires and the caller catches `TimeoutError`
- **THEN** the catch runs after the operation has been cancelled and cleaned

#### Scenario: Shielded cancellation is delivered afterward

- **WHEN** cancellation is requested while a task is inside a `cancellation shield` block
- **THEN** the block completes and `CancelledError` is delivered at the first safe point after the block

#### Scenario: Uncaught cancellation still ends the task

- **WHEN** a cancelled task does not catch `CancelledError`
- **THEN** the task fails with `CancelledError` and its owning scope treats it as a cleaned, cancelled child

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

### Requirement: Task rejection is distinct from `Result.Error`

An unhandled `Throwable` escaping a task body SHALL reject that task and drive
sibling-failure propagation. A task that returns `Result.Error(e)` SHALL be a
fulfilled task carrying that value; the language SHALL NOT convert it to a task
rejection and SHALL NOT convert a task rejection to a `Result.Error`. In a
`Task.settled` result, a returned `Error(e)` SHALL appear as `Fulfilled(Error(e))`
and only an unhandled throwable SHALL appear as `Rejected`.

#### Scenario: Returned error is a fulfilled value

- **WHEN** a task returns `Error(problem)` and a sibling is running
- **THEN** the sibling is not cancelled and `Task.settled` records `Fulfilled(Error(problem))`

#### Scenario: Unhandled throwable rejects the task

- **WHEN** a task body lets a custom exception escape
- **THEN** the task is rejected, active siblings are cancelled and cleaned, and the exception propagates with secondary failures suppressed

### Requirement: Suppressed failures aggregate during structured cancellation

The language SHALL append to the primary throwable's `suppressed()` list any
exception raised by a sibling or by cleanup while a structured scope cancels
siblings after a primary failure, and SHALL preserve the primary throwable's
identity, origin, cause, and stack trace.

#### Scenario: Sibling cleanup throws during propagation

- **WHEN** a child fails, a sibling's cleanup throws while being cancelled, and the primary failure propagates out of the scope
- **THEN** the sibling's exception is in the primary's `suppressed()` list and the primary is otherwise unchanged
