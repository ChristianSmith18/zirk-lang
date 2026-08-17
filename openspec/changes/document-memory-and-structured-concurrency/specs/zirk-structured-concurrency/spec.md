## ADDED Requirements

### Requirement: Typed structured tasks
`task` SHALL create a child in the current structured scope and return `Task<T>`, while `await` SHALL produce exactly `T` and scope exit MUST NOT abandon unfinished children.

#### Scenario: Scope exits with running child
- **WHEN** control reaches the end of a scope containing an unfinished child task
- **THEN** the scope awaits completion or requests cancellation and awaits cleanup before exiting

### Requirement: Structured task failure
An unhandled task exception SHALL fail that task, cancel its siblings, await their cleanup, propagate the primary failure, and attach additional cleanup or sibling failures as suppressed; `Result.Error` SHALL remain an ordinary successful task value.

#### Scenario: One child throws
- **WHEN** a child throws while sibling tasks are active
- **THEN** the siblings are cancelled and cleaned before the primary exception propagates

### Requirement: Supervised long-lived services
Zirk MUST NOT provide unrestricted task detachment, and long-lived work SHALL be transferred explicitly to an application root supervisor that owns shutdown, cancellation, and error reporting.

#### Scenario: Code attempts to detach a task
- **WHEN** ordinary code attempts to detach a task from all scopes
- **THEN** compilation fails and directs the developer to an application service supervisor

### Requirement: Cooperative cancellation and shielding
Cancellation SHALL be cooperative, idempotent, observable at defined safe points, and propagated from parent to child; `cancellation shield` SHALL defer delivery only for its bounded region and deliver pending cancellation afterward.

#### Scenario: Cancelled task enters cleanup shield
- **WHEN** cancellation is requested while a task performs shielded commit cleanup
- **THEN** cleanup completes and cancellation is observed immediately after the shield

### Requirement: Structured timeout
`await operation timeout duration` SHALL cancel the operation at expiry, await its cleanup, and throw `TimeoutError` without leaving background work.

#### Scenario: Operation exceeds timeout
- **WHEN** an awaited operation remains incomplete after its timeout
- **THEN** it is cancelled and cleaned before `TimeoutError` escapes

### Requirement: Task aggregation policies
`Task.all` SHALL cancel remaining tasks after the first unhandled exception, `Task.first` SHALL return the first completed task and cancel the rest, and `Task.settled` SHALL allow every task to finish and preserve input order as `TaskSettlement<T>`.

#### Scenario: Settled aggregation includes failures
- **WHEN** one task fulfills, one throws, and one is cancelled
- **THEN** `Task.settled` returns ordered `Fulfilled`, `Rejected`, and `Cancelled` settlements without sibling failure cancellation

### Requirement: Result and settlement separation
A fulfilled `Task<Result<T,E>>` SHALL contain either `Ok` or `Error` inside `TaskSettlement.Fulfilled`, while only an unhandled throwable SHALL produce `Rejected`.

#### Scenario: Task returns Result Error
- **WHEN** a task returns `Error(problem)` normally
- **THEN** settled aggregation records `Fulfilled(Error(problem))`

### Requirement: Fair selection
`select` SHALL wait for the first ready task, channel operation, timer, or cancellation signal; execute exactly one branch; preserve losing operations; support `default`; treat closure as a ready channel outcome; and avoid permanent starvation when multiple branches are ready.

#### Scenario: Message arrives before timer
- **WHEN** a selected channel receive becomes ready before `after 5s`
- **THEN** the message branch executes and the timer branch does not

### Requirement: Typed channels and closure
`Channel<T>` SHALL support bounded and unbounded construction, suspendible send/receive, nonblocking try operations, explicit close, observable capacity/length, backpressure, and an unambiguous distinction between closure and temporary absence.

#### Scenario: Bounded channel is full
- **WHEN** a sender uses suspendible send on a full bounded channel
- **THEN** the sender suspends without blocking an OS thread until capacity or closure is observed

### Requirement: Derived transfer and sharing
The compiler SHALL derive non-user-forgeable `Transfer` and `Share` properties: values and projections copy, strict immutable references may share, exclusive mutable references may transfer, cloned references become independent, and synchronization-aware references may share.

#### Scenario: Mutable alias crosses task boundary
- **WHEN** a mutable reference would remain usable by the parent while a child can mutate it concurrently
- **THEN** compilation fails and suggests transfer, strict sharing, synchronization, or cloning

### Requirement: Safe task captures
Task captures SHALL snapshot values and projections, share strict immutable or synchronization-aware references, and reject mutable reference captures that are neither exclusive transfers nor statically non-overlapping.

#### Scenario: Projected child is captured
- **WHEN** a task captures `users[0]` rather than the complete `users` reference
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
