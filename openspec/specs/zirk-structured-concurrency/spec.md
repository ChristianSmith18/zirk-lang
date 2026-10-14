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
Zirk MUST NOT provide unrestricted branch detachment. Deliberate short-lived fire-and-forget work SHALL go through `Concurrent.detach`, owned by the app background scope and subject to bounded admission, cancellation, drain deadline, and `App.on_background_error`. Long-lived services SHALL be transferred through `application.spawn_service` to the application root supervisor, which owns readiness, startup failure, cancellation, ordered shutdown, resource cleanup, and final diagnostics. Deadline expiry SHALL report unfinished work; it SHALL NOT claim to forcibly terminate a native thread.

#### Scenario: Code attempts to detach a task
- **WHEN** ordinary code attempts to detach a task from all scopes
- **THEN** compilation fails and directs the developer to an application service supervisor

#### Scenario: A service outlives its request
- **WHEN** a handler calls `application.spawn_service(serve_metrics)` and returns
- **THEN** `serve_metrics` keeps running under the supervisor and is shut down in order when the application stops
### Requirement: Cooperative cancellation
Cancellation SHALL be cooperative, idempotent, observable only at defined safe points (a blocking channel operation, a timer wait, `Timer.sleep`, an explicit check), and propagated from a parent scope to its child branches. Cancellation SHALL throw the compiler-known `CancelledError` at the safe point and SHALL NOT stop a branch at an arbitrary instruction; the branch runs ordinary cleanup.

#### Scenario: Parent scope is cancelled
- **WHEN** a scope is cancelled while a child branch is suspended at a safe point
- **THEN** the child observes `CancelledError` at that point and runs its cleanup before finishing

### Requirement: Typed channels and closure
`Channel<T>` SHALL support explicit bounded construction, zero-capacity
rendezvous, defensively limited dynamically growing construction, suspendible
send/receive that are cancellation safe points, nonblocking typed `try_*`
operations, idempotent close, and sender backpressure. `receive()` SHALL return
`Some(v)` while queued values remain and `None` after a closed channel drains;
`drain()` SHALL collect values in order until that point. Standard broadcast,
latest-value watch, and one-shot channel families remain deferred.

#### Scenario: Bounded channel is full
- **WHEN** a sender uses suspendible send on a full bounded channel
- **THEN** the sender suspends without blocking an OS thread until capacity or closure is observed

#### Scenario: Dynamic channel reaches its defense limit
- **WHEN** `Channel.unbounded(limit:)` reaches the mandatory configured limit
- **THEN** sending applies backpressure or returns the documented typed failure
  rather than allocating without bound

#### Scenario: try receive distinguishes empty from closed
- **WHEN** `try_receive()` is called on an open empty channel and then on a closed drained channel
- **THEN** it returns `Empty` first and `Closed` second

#### Scenario: Closed channel drains before absence

- **WHEN** a closed channel contains three queued values and `receive` is called four times
- **THEN** the first three calls return the values in FIFO order and the fourth returns `None`

#### Scenario: Cancellation interrupts a suspended receive

- **WHEN** a branch is cancelled while suspended in `channel.receive()`
- **THEN** `receive` raises `CancelledError` at that safe point

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

#### Scenario: Non-associative combiner is rejected
- **WHEN** a parallel reduction uses a combiner the checker cannot establish as associative
- **THEN** compilation fails unless the caller selects `reduce_ordered`

### Requirement: Scoped threads and blocking adapter
`Thread.run(fn)` SHALL use bounded managed OS-thread admission for native affinity or blocking isolation and return its result, suspending the calling branch cooperatively while the executor keeps working. Every managed participant SHALL publish stable roots and participate in collector coordination, including while blocked in native code. `fn` SHALL NOT `spawn`, open `concurrent { }`, use a channel, or call `Timer.sleep`; transitive effects SHALL be checked conservatively. If a target cannot uphold the collector and lifetime contract, the feature SHALL be rejected there.

#### Scenario: Blocking native API is called from task
- **WHEN** a developer wraps a blocking C call in the managed `Thread.run` adapter
- **THEN** the calling task suspends while managed execution runs the call and the executor services other tasks

