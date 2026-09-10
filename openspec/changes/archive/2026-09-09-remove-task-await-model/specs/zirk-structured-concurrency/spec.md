## REMOVED Requirements

### Requirement: Task results have one consumer

**Reason**: The `task` / `await` surface is removed. There is no `Task<T>` handle
and no `await` operator.

**Migration**: A concurrent branch is a named binding inside `concurrent { }`
(`inmut x = f()`) or a `spawn` with a `Job<T>` handle; `job.wait()` collects a
`Job` result and a second `wait()` is a compile-time error. Defined by the
`concurrent-blocks-and-timers` change.

### Requirement: Typed structured tasks

**Reason**: `task` creates no language-level value any more.

**Migration**: `concurrent { }` is the structured scope; `spawn` adds a dynamic
branch. Both defined by `concurrent-blocks-and-timers`.

### Requirement: Structured timeout

**Reason**: `await operation timeout duration` is removed with `await`.

**Migration**: `Concurrent.of(fn).within(duration)` — defined by
`concurrency-completion`.

### Requirement: Task aggregation policies

**Reason**: `Task.all` / `Task.first` / `Task.settled` are removed with `Task<T>`.

**Migration**: `concurrent { }` (wait-all with names), `Concurrent.each` /
`Concurrent.each_settled` (over a collection), `Concurrent.of(...).first()` /
`.settled()` — defined by `concurrent-blocks-and-timers` and
`concurrency-completion`.

### Requirement: Result and settlement separation

**Reason**: `TaskSettlement<T>` is removed.

**Migration**: `Concurrent.each_settled` returns `List<Outcome<T>>` with
`Fulfilled` / `Rejected` / `Cancelled`; a returned `Result.Error` stays a normal
value inside `Fulfilled`. Defined by `concurrency-completion`.

### Requirement: Fair selection

**Reason**: `select` is removed.

**Migration**: multi-source waiting is expressed with `Concurrent.of(...).first()`
over channel-receive and timer thunks, plus `Channel.try_receive`. Defined by
`typed-channels` and `concurrency-completion`.

## MODIFIED Requirements

### Requirement: Cancellation metadata and scheduling remains safe
Cancellation of a concurrent operation SHALL be idempotent, MAY carry an optional
typed `CancellationReason` whose default is `Cancelled`, and the runtime SHALL
NOT expose user-controlled scheduling priority. The runtime SHALL schedule fairly
and prevent starvation as an implementation responsibility.

#### Scenario: Cancellation omits a reason
- **WHEN** a concurrent operation is cancelled with no reason given
- **THEN** observers receive the default `CancellationReason.Cancelled`

### Requirement: Structured failure propagation
An unhandled exception in a concurrent branch SHALL fail that branch, cancel its
sibling branches in the same scope, await their cleanup, propagate the primary
failure out of the scope, and attach additional cleanup or sibling failures as
suppressed. A returned `Result.Error` SHALL remain an ordinary successful branch
value, never a branch failure.

#### Scenario: One branch throws
- **WHEN** a branch throws while sibling branches in the same scope are active
- **THEN** the siblings are cancelled and cleaned before the primary exception propagates out of the scope

### Requirement: Cooperative cancellation
Cancellation SHALL be cooperative, idempotent, observable only at defined safe
points (a blocking channel operation, a timer wait, `Timer.sleep`, an explicit
check), and propagated from a parent scope to its child branches. Cancellation
SHALL throw the compiler-known `CancelledError` at the safe point and SHALL NOT
stop a branch at an arbitrary instruction; the branch runs ordinary cleanup.

#### Scenario: Parent scope is cancelled
- **WHEN** a scope is cancelled while a child branch is suspended at a safe point
- **THEN** the child observes `CancelledError` at that point and runs its cleanup before finishing

### Requirement: Safe concurrent captures
A concurrent branch SHALL capture by the ordinary closure rules: values and
projections are snapshots, strict immutable complete references may be shared,
exclusive mutable references may transfer when statically safe, and an ambiguous
shared mutable alias is a compile-time error. A branch SHALL NOT mutate a
variable captured from an enclosing scope.

#### Scenario: Projected value is captured
- **WHEN** a branch captures `users[0]` rather than the complete `users` reference
- **THEN** it receives the independent projected value under the ordinary projection rule

#### Scenario: Mutable capture is rejected
- **WHEN** a branch body assigns a variable declared in the enclosing scope
- **THEN** compilation fails, and the developer is directed to a channel or the branch's return value

### Requirement: Data-race analysis is enforced before real parallelism exists
The compiler SHALL derive and enforce the `Transfer` and `Share` boundary rules
at concurrent-branch creation and captures, at `concurrent { }` results, and at
channel send and receive, even while the executor is single-threaded. The
analysis SHALL reject a mutable alias that would remain usable by a parent while
a child branch can mutate it, and SHALL name transfer, strict immutable sharing,
cloning, or a channel as the resolution.

#### Scenario: Shared mutable alias across a branch boundary is rejected
- **WHEN** a branch captures a mutable reference that its parent keeps using
- **THEN** compilation fails and names the available resolutions

### Requirement: A suspended operation's references stay reachable
A reference reachable only from a suspended concurrent branch SHALL NOT be
reclaimed by the garbage collector while that branch is alive, including a
reference held only in a compiler-managed temporary at the suspension point, a
reference in a queued channel value, and a reference in a `Job<T>` result slot
that has not yet been consumed.

#### Scenario: Local held across a safe point survives collection
- **WHEN** a branch holds the only reference to an object across a blocking channel operation and a collection runs
- **THEN** the object is intact when the branch resumes

### Requirement: Single-threaded cooperative executor is the Phase 5 vehicle
The implementation SHALL deliver the structured-branch, cancellation, and channel
behaviour of this capability on a single-threaded cooperative executor for
roadmap Phase 5. No branch SHALL be preempted at an arbitrary instruction, and a
branch SHALL observe cancellation, sibling failure, and channel readiness only at
a defined safe point. Real parallelism, operating-system threads, `Mutex<T>`, and
`Atomic<T>` are delivered later by the `concurrency-completion` change.

#### Scenario: No preemption between safe points
- **WHEN** a branch mutates local state and then reaches a blocking channel operation
- **THEN** no other branch has run between the mutation and that operation
