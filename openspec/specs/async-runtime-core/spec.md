# async-runtime-core Specification

## Purpose
TBD - created by archiving change fase-5-async-core. Update Purpose after archive.
## Requirements
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

The implementation SHALL retain first-in-first-out suspended sender and
receiver lists. A sender resumed after capacity becomes available SHALL enqueue
its value; a receiver resumed after close SHALL first drain queued values and
then observe closure. The executor thread SHALL never block on either operation.

#### Scenario: Bounded channel applies backpressure

- **WHEN** a sender calls suspendible send on a full bounded channel
- **THEN** the sender suspends without blocking the operating-system thread until capacity is available or the channel is closed

#### Scenario: Sender resumes when capacity frees

- **WHEN** a branch is suspended in `send` on a full channel and another branch receives one value
- **THEN** the suspended sender resumes and enqueues its value

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

The collector trace hook SHALL enumerate queued values but SHALL NOT traverse
the sender or receiver task-id lists: their live branches are already enumerated
by the executor. This extension SHALL not alter the shared object header.

#### Scenario: Queued reference survives collection

- **WHEN** a reference value is sent into a channel, no other reference to it exists, and a collection runs before it is received
- **THEN** the value is still receivable and intact after the collection

#### Scenario: Close wakes every waiter

- **WHEN** a channel with two suspended receivers and one suspended sender is closed
- **THEN** the receivers resume observing drained closure and the suspended `send` raises

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

### Requirement: Worker pool and thread-aware roots

The runtime SHALL provide a fixed worker pool for `parallel` regions while the
cooperative executor stays single-threaded. During a parallel region, collection
SHALL park every worker at a safepoint and enumerate every task and worker
shadow-stack chain before mark-sweep resumes the world.

#### Scenario: Collection during a parallel region
- **WHEN** an allocation-heavy parallel region crosses the GC threshold
- **THEN** every worker and the executor park, live references survive, and the region resumes

### Requirement: Executor task and timer admission is bounded

Each executor SHALL admit at most 4,096 nonterminal tasks, including its root
task, and at most 4,096 occupied timer permits. Admission beyond either limit
SHALL report a typed capacity error to the caller and SHALL NOT create a task,
timer, scope registration, or retained capture root. A task releases its
active-task permit when it reaches any
terminal state (success, failure, or cancellation), even when its joinable
outcome remains retained for a later join. Thus this task limit bounds active
work, not retained terminal task records or total result memory. A timer releases
its active-timer permit when it expires or is cancelled/disarmed. A timer-backed
callback reserves both a task permit and one timer permit before publication;
its timer permit remains occupied between periodic arms and is released at
terminal cleanup. Ordinary waits occupy timer permits only while armed. The
limits are per executor, not process-wide, and do not limit OS threads managed
outside that executor.

#### Scenario: task admission reaches the executor ceiling

- **WHEN** an executor, including its root task, has 4,096 nonterminal tasks
  and another task is requested
- **THEN** the request returns a typed task-capacity error and does not
  register or schedule another task

#### Scenario: task capacity is returned at termination

- **WHEN** an accepted task succeeds, fails, or is cancelled
- **THEN** one active-task permit becomes available even if its joinable
  outcome remains unconsumed

#### Scenario: timer admission reaches the executor ceiling

- **WHEN** an executor has 4,096 armed timers and another timer is requested
- **THEN** the request returns a typed timer-capacity error without parking a
  caller or retaining a callback capture

#### Scenario: timer capacity is returned on expiry or cancellation

- **WHEN** an armed timer expires or is cancelled/disarmed
- **THEN** one active-timer permit becomes available exactly once

### Requirement: JobWaitAny rejects invalid task handles before suspension

The `zirk_rt_job_wait_any` ABI SHALL validate every in-range task ID before it
registers the caller as a waiter. A list containing a stale or non-live ID, a
duplicate ID, the current task's own ID, or a result already consumed or
claimed by another waiter SHALL terminate through the runtime fatal-error path
(exit code 70) before suspension. Invalid handles SHALL NOT be reported using
the `u64::MAX` caller-cancellation sentinel. The existing `2..=8` count bound
and return contract SHALL remain unchanged.

#### Scenario: invalid wait-any task ID fails closed

- **WHEN** an executor task calls `zirk_rt_job_wait_any` with an in-range list
  containing a stale, duplicate, self, or already-awaited task ID
- **THEN** the runtime emits a fatal diagnostic and exits with code 70 before
  suspending the caller
- **AND** it does not return the caller-cancellation sentinel

#### Scenario: valid wait-any still reports caller cancellation

- **WHEN** a valid wait-any call observes pending cancellation for its caller
- **THEN** it returns `u64::MAX` as before

### Requirement: Compiler deadline race has a separately bounded wait-any ABI

The runtime SHALL expose `zirk_rt_job_wait_any_with_timer` for compiler-generated
deadline races over 2..=9 live task IDs (at most eight source jobs and one
compiler-owned timer job). It SHALL preserve the validation, completion-order,
and cancellation-sentinel behavior of `zirk_rt_job_wait_any`. The existing
`zirk_rt_job_wait_any` ABI SHALL remain bounded to 2..=8 and SHALL NOT be widened.
Both entry points SHALL reject an out-of-range count before reading the ID
pointer.

#### Scenario: eight source jobs can race one deadline

- **WHEN** compiler-generated code waits for eight source jobs and one timer
- **THEN** the timer-specific ABI accepts nine valid IDs while ordinary
  `zirk_rt_job_wait_any` continues to reject a count of nine

#### Scenario: timer ABI rejects an out-of-range count before pointer access

- **WHEN** the timer-specific ABI receives a count outside 2..=9
- **THEN** it returns the cancellation sentinel without reading the ID pointer

### Requirement: App background scope and shutdown drain

The runtime SHALL create one `App.background` sub-scope of `main`'s implicit
scope. `Concurrent.detach(fn)` SHALL register `fn` as a branch of it. On `main`
completion or a shutdown signal the runtime SHALL cancel `App.background`, drain
it within `App.shutdown_grace` (default 5 seconds, settable), then exit. An
unhandled exception in a detached branch SHALL be delivered to
`App.on_background_error` (a settable `(Throwable): Void`, default logs to
stderr) and SHALL NOT abort the process.

#### Scenario: detached work is drained at shutdown
- **WHEN** `main` completes with one detached branch still running
- **THEN** the branch is cancelled and given up to the grace period to clean up before the process exits

#### Scenario: detached failure reaches the handler
- **WHEN** a detached branch throws and `App.on_background_error` is set
- **THEN** the handler receives the throwable and the process is not aborted

### Requirement: Thread.run join and the synchronizer runtime

The runtime SHALL spawn a bounded managed OS thread for `Thread.run`, register it with the
collector safepoint, suspend the calling branch, and resume it on join. The
runtime SHALL provide `Atomic<T>` over the target's native atomics, `Mutex<T>`
with FIFO fairness, and `RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>`.

#### Scenario: executor works during Thread.run
- **WHEN** one branch is inside `Thread.run(blocking_decode)` and another waits on a channel
- **THEN** the executor services the channel branch until the OS thread joins

#### Scenario: Once runs its initializer once
- **WHEN** several branches call `once.get_or_init(f)` concurrently
- **THEN** `f` runs exactly once and every caller observes the same value
