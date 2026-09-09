## MODIFIED Requirements

### Requirement: Supervised long-lived services
Zirk MUST NOT provide unrestricted branch detachment. Deliberate fire-and-forget
work SHALL go through `Concurrent.detach`, which spawns into the app background
scope (cancelled and drained at shutdown, failures to `App.on_background_error`).
Long-lived services SHALL be transferred through `application.spawn_service` to
the application root supervisor, which owns startup failure, cancellation,
ordered shutdown, resource cleanup, and final diagnostics.

#### Scenario: Code attempts an unsupervised detach
- **WHEN** ordinary code attempts to detach a branch from every scope without `Concurrent.detach` or `application.spawn_service`
- **THEN** compilation fails and directs the developer to one of those two

#### Scenario: A service outlives its request
- **WHEN** a handler calls `application.spawn_service(serve_metrics)` and returns
- **THEN** `serve_metrics` keeps running under the supervisor and is shut down in order when the application stops

### Requirement: Scoped threads and blocking adapter
`Thread.run(fn)` SHALL run `fn` on a fresh operating-system thread for native
affinity or blocking isolation and return its result, suspending the calling
branch cooperatively while the executor keeps working. `fn` SHALL NOT `spawn`,
open `concurrent { }`, use a channel, or call `Timer.sleep`. The OS thread SHALL
register with the collector and park at a safepoint on request.

#### Scenario: Blocking native API is called through Thread.run
- **WHEN** a developer wraps a blocking C call in `Thread.run`
- **THEN** the calling branch suspends while the OS thread runs the call and the executor services other branches

#### Scenario: suspension inside a Thread.run body is rejected
- **WHEN** a `Thread.run` body performs a channel operation
- **THEN** compilation fails and directs asynchronous I/O to `concurrent { }`

### Requirement: Structured synchronization
`Mutex<T>` SHALL provide scoped access through `mutex.with(closure)` that prevents
the writable view from escaping, and a mutex guard MUST NOT be held across a safe
point (`Timer.sleep`, a channel operation, `spawn`, `Thread.run`). The standard
library SHALL provide `RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` as
library types, not language syntax.

#### Scenario: Mutex guard held across a safe point
- **WHEN** a `mutex.with` closure calls `Timer.sleep` or a channel receive
- **THEN** compilation fails and identifies `RwLock<T>` or a redesign

#### Scenario: Guard escape is rejected
- **WHEN** a `mutex.with` closure returns or stores the writable view
- **THEN** compilation fails

### Requirement: Safe atomics
`Atomic<T>` SHALL exist only for supported boolean, integer, and low-level
pointer forms and operations (`load` / `store` / `exchange` / `compare_exchange`
and documented numeric updates), SHALL default to sequentially consistent
ordering, and SHALL require an `unsafe` context for `AtomicOrder.relaxed` /
`.acquire` / `.release`, where the programmer carries the ordering proof
obligation.

#### Scenario: Relaxed load is used in safe code
- **WHEN** code requests `AtomicOrder.relaxed` outside `unsafe`
- **THEN** compilation fails because the proof obligation is explicit

### Requirement: Safe-code data-race freedom
Safe Zirk SHALL reject concurrent unsynchronized accesses when at least one
access mutates shared state, enforced at concurrent-branch captures, `concurrent`
results, channel send and receive, the `parallel` region boundary, and the
`Thread.run` boundary, while making no guarantee that independent branch
completion order is deterministic.

#### Scenario: Two branches mutate a shared list
- **WHEN** two concurrent branches mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap

#### Scenario: Thread.run boundary race is rejected
- **WHEN** a `Thread.run` body mutates a list its enclosing scope also reads
- **THEN** compilation fails and names transfer, strict immutable sharing, cloning, a channel, or a synchronizer

## ADDED Requirements

### Requirement: Non-cancellable region

`Concurrent.protect(fn)` SHALL run `fn` with the current branch's cancellation
delivery deferred, and SHALL deliver any pending `CancelledError` at the first
safe point after `fn` returns. The region SHALL be bounded; the compiler SHOULD
diagnose an unbounded protected region and the runtime unresolvable-wait detector
SHALL be the backstop. `Concurrent.protect` replaces the removed `cancellation
shield`.

#### Scenario: cleanup completes then cancellation is delivered
- **WHEN** a branch is cancelled while inside `Concurrent.protect` running a channel send
- **THEN** the send completes and `CancelledError` is delivered at the next safe point after `protect` returns