#### Scenario: suspension inside a Thread.run body is rejected
- **WHEN** a `Thread.run` body performs a channel operation
- **THEN** compilation fails and directs asynchronous I/O to `concurrent { }`
### Requirement: Structured synchronization
`Mutex<T>` SHALL provide scoped access through `mutex.with(closure)` that prevents
the writable view from escaping, and a mutex guard MUST NOT be held across a safe
point (`Timer.sleep`, a channel operation, `spawn`, `Thread.run`). The standard
library SHALL provide `RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` as
library types, not language syntax.

#### Scenario: Mutex guard crosses await
- **WHEN** code attempts to await while holding an ordinary mutex guard
- **THEN** compilation fails or the required specialized synchronization contract is identified

#### Scenario: Guard escape is rejected
- **WHEN** a `mutex.with` closure returns or stores the writable view
- **THEN** compilation fails
### Requirement: Safe atomics
`Atomic<T>` SHALL exist only for supported boolean and integer scalar widths;
pointer forms, including managed GC pointers, are unsupported until a separate
rooting and lifetime protocol is approved. Its scalar payload SHALL be inline
in the GC-managed object allocation with target-correct alignment and SHALL
not be traced as a pointer. Supported operations, result semantics, arithmetic
behavior, and target widths are defined by the synchronization capability
contract. Operations SHALL default to sequentially consistent ordering, and
all weaker orderings SHALL require an `unsafe` context, where the programmer
carries the ordering proof obligation.

#### Scenario: Relaxed load is used in safe code
- **WHEN** code requests `AtomicOrder.relaxed` outside `unsafe`
- **THEN** compilation fails because the proof obligation is explicit
### Requirement: Safe-code data-race freedom
Safe Zirk SHALL reject concurrent unsynchronized accesses when at least one
access mutates shared state, enforced at concurrent-branch captures, `concurrent`
results, channel send and receive, the `parallel` region boundary, and the
`Thread.run` boundary, while making no guarantee that independent branch
completion order is deterministic.

#### Scenario: Two tasks mutate shared list
- **WHEN** two concurrent tasks mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap

#### Scenario: Parallel region mutates a shared alias
- **WHEN** a parallel region mutates an alias that remains mutable outside the region
- **THEN** compilation fails and identifies the required safe boundary

#### Scenario: Two branches mutate a shared list
- **WHEN** two concurrent branches mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap

#### Scenario: Thread.run boundary race is rejected
- **WHEN** a `Thread.run` body mutates a list its enclosing scope also reads
- **THEN** compilation fails and names transfer, strict immutable sharing, cloning, a channel, or a synchronizer
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

### Requirement: Cancellation is a cleanup-aware structured exit

Cancellation SHALL enter the compiler's structured cleanup path rather than unwind across generated language frames as an opaque host panic. A cancelled branch SHALL be reported terminal only after its lexical cleanup has completed.

#### Scenario: Cancellation crosses nested cleanup scopes
- **WHEN** cancellation leaves nested `try/finally`, resource, and unsafe scopes
- **THEN** cleanup executes from the innermost scope outward and the branch reports cancellation afterward

#### Scenario: Cancellation is observed after cleanup
- **WHEN** a job is cancelled while it owns language resources
- **THEN** `Job` completion becomes observable only after cleanup and rollback obligations finish

### Requirement: Non-cancellable region

`Concurrent.protect(fn)` SHALL support synchronous, non-suspending cleanup only. It SHALL preserve pending cancellation and deliver it after cleanup; it SHALL NOT make channel, timer, task, or thread waits immune to cancellation. Transitive suspension, blocking, spawning, or incomplete effects SHALL be rejected. The compiler does not prove cleanup termination or a time bound, and the runtime does not preempt a non-suspending loop; cleanup code must terminate for cancellation to be delivered. The checker conservatively rejects potentially blocking work as well as suspension. `Concurrent.protect` replaces the removed `cancellation shield`; no runtime unresolvable-wait detector is assumed.

#### Scenario: cleanup completes then cancellation is delivered
- **WHEN** a branch is cancelled while inside `Concurrent.protect` running synchronous cleanup whose effects are known to be non-suspending, non-blocking, and non-spawning
- **THEN** cleanup completes and `CancelledError` is delivered at the next safe point after `protect` returns

#### Scenario: unsafe cleanup effects are rejected
- **WHEN** a protected closure directly or transitively calls a channel wait, timer, spawn, or managed thread operation
- **THEN** compilation rejects the incomplete, suspending, blocking, or spawning effect summary before lowering
