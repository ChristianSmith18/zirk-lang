## ADDED Requirements

### Requirement: Mutex provides scoped access with no guard escape

`Mutex<T>` SHALL expose access only through `mutex.with(closure)`, which SHALL
lock, run the closure with a writable view of the `T`, and unlock on every exit
edge. The writable view SHALL NOT escape the closure. The closure SHALL NOT
contain a safe point (`Timer.sleep`, a channel operation, `spawn`,
`Thread.run`); the compiler SHALL reject a mutex guard held across suspension and
SHALL name `RwLock<T>` or a redesign.

#### Scenario: guard cannot escape
- **WHEN** a `mutex.with` closure returns or stores the writable view
- **THEN** compilation fails

#### Scenario: awaiting under a mutex is rejected
- **WHEN** a `mutex.with` closure calls `Timer.sleep` or a channel receive
- **THEN** compilation fails and names `RwLock<T>` or a redesign

### Requirement: Atomic defaults to sequential consistency

`Atomic<T>` SHALL exist for supported boolean, integer, and low-level pointer
forms with `load` / `store` / `exchange` / `compare_exchange` and documented
numeric updates. Operations SHALL default to sequentially consistent ordering.
`AtomicOrder.relaxed` / `.acquire` / `.release` SHALL be accepted only inside an
`unsafe` context, where the programmer carries the ordering proof obligation.

#### Scenario: relaxed ordering outside unsafe is rejected
- **WHEN** code requests `AtomicOrder.relaxed` outside `unsafe`
- **THEN** compilation fails because the proof obligation is explicit

### Requirement: Concurrent.protect defers cancellation over a bounded region

`Concurrent.protect(fn)` SHALL run `fn` with the current branch's cancellation
delivery deferred, and SHALL deliver any pending `CancelledError` at the first
safe point after `fn` returns. The region SHALL be bounded; the compiler SHOULD
diagnose an unbounded protected region and the runtime unresolvable-wait detector
SHALL be the backstop.

#### Scenario: cleanup completes then cancellation is delivered
- **WHEN** a branch is cancelled while inside `Concurrent.protect` running a channel send
- **THEN** the send completes and `CancelledError` is delivered at the next safe point after `protect` returns

### Requirement: Safe-code data-race freedom is fully enforced

The compiler SHALL enforce the `Transfer` and `Share` boundary rules at branch
captures, `concurrent` results, channel send and receive, the `parallel` region
boundary, and the `Thread.run` boundary. A concurrent unsynchronized access where
at least one access mutates shared state SHALL be a compile-time error naming
transfer, strict immutable sharing, cloning, a channel, or a synchronizer.

#### Scenario: two branches mutate one list
- **WHEN** two concurrent branches mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap
