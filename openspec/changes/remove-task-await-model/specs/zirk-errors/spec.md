## REMOVED Requirements

### Requirement: Task rejection is distinct from `Result.Error`
**Reason**: phrased in terms of `task` bodies and `Task.settled`.
**Migration**: restated as "a concurrent branch's failure is distinct from `Result.Error`" in `concurrent-blocks-and-timers`, and the settlement rule (`Fulfilled(Error(e))` vs `Rejected`) moves to `Outcome<T>` in `concurrency-completion`.

## MODIFIED Requirements

### Requirement: Cancellation and timeout are compiler-known catchable failures

The language SHALL define `CancelledError` and `TimeoutError` as concrete, compiler-known `RuntimeError` subclasses. A concurrent branch SHALL observe `CancelledError` only at a defined safe point after cancellation is requested, and a caller MAY catch either type as itself or as any `RuntimeError` ancestor. Neither SHALL require a `throws` declaration. `CancelledError` SHALL participate in ordinary `try` / `catch` / `finally` so a cancelled branch can run cleanup. A branch that does not catch `CancelledError` SHALL fail with it and its owning scope SHALL treat it as a cleaned, cancelled branch.

#### Scenario: Cancellation is caught without a declaration

- **WHEN** a branch is cancelled and its body catches `CancelledError` around a safe point
- **THEN** the catch runs at the next safe point and the branch can clean up before finishing

#### Scenario: Timeout is caught without a declaration

- **WHEN** `Concurrent.of(op).within(5s)` expires and the caller catches `TimeoutError`
- **THEN** the catch runs after `op` has been cancelled and cleaned

#### Scenario: Uncaught cancellation still ends the branch

- **WHEN** a cancelled branch does not catch `CancelledError`
- **THEN** the branch fails with `CancelledError` and its owning scope treats it as a cleaned, cancelled branch
