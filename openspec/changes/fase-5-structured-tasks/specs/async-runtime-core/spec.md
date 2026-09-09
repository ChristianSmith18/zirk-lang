## MODIFIED Requirements

### Requirement: Aggregation runtime for task collections

The runtime SHALL provide the backing operations for `Task.all`, `Task.combine`,
`Task.first`, and `Task.settled`. `Task.all` SHALL preserve input order and
SHALL, on the first unhandled failure, cancel the unfinished tasks, await their
cleanup, and propagate the primary failure with the others suppressed.
`Task.combine` SHALL apply the same fail-fast policy over a fixed-arity
heterogeneous set and resolve with a tuple of the member results in argument
order. `Task.first` SHALL resolve with the first task to complete and SHALL
cancel and clean the remainder. `Task.settled` SHALL let every task finish, SHALL
preserve input order, and SHALL NOT cancel a task merely because another task
rejected. A member whose control block is marked uncancelable SHALL NOT be
cancelled by a fail-fast policy; the aggregation SHALL await it before resolving.

#### Scenario: All-aggregation cancels on first failure

- **WHEN** one task in a `Task.all` throws an unhandled exception while siblings are still running
- **THEN** the siblings are cancelled and cleaned before the exception propagates, and their later failures are attached as suppressed

#### Scenario: Combine resolves with a tuple

- **WHEN** a `Task.combine` over three tasks completes with all members fulfilled
- **THEN** it resolves with a three-element tuple in argument order

#### Scenario: Combine keeps an uncancelable member running

- **WHEN** a `Task.combine` has one ordinary member that throws and one uncancelable member still running
- **THEN** the uncancelable member is not cancelled, the aggregation waits for it, and then the exception propagates

#### Scenario: Settled-aggregation lets every task finish

- **WHEN** a `Task.settled` input set has one task that fulfills, one that throws, and one that is cancelled
- **THEN** the result lists ordered `Fulfilled`, `Rejected`, and `Cancelled` settlements and no task was cancelled because of a sibling's rejection

### Requirement: Timer service backs delayed and timed operations

The runtime SHALL provide a monotonic timer service that the executor consults
each scheduling turn. `after duration` in `select`, `await operation timeout
duration`, and the `Task.sleep` / `Task.after` / `Task.every` surface SHALL be
implemented through this service. `Task.sleep` SHALL suspend the running task
until its deadline; `Task.after` SHALL run its body once at its deadline as a
scope-owned child; `Task.every` SHALL re-arm a deadline after each body run until
cancelled. A negative duration SHALL be rejected as a controlled error before any
waiting begins.

#### Scenario: Timer fires and unblocks its waiter

- **WHEN** a task waits on a timer and the deadline passes
- **THEN** the executor unblocks that task on the next scheduling turn

#### Scenario: Nearest deadline bounds an idle executor

- **WHEN** no task is ready but a timer is armed
- **THEN** the executor waits at most until the nearest deadline before running again

#### Scenario: `Task.sleep` is backed by the timer service

- **WHEN** a task calls `await Task.sleep(50ms)`
- **THEN** the runtime arms one timer deadline and resumes the task when it expires, running other ready tasks in the meantime

## ADDED Requirements

### Requirement: Uncancelable tasks and cancellation sweeps

The task control block SHALL carry an `uncancelable` marker, set at spawn by the
uncancelable-spawn entry point and clear otherwise. A parent- or
sibling-cancellation sweep SHALL skip every task whose `uncancelable` marker is
set. `request_cancel` SHALL be idempotent: it SHALL record the reason, and if the
target is suspended at a cancellable wait it SHALL make the target ready so it
observes the cancellation at its next safe point. A safe point SHALL raise
`CancelledError` only when cancellation is pending, the task's shield depth is
zero, and the task is not uncancelable. A shielded await SHALL raise the running
task's shield depth for the duration of the wait and lower it on resume or
unwind.

#### Scenario: Sweep skips an uncancelable task

- **WHEN** a scope is cancelled and it has one ordinary child and one uncancelable child
- **THEN** the ordinary child receives the cancellation and the uncancelable child does not

#### Scenario: Shielded await holds a pending cancellation

- **WHEN** a task is cancelled while suspended in a shielded await
- **THEN** the shielded await completes normally and the task observes `CancelledError` at the next safe point after it returns

#### Scenario: Cancellation request is idempotent

- **WHEN** `request_cancel` is called twice on the same task before it reaches a safe point
- **THEN** the task observes one `CancelledError` carrying the first reason and the second call has no additional effect
