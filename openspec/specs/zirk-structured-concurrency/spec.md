# zirk-structured-concurrency Specification

## Purpose
Defines model-neutral structured concurrency, cancellation, transfer/share
safety, channels, threads, parallelism, and synchronization.
## Requirements
### Requirement: Cancellation metadata and scheduling remain safe
Cancellation of a concurrent operation SHALL be idempotent, MAY carry an optional typed `CancellationReason` whose default is `Cancelled`, and SHALL NOT expose user-controlled scheduling priority.
The runtime SHALL schedule fairly and prevent starvation as an implementation
responsibility.

#### Scenario: Cancellation omits a reason
- **WHEN** source invokes `operation.cancel()`
- **THEN** observers receive the default `CancellationReason.Cancelled`

### Requirement: Structured failure propagation
An unhandled exception in a concurrent branch SHALL fail that branch, cancel its sibling branches in the same scope, await their cleanup, propagate the primary failure out of the scope, and attach additional cleanup or sibling failures as suppressed. A returned `Result.Error` SHALL remain an ordinary successful branch value, never a branch failure.

#### Scenario: One child throws
- **WHEN** a branch throws while sibling branches in the same scope are active
- **THEN** the siblings are cancelled and cleaned before the primary exception propagates

### Requirement: Supervised long-lived services
Zirk MUST NOT provide unrestricted task detachment, and long-lived work SHALL be transferred explicitly to an application root supervisor that owns shutdown, cancellation, and error reporting.

#### Scenario: Code attempts to detach a task
- **WHEN** ordinary code attempts to detach a task from all scopes
- **THEN** compilation fails and directs the developer to an application service supervisor

### Requirement: Cooperative cancellation
Cancellation SHALL be cooperative, idempotent, observable only at defined safe points (a blocking channel operation, a timer wait, `Timer.sleep`, an explicit check), and propagated from a parent scope to its child branches. Cancellation SHALL throw the compiler-known `CancelledError` at the safe point and SHALL NOT stop a branch at an arbitrary instruction; the branch runs ordinary cleanup.

#### Scenario: Parent scope is cancelled
- **WHEN** a scope is cancelled while a child branch is suspended at a safe point
- **THEN** the child observes `CancelledError` at that point and runs its cleanup before finishing

### Requirement: Typed channels and closure
`Channel<T>` SHALL support explicit bounded construction, zero-capacity
rendezvous, defensively limited dynamically growing construction, suspendible
send/receive, nonblocking try operations, idempotent close, backpressure, and an
unambiguous distinction between value, closure, failure, and temporary absence.
Queued values SHALL drain before closure is observed. Standard broadcast,
latest-value watch, and one-shot channel families SHALL preserve the same
transfer and cancellation rules.

#### Scenario: Bounded channel is full
- **WHEN** a sender uses suspendible send on a full bounded channel
- **THEN** the sender suspends without blocking an OS thread until capacity or closure is observed

#### Scenario: Dynamic channel reaches its defense limit
- **WHEN** `Channel.unbounded(limit:)` reaches the mandatory configured limit
- **THEN** sending applies backpressure or returns the documented typed failure
  rather than allocating without bound

### Requirement: Derived transfer and sharing
The compiler SHALL derive non-user-forgeable `Transfer` and `Share` properties: values and projections copy, strict immutable references may share, exclusive mutable references may transfer, cloned references become independent, and synchronization-aware references may share.

#### Scenario: Mutable alias crosses task boundary
- **WHEN** a mutable reference would remain usable by the parent while a child can mutate it concurrently
- **THEN** compilation fails and suggests transfer, strict sharing, synchronization, or cloning

### Requirement: Safe concurrent captures
A concurrent branch SHALL capture by the ordinary closure rules: values and projections are snapshots, strict immutable complete references may be shared, exclusive mutable references may transfer when statically safe, and an ambiguous shared mutable alias is a compile-time error. A branch SHALL NOT mutate a variable captured from an enclosing scope.

#### Scenario: Projected child is captured
- **WHEN** a branch captures `users[0]` rather than the complete `users` reference
- **THEN** it receives the independent projected value under the ordinary projection rule

### Requirement: Parallel CPU operations
`parallel` SHALL represent finite CPU work, reject unmanaged blocking I/O and unsynchronized mutation, preserve input order for ordered map-like operations, and require an explicit unordered variant when completion order is desired.

#### Scenario: Parallel map finishes out of order
- **WHEN** later input elements finish before earlier elements
- **THEN** the returned ordered collection still matches input order

