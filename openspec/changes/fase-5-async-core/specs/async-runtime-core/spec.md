## ADDED Requirements

### Requirement: Single-threaded cooperative executor

The runtime SHALL provide one executor that runs on a single operating-system
thread and schedules tasks cooperatively. A task SHALL yield control only at a
defined safe point (`await`, a suspending channel operation, `select`, a timer
wait, or an explicit cancellation check); the executor SHALL NOT preempt a
running task at an arbitrary instruction. The executor SHALL service ready tasks
from a first-in-first-out ready queue and SHALL check timer deadlines once per
scheduling turn. The executor SHALL NOT expose task priority as a user-controlled
value and SHALL prevent indefinite starvation of a ready task.

#### Scenario: Task runs until it suspends

- **WHEN** a task performs a long computation with no safe point
- **THEN** no other task is scheduled until that task reaches a safe point or completes

#### Scenario: Ready tasks are served fairly

- **WHEN** several tasks are ready at the same time
- **THEN** the executor runs them in first-in-first-out order and each one makes progress

#### Scenario: Executor drives the program to completion

- **WHEN** `main` starts child tasks and returns
- **THEN** the executor runs until `main`'s body and every descendant task have finished before the process exits

### Requirement: Executor detects an unresolvable wait

The executor SHALL abort with a diagnostic when the ready queue is empty, no
timer is armed, and at least one task is still suspended waiting — a program state
that can never make progress.

#### Scenario: All tasks are blocked forever

- **WHEN** every live task is suspended on a channel or handle that nothing will ever complete, and no timer is armed
- **THEN** the executor aborts and reports an unresolvable wait rather than hanging

### Requirement: Stackful task suspension and resumption

Creating a task SHALL allocate a dedicated stack and a task control block, and
SHALL start the task immediately as ready. Suspending a task SHALL save its
execution context (callee-saved registers, stack pointer, and resume point) into
its control block and return control to the executor. Resuming a task SHALL
restore that context. A plain function call SHALL use the ordinary native calling
convention whether or not the callee reaches a safe point internally; the
compiler SHALL NOT transform function bodies into resumable state machines and
SHALL NOT introduce a second calling convention for suspendable code.

#### Scenario: Function that awaits internally has no special ABI

- **WHEN** a function body creates a child task and awaits it, and another function calls that function directly
- **THEN** the call site uses the ordinary calling convention and the caller's frame is suspended and resumed together with the running task's stack

#### Scenario: Suspended frame is resumed intact

- **WHEN** a task suspends at `await` with local variables in scope and is later resumed
- **THEN** every local variable holds the value it had at suspension

### Requirement: Garbage-collection roots span every live task

The runtime SHALL enumerate garbage-collection roots through one shadow-stack
chain per task, anchored in that task's control block. A collection SHALL walk
the chain of every task the executor still owns — ready, running, suspended, or
running structured cleanup — not only the running task's chain. A reference held
by a suspended task, including a reference that lives only in a compiler-spilled
temporary slot at the suspension point, SHALL NOT be reclaimed while that task is
alive. The object header SHALL be unchanged by this requirement.

#### Scenario: Suspended task keeps its references alive

- **WHEN** a task holds the only reference to an object in a local variable, suspends at `await`, and a collection runs while another task allocates
- **THEN** the object survives the collection and is intact when the task resumes

#### Scenario: Reference only in a transient slot survives

- **WHEN** a task holds a reference solely in a compiler-spilled temporary across a suspension point and a collection runs
- **THEN** the reference is treated as a root and the object survives

#### Scenario: Finished task stops rooting its result

- **WHEN** a task has completed and its result has been consumed by `await`
- **THEN** the task's control block no longer roots any object and its stack is reclaimed

### Requirement: Timer service backs delayed and timed operations

The runtime SHALL provide a monotonic timer service that the executor consults
each scheduling turn. `after duration` in `select` and `await operation timeout
duration` SHALL be implemented through this service. A negative duration SHALL be
rejected as a controlled error before any waiting begins.

#### Scenario: Timer fires and unblocks its waiter

