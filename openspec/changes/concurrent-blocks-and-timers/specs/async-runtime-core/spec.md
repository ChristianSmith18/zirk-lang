## MODIFIED Requirements

### Requirement: Single-threaded cooperative executor

The runtime SHALL provide one executor that runs on a single operating-system
thread and schedules branches cooperatively. A branch SHALL yield control only at
a defined safe point (`Timer.sleep`, a blocking channel operation, a timer wait,
or an explicit cancellation check); the executor SHALL NOT preempt a running
branch at an arbitrary instruction. The executor SHALL service ready branches
from a first-in-first-out ready queue and SHALL check timer deadlines once per
scheduling turn. The executor SHALL NOT expose branch priority as a
user-controlled value and SHALL prevent indefinite starvation of a ready branch.

#### Scenario: Branch runs until it suspends
- **WHEN** a branch performs a long computation with no safe point
- **THEN** no other branch is scheduled until it reaches a safe point or completes

#### Scenario: Ready branches are served fairly
- **WHEN** several branches are ready at the same time
- **THEN** the executor runs them in first-in-first-out order and each makes progress

#### Scenario: Executor drives the program to completion
- **WHEN** `main` starts branches and reaches the end of its body
- **THEN** the executor runs until `main`'s body and every owned branch have finished (or ambient timers cancelled) before the process exits

### Requirement: Timer service backs delayed and timed operations

The runtime SHALL provide a monotonic timer service that the executor consults
each scheduling turn. `Timer.sleep`, `Timer.after`, `Timer.every`, and (in a
later change) `Concurrent.of(...).within(d)` SHALL be implemented through this
service. `Timer.sleep` SHALL suspend the calling branch until its deadline;
`Timer.after` SHALL run its thunk once at its deadline on a branch of the owning
scope; `Timer.every` SHALL re-arm a deadline after each thunk run until cancelled.
A negative duration SHALL be rejected as a controlled error before any waiting
begins.

#### Scenario: Timer fires and unblocks its waiter
- **WHEN** a branch waits on a timer and the deadline passes
- **THEN** the executor unblocks that branch on the next scheduling turn

#### Scenario: Nearest deadline bounds an idle executor
- **WHEN** no branch is ready but a timer is armed
- **THEN** the executor waits at most until the nearest deadline before running again

#### Scenario: Timer.every re-arms
- **WHEN** `Timer.every(3ms, tick)` runs and is not yet cancelled
- **THEN** the runtime arms a fresh deadline after each `tick` returns

### Requirement: Executor owns the root scope of `main`

The runtime entry SHALL run `main`'s body as the root branch of an implicit
`concurrent` scope and SHALL NOT return the program's exit status until that
branch and every branch it owns have completed, or been cancelled and cleaned
(ambient timers are cancelled on normal exit). An exception that escapes the root
branch SHALL cause a nonzero exit status.

#### Scenario: Program waits for spawned children
- **WHEN** `main` spawns a branch and reaches the end of its body while the branch runs
- **THEN** the process does not exit until the branch has finished or been cancelled and cleaned

#### Scenario: Uncaught failure in the root branch
- **WHEN** an exception escapes `main`'s body after children are cleaned
- **THEN** the process exits with a nonzero status

## ADDED Requirements

### Requirement: Scope join runtime

The runtime SHALL track, per `concurrent` scope, its registered branches, a
cancellation flag, a primary-failure slot, and a suppressed-failure list.
`ScopeExit` SHALL wait for every registered branch to reach a terminal state; on
the first unhandled branch exception it SHALL request cancellation of the other
branches, wait for each to reach `cleanup_state == Done`, and resolve the scope
with the primary exception and the suppressed list. Ambient timer branches SHALL
be cancelled, not waited for, on `ScopeExit`.

#### Scenario: scope waits for all branches
- **WHEN** a scope has three registered branches and control reaches `ScopeExit`
- **THEN** the scope does not continue until all three are terminal

#### Scenario: first failure cancels the rest with suppression
- **WHEN** one branch throws while two siblings run
- **THEN** the siblings are cancelled and cleaned and the scope resolves with the thrown exception plus any sibling failure suppressed