### Requirement: Parallel reductions
Parallel reduction SHALL require an associative combiner, MAY regroup operations, and SHALL provide an explicit deterministic variant when grouping-sensitive results are required.

#### Scenario: Floating reduction is parallel
- **WHEN** floating values are reduced with the ordinary parallel reduction
- **THEN** documentation and types do not promise bit-identical grouping to sequential evaluation

### Requirement: Scoped threads and blocking adapter
`thread` SHALL create scoped OS execution for native affinity or blocking isolation, and `task.blocking` SHALL execute legacy blocking work on a separate pool without blocking the task scheduler.

#### Scenario: Blocking native API is called from task
- **WHEN** a developer wraps it in `task.blocking`
- **THEN** the current task suspends while a blocking pool executes the call

### Requirement: Structured synchronization
`Mutex<T>` SHALL provide scoped access that prevents lock escape, ordinary mutex guards MUST NOT cross `await`, and the standard library SHALL provide `RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` as library types rather than language syntax.

#### Scenario: Mutex guard crosses await
- **WHEN** code attempts to await while holding an ordinary mutex guard
- **THEN** compilation fails or the required specialized synchronization contract is identified

### Requirement: Safe atomics
`Atomic<T>` SHALL exist only for supported values and operations, SHALL default to sequentially consistent ordering, and SHALL require unsafe code for explicitly weaker memory ordering.

#### Scenario: Relaxed load is used in safe code
- **WHEN** code requests `AtomicOrder.relaxed` outside unsafe
- **THEN** compilation fails because the proof obligation is explicit

### Requirement: Safe-code data-race freedom
Safe Zirk SHALL reject concurrent unsynchronized accesses when at least one access mutates shared state, while making no guarantee that independent task completion order is deterministic.

#### Scenario: Two tasks mutate shared list
- **WHEN** two concurrent tasks mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap

### Requirement: Single-threaded cooperative executor is the Phase 5 step 1-3 vehicle

The implementation SHALL deliver the structured-task, cancellation, aggregation,
selection, and channel behavior of this capability on a single-threaded
cooperative executor for roadmap Phase 5 steps 1 to 3. On that executor no task
SHALL be preempted at an arbitrary instruction, and a task SHALL observe
cancellation, sibling failure, timeouts, channel readiness, and `select`
readiness only at a defined safe point. Real parallelism, operating-system
threads, `parallel`, `Mutex<T>`, and `Atomic<T>` remain out of this vehicle and
are delivered later.

#### Scenario: No preemption between safe points

- **WHEN** a task mutates local state and then reaches an `await`
- **THEN** no other task has run between the mutation and the `await`

#### Scenario: Deferred concurrency constructs still diagnose

- **WHEN** source uses `parallel`, `thread`, `Mutex`, or `Atomic`
- **THEN** the compiler names the construct and the later step that delivers it, and does not treat it as implemented

### Requirement: A suspended task's references stay reachable

A reference reachable only from a suspended task SHALL NOT be reclaimed by the
garbage collector while that task is alive, including a reference held only in a
compiler-managed temporary at the suspension point, a reference in a queued
channel value, and a reference in a `Task<T>` result slot that has not yet been
consumed.

#### Scenario: Local held across await survives collection

- **WHEN** a task holds the only reference to an object across an `await` and a collection runs
- **THEN** the object is intact when the task resumes

#### Scenario: Unconsumed result survives collection

- **WHEN** a task has completed with a reference result that no `await` has consumed yet, and a collection runs
- **THEN** the result is intact when it is later awaited

### Requirement: Data-race analysis is enforced before real parallelism exists

The compiler SHALL derive and enforce the `Transfer` and `Share` boundary rules
of this capability at `task` creation and captures, `task scope` results, channel
send and receive, and `select` branch values, even though the Phase 5 step 1-3
executor is single-threaded. The analysis SHALL reject a mutable alias that would
remain usable by a parent while a child can mutate it, and SHALL name transfer,
strict immutable sharing, cloning, or a channel as the resolution.

#### Scenario: Shared mutable alias across a task boundary is rejected

- **WHEN** a task captures a mutable reference that its parent keeps using
- **THEN** compilation fails and names the available resolutions

#### Scenario: Transferred exclusive reference is unusable by the sender

- **WHEN** a parent transfers an exclusive mutable reference into a child task
- **THEN** the parent cannot use that reference again until it returns through a structured result or a channel
