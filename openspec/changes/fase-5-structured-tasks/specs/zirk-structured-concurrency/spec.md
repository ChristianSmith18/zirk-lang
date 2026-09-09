## RENAMED Requirements

- FROM: `### Requirement: Cooperative cancellation and shielding`
- TO: `### Requirement: Cooperative cancellation and uncancelable tasks`

## MODIFIED Requirements

### Requirement: Typed structured tasks
`task` SHALL create a child in the current structured scope and return `Task<T>`, while `await` SHALL produce exactly `T` and scope exit MUST NOT abandon unfinished children. The current structured scope SHALL be the enclosing function body or the block of an enclosing `task { block }`; there SHALL be no `task scope` keyword, and a nested supervision boundary SHALL be expressed with the surrounding scope plus a `Task.combine` or `Task.all` join.

#### Scenario: Scope exits with running child
- **WHEN** control reaches the end of a scope containing an unfinished child task
- **THEN** the scope awaits completion or requests cancellation and awaits cleanup before exiting

#### Scenario: No `task scope` keyword
- **WHEN** source contains `task scope { ... }`
- **THEN** compilation fails, reports that `task scope` was removed, and points at the surrounding function scope plus `Task.combine` / `Task.all` as the replacement

### Requirement: Cooperative cancellation and uncancelable tasks
Cancellation SHALL be cooperative, idempotent, observable at defined safe points, and propagated from parent to child. A `final task` SHALL produce a `Task.Final<T>` that the runtime SHALL exclude from every parent- and sibling-cancellation sweep and SHALL run to completion; `handle.cancel()` on a `Task.Final<T>` SHALL be a compile-time error. Awaiting a `Task.Final<T>` SHALL defer delivery of the awaiting task's own pending cancellation until that await completes, after which the pending `CancelledError` SHALL be delivered at the next safe point. There SHALL be no `cancellation shield` statement.

#### Scenario: Cancelled task enters shielded cleanup
- **WHEN** cancellation is requested while a task awaits a `final task { ... }` cleanup block
- **THEN** the cleanup block completes and `CancelledError` is delivered immediately after the await returns

#### Scenario: A final task is not cancelled by its parent
- **WHEN** a parent scope is cancelled while a `final task` child is still running
- **THEN** the child is not cancelled, the parent waits for the child to finish, and only then does cancellation propagate

#### Scenario: Cancelling a final task is rejected
- **WHEN** source calls `.cancel()` on a `Task.Final<T>` handle
- **THEN** compilation fails and points at the `final task` creation site

#### Scenario: No `cancellation shield` statement
- **WHEN** source contains `cancellation shield { ... }`
- **THEN** compilation fails, reports that `cancellation shield` was removed, and points at `final task { ... }` as the replacement

### Requirement: Task aggregation policies
`Task.all` SHALL cancel remaining tasks after the first unhandled exception, `Task.combine` SHALL apply the same fail-fast policy over a fixed-arity heterogeneous set and produce a tuple of the member element types, `Task.first` SHALL return the first completed task and cancel the rest, and `Task.settled` SHALL allow every task to finish and preserve input order as `TaskSettlement<T>`. A `Task.Final<T>` member SHALL NOT be cancelled by a fail-fast policy; the aggregation SHALL await it before propagating.

#### Scenario: Combine returns a tuple and fails fast
- **WHEN** `await Task.combine(a, b, c)` is evaluated and `b` throws while `a` and `c` are still running
- **THEN** `a` and `c` are cancelled and cleaned, and `b`'s exception propagates with the others suppressed

#### Scenario: Settled aggregation includes failures
- **WHEN** one task fulfills, one throws, and one is cancelled
- **THEN** `Task.settled` returns ordered `Fulfilled`, `Rejected`, and `Cancelled` settlements without sibling failure cancellation

#### Scenario: A final member survives a sibling failure
- **WHEN** `await Task.combine(a, final task b())` is evaluated and `a` throws
- **THEN** `b` keeps running, the combine waits for `b` to finish, and then `a`'s exception propagates

### Requirement: Data-race analysis is enforced before real parallelism exists