- **WHEN** a task waits on a timer and the deadline passes
- **THEN** the executor unblocks that task on the next scheduling turn

#### Scenario: Nearest deadline bounds an idle executor

- **WHEN** no task is ready but a timer is armed
- **THEN** the executor waits at most until the nearest deadline before running again

### Requirement: Channel runtime with cooperative suspension

The runtime SHALL provide a channel implementation supporting bounded
construction with a fixed positive capacity, zero-capacity rendezvous, and
dynamically growing construction with a mandatory defense limit. Suspendible send
and receive SHALL cooperate with the executor and SHALL NOT block the
operating-system thread. Try-send and try-receive SHALL never suspend and SHALL
return a typed outcome that distinguishes a value, temporary fullness or absence,
and closure. Close SHALL be idempotent, SHALL wake every suspended sender and
receiver, and SHALL allow already-queued values to be received before closure is
observed. A full bounded channel SHALL apply backpressure to senders. A
dynamically growing channel that reaches its configured limit SHALL apply
backpressure or return its documented typed failure rather than allocating
without bound.

#### Scenario: Bounded channel applies backpressure

- **WHEN** a sender calls suspendible send on a full bounded channel
- **THEN** the sender suspends without blocking the operating-system thread until capacity is available or the channel is closed

#### Scenario: Rendezvous channel pairs send with receive

- **WHEN** a zero-capacity channel has a pending sender and a receiver arrives
- **THEN** the value transfers directly and both operations complete

#### Scenario: Queued values drain before closure is observed

- **WHEN** a channel with queued values is closed and a receiver reads repeatedly
- **THEN** the receiver observes each queued value first and only then observes closure

#### Scenario: Growing channel honors its defense limit

- **WHEN** a dynamically growing channel constructed with an explicit limit reaches that limit
- **THEN** further sends apply backpressure or return the documented typed failure instead of allocating without bound

### Requirement: Channel storage is traced by the collector

A channel SHALL be a garbage-collected heap object whose queued values are
enumerated as roots while they remain in the channel. A reference sent into a
channel SHALL remain reachable until it is received or the channel is collected as
unreachable.

#### Scenario: Queued reference survives collection

- **WHEN** a reference value is sent into a channel, no other reference to it exists, and a collection runs before it is received
- **THEN** the value is still receivable and intact after the collection

### Requirement: Aggregation runtime for task collections

The runtime SHALL provide the backing operations for `Task.all`, `Task.first`,
and `Task.settled`. `Task.all` SHALL preserve input order and SHALL, on the first
unhandled failure, cancel the unfinished tasks, await their cleanup, and
propagate the primary failure with the others suppressed. `Task.first` SHALL
resolve with the first task to complete and SHALL cancel and clean the remainder.
`Task.settled` SHALL let every task finish, SHALL preserve input order, and SHALL
NOT cancel a task merely because another task rejected.

#### Scenario: All-aggregation cancels on first failure

- **WHEN** one task in a `Task.all` throws an unhandled exception while siblings are still running
- **THEN** the siblings are cancelled and cleaned before the exception propagates, and their later failures are attached as suppressed

#### Scenario: Settled-aggregation lets every task finish

- **WHEN** a `Task.settled` input set has one task that fulfills, one that throws, and one that is cancelled
- **THEN** the result lists ordered `Fulfilled`, `Rejected`, and `Cancelled` settlements and no task was cancelled because of a sibling's rejection

### Requirement: Executor owns the root scope of `main`

The runtime entry SHALL run `main`'s body as the root task of the executor and
SHALL NOT return the program's exit status until that task and all of its
descendants have completed or been cleaned. An exception that escapes the root
task SHALL cause a nonzero exit status, consistent with an uncaught exception
today.

#### Scenario: Program waits for background children

- **WHEN** `main` starts a child task and reaches the end of its body while the child is still running
- **THEN** the process does not exit until the child has finished or been cancelled and cleaned

#### Scenario: Uncaught failure in the root task

- **WHEN** an exception escapes `main`'s body after children are cleaned
- **THEN** the process exits with a nonzero status
