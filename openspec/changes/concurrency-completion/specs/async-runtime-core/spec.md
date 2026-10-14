## ADDED Requirements

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