The compiler SHALL derive and enforce the `Transfer` and `Share` boundary rules
of this capability at `task` creation and captures, structured-scope results,
`Task.combine` / `Task.all` members, channel send and receive, and `select`
branch values, even though the Phase 5 step 1-3 executor is single-threaded. The
analysis SHALL reject a mutable alias that would remain usable by a parent while
a child can mutate it, and SHALL name transfer, strict immutable sharing,
cloning, or a channel as the resolution.

#### Scenario: Shared mutable alias across a task boundary is rejected

- **WHEN** a task captures a mutable reference that its parent keeps using
- **THEN** compilation fails and names the available resolutions

#### Scenario: Transferred exclusive reference is unusable by the sender

- **WHEN** a parent transfers an exclusive mutable reference into a child task
- **THEN** the parent cannot use that reference again until it returns through a structured result or a channel

## ADDED Requirements

### Requirement: `final task` is a call-site modifier

`final` before `task` SHALL be accepted in exactly the positions `task` is
accepted — `final task expression` and `final task { block }` — and SHALL never
appear in a function signature or a declared return type. The callee SHALL be an
ordinary function; the caller alone SHALL decide that the spawned task is
uncancelable. `final final task` SHALL be idempotent, not an error. A `final task`
SHALL NOT appear inside a reversible unsafe transaction body, where spawning is
already forbidden.

#### Scenario: Final task over a call
- **WHEN** source contains `mut h: Task.Final<Void> = final task audit.write(event);`
- **THEN** the binding type is `Task.Final<Void>` and `audit.write` is unchanged

#### Scenario: Final in a signature is rejected
- **WHEN** a function is declared to return `Task.Final<T>` as its written signature type
- **THEN** compilation fails and states that `final` is a call-site modifier only

### Requirement: A `final task` body must be bounded

A `final task` body SHALL be expected to complete without an unbounded wait. The
compiler SHOULD diagnose a `final task` whose body performs an `await` with no
reachable completion and no timeout. The runtime's unresolvable-wait detector
SHALL abort with a diagnostic if a `final task` leaves the executor with no
runnable work and no armed timer.

#### Scenario: Unbounded final task is diagnosed
- **WHEN** a `final task` body awaits a handle that can never complete
- **THEN** the compiler emits a bounded-cleanup warning at the `final task` site

#### Scenario: Runtime backstop aborts a stuck final task
- **WHEN** at runtime a `final task` is the only suspended task, the ready queue is empty, and no timer is armed
- **THEN** the executor aborts the program with an unresolvable-wait diagnostic naming the stuck task

### Requirement: Timer-driven tasks on `Task`

`Task.sleep(duration)` SHALL suspend the awaiting task for the duration and SHALL
be a cancellation safe point. `Task.after(duration, body)` SHALL run `body` once
after the duration and SHALL be an ordinary scope-owned child task.
`Task.every(duration, body)` SHALL run `body` repeatedly with a fixed delay
between runs until it is cancelled, and its `Task<Void>` SHALL complete only
through cancellation. All three SHALL arm through the runtime timer service,
SHALL reject a negative duration before waiting, and SHALL be owned by the
current structured scope so an un-awaited timer task is cancelled or joined at
scope exit rather than orphaned.

#### Scenario: Sleep yields the executor and resumes after the delay
- **WHEN** a task performs `await Task.sleep(10ms)` while another task is ready
- **THEN** the other task runs during the delay and the sleeping task resumes after the deadline

#### Scenario: Cancelling a sleep raises at the await
- **WHEN** a task is cancelled while suspended in `await Task.sleep(1s)`
- **THEN** the await raises `CancelledError` at that point

#### Scenario: A repeating interval is cancelled by its scope
- **WHEN** a function creates `Task.every(1s, tick)` and returns without awaiting it
- **THEN** the interval task is cancelled as part of scope exit and does not keep running

#### Scenario: Deferred work is not orphaned
- **WHEN** a function creates `Task.after(5s, flush)` and returns after 1 second
- **THEN** the deferred task is cancelled at scope exit rather than firing ownerless later
