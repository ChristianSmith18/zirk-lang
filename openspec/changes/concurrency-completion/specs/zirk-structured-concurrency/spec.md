## MODIFIED Requirements

### Requirement: Supervised long-lived services
Zirk MUST NOT provide unrestricted branch detachment. Deliberate short-lived fire-and-forget work SHALL go through `Concurrent.detach`, owned by the app background scope and subject to bounded admission, cancellation, drain deadline, and `App.on_background_error`. Long-lived services SHALL be transferred through `application.spawn_service` to the application root supervisor, which owns readiness, startup failure, cancellation, ordered shutdown, resource cleanup, and final diagnostics. Deadline expiry SHALL report unfinished work; it SHALL NOT claim to forcibly terminate a native thread.

#### Scenario: Code attempts to detach a task
- **WHEN** ordinary code attempts to detach a task from all scopes
- **THEN** compilation fails and directs the developer to an application service supervisor

#### Scenario: A service outlives its request
- **WHEN** a handler calls `application.spawn_service(serve_metrics)` and returns
- **THEN** `serve_metrics` keeps running under the supervisor and is shut down in order when the application stops

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

## ADDED Requirements

### Requirement: Non-cancellable region

`Concurrent.protect(fn)` SHALL support synchronous, non-suspending cleanup only. It SHALL preserve pending cancellation and deliver it after cleanup; it SHALL NOT make channel, timer, task, or thread waits immune to cancellation. Transitive suspension, blocking, spawning, or incomplete effects SHALL be rejected. The compiler does not prove cleanup termination or a time bound, and the runtime does not preempt a non-suspending loop; cleanup code must terminate for cancellation to be delivered. The checker conservatively rejects potentially blocking work as well as suspension. `Concurrent.protect` replaces the removed `cancellation shield`; no runtime unresolvable-wait detector is assumed.

#### Scenario: cleanup completes then cancellation is delivered
- **WHEN** a branch is cancelled while inside `Concurrent.protect` running synchronous cleanup whose effects are known to be non-suspending, non-blocking, and non-spawning
- **THEN** cleanup completes and `CancelledError` is delivered at the next safe point after `protect` returns

#### Scenario: unsafe cleanup effects are rejected
- **WHEN** a protected closure directly or transitively calls a channel wait, timer, spawn, or managed thread operation
- **THEN** compilation rejects the incomplete, suspending, blocking, or spawning effect summary before lowering
