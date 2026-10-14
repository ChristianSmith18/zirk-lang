# synchronization

## Purpose

Defines the normative requirements for synchronization in Zirk.

## Requirements

### Requirement: Mutex provides scoped access with no guard escape

`Mutex<T>` SHALL expose access only through `mutex.with(closure)`, which SHALL acquire through a scheduler-integrated wait queue (never block the cooperative executor), run the closure with a writable view of the `T`, and unlock on every exit edge. Fairness, reentrancy, poisoning, cancellation, and panic behavior SHALL be explicit; the default mutex is non-reentrant and FIFO-fair. The writable view SHALL NOT escape the closure. The closure SHALL NOT
contain a safe point (`Timer.sleep`, a channel operation, `spawn`,
`Thread.run`); the compiler SHALL reject a mutex guard held across suspension and
SHALL name `RwLock<T>` or a redesign.

#### Scenario: mutex waiter cancellation removes the waiter
- **WHEN** a task is cancelled while queued for a mutex
- **THEN** it is removed safely and the next eligible waiter can acquire the mutex

#### Scenario: no mutex wait blocks the executor
- **WHEN** a task requests a held mutex while another task can release it
- **THEN** the requesting task parks through the scheduler and unrelated ready tasks continue to run

#### Scenario: barrier cancellation breaks its generation
- **WHEN** one participant is cancelled while waiting at a barrier
- **THEN** all participants in that generation wake with the documented broken-barrier outcome

#### Scenario: failed Once initializer follows retry policy
- **WHEN** the initializer throws before publishing a value
- **THEN** callers observe the documented failure and retry behavior without observing partial initialization

#### Scenario: guard cannot escape
- **WHEN** a `mutex.with` closure returns or stores the writable view
- **THEN** compilation fails

#### Scenario: awaiting under a mutex is rejected
- **WHEN** a `mutex.with` closure calls `Timer.sleep` or a channel receive
- **THEN** compilation fails and names `RwLock<T>` or a redesign
### Requirement: RwLock uses FIFO writer-aware admission

`RwLock<T>` SHALL expose read-only and writable views only through synchronous
closures, SHALL be non-reentrant, and SHALL NOT expose poisoning or
upgrade/downgrade operations. Waiters SHALL be queued in arrival order. When no
writer is queued, new readers MAY join the active reader cohort; once a writer
queues, later readers SHALL NOT bypass it. On release, the queue head SHALL be
admitted; if it is a reader, the contiguous reader cohort ahead of the next
writer SHALL be admitted together. Cancellation SHALL remove a queued waiter or
revoke a pending handoff before waking the next eligible waiter. Panic or normal
return SHALL release the acquired view. A lock wait SHALL park the task through
the executor, and user code SHALL execute without holding the host bookkeeping
mutex.

There SHALL be no upgrade or downgrade API. A caller that needs to change from
read to write access SHALL release the read closure and make a separate write
request, accepting intervening updates.

#### Scenario: queued writer is not starved by later readers
- **WHEN** readers hold the lock, a writer queues, and more readers arrive
- **THEN** existing readers may finish, but later readers wait until the queued writer and its preceding reader cohort have been admitted and released

#### Scenario: cancelled RwLock waiter cannot strand a cohort
- **WHEN** a reader or writer is cancelled while queued or selected for handoff
- **THEN** its registration is removed and the next eligible waiter proceeds

#### Scenario: RwLock closure panic releases access
- **WHEN** a read or write closure panics
- **THEN** the lock releases that access and the next eligible waiter can proceed

#### Scenario: upgrade requires a fresh write request
- **WHEN** code needs write access after a read closure
- **THEN** it exits the read closure and requests write access separately, with no promise that the value remained unchanged between the two accesses
### Requirement: Semaphore permits are bounded, FIFO, and closeable

`Semaphore` SHALL be constructed with an initial permit count no greater than a
fixed maximum. `acquire` SHALL queue waiters FIFO and park through the executor;
`try_acquire` SHALL never park. A successful acquisition SHALL return an RAII
permit whose drop releases exactly one slot. Cancellation SHALL remove a queued
waiter or release a permit already granted but not yet consumed. `close` SHALL
be idempotent, reject future acquisitions, and wake all queued waiters with a
closed outcome; dropping a permit after close remains valid and SHALL NOT
reopen admission.

#### Scenario: release wakes the oldest semaphore waiter
- **WHEN** a permit is released while multiple tasks wait in FIFO order
- **THEN** the oldest eligible waiter proceeds before later waiters

#### Scenario: cancellation does not consume a semaphore permit
- **WHEN** a waiting task is cancelled before it consumes a granted permit
- **THEN** the permit is forwarded to the next waiter or returned to capacity

#### Scenario: close releases all semaphore waiters
- **WHEN** the semaphore closes while tasks are queued
- **THEN** all waiters wake with the closed outcome and no future acquisition succeeds
### Requirement: Barrier cancellation breaks its generation

`Barrier` SHALL use a fixed nonzero participant count and generation number.
Participants SHALL park through the executor until the configured count arrives;
exactly one participant (the last arrival) is the generation leader. If any
participant is cancelled while waiting, that generation SHALL become broken and
all waiters in it SHALL wake with `Broken`. A broken barrier SHALL remain
unusable; no participant may remain stranded in that generation.

#### Scenario: the last barrier participant releases one generation
- **WHEN** the configured number of participants arrives
- **THEN** all are released from that generation and exactly one receives the leader result

#### Scenario: cancellation breaks and wakes a barrier generation
- **WHEN** a participant is cancelled while the generation is incomplete
- **THEN** every waiter in that generation wakes with `Broken`, and later waits also return `Broken`
### Requirement: Once retries an initializer that panics

`Once<T>` SHALL publish one fully initialized value. Concurrent callers SHALL
wait through the executor until initialization succeeds or the initializer
panics. A panic SHALL publish no partial value, restore the cell to the
uninitialized state, wake waiters, and propagate to the initializing caller; one
waiter may then retry. Cancellation while waiting SHALL remove that waiter
without altering the initializer's state. The initialized value SHALL be
observable only through a scoped read closure.

#### Scenario: concurrent Once callers observe one value
- **WHEN** several tasks call `get_or_init` concurrently
- **THEN** one initializer succeeds and every caller observes the same published value

#### Scenario: Once panic permits a clean retry
- **WHEN** an initializer panics before publication
- **THEN** its caller receives the panic and a waiting caller can initialize without observing partial state
### Requirement: Atomic defaults to sequential consistency

`Atomic<T>` SHALL exist only for an explicit per-target matrix of supported boolean and integer widths. `Atomic(initial)` constructs an `Atomic<T>`; its scalar payload SHALL be stored inline in the Atomic object's GC-managed allocation, using target-correct alignment. The GC descriptor SHALL treat the payload as a non-reference scalar. The payload SHALL NOT be a separately allocated native handle, so collection of the Atomic object also ends its storage lifetime without a manual free operation.

The source API SHALL provide `load(order = AtomicOrder.seq_cst): T`, `store(value: T, order = AtomicOrder.seq_cst): Void`, `exchange(value: T, order = AtomicOrder.seq_cst): T`, `compare_exchange(expected: T, desired: T, success = AtomicOrder.seq_cst, failure = AtomicOrder.seq_cst): Result<T,T>`, and integer-only `fetch_add(value: T, order = AtomicOrder.seq_cst): T` / `fetch_sub(value: T, order = AtomicOrder.seq_cst): T`. `compare_exchange` SHALL return `Ok(observed)` when the comparison succeeds and `Error(observed)` when it fails. Integer fetch operations SHALL wrap modulo the selected width and return the previous value. Pointer atomics SHALL remain unsupported, including inside `unsafe`, until a separate rooting and lifetime protocol exists. `compare_exchange` SHALL validate success/failure order pairs. Operations SHALL default to sequentially consistent ordering.
`AtomicOrder.relaxed` / `.acquire` / `.release` / `.acq_rel` SHALL be accepted
only inside an `unsafe` context, where the programmer carries the ordering proof obligation.
Order legality is operation-specific: `load` accepts only `relaxed`, `acquire`,
or `seq_cst`; `store` accepts only `relaxed`, `release`, or `seq_cst`;
`exchange`, `fetch_add`, and `fetch_sub` accept all five orderings. A
compare-exchange failure ordering SHALL never be `release` or `acq_rel` and
SHALL NOT be stronger than its success ordering. A statically known invalid
ordering SHALL produce a compilation diagnostic. If an ordering supplied at
runtime is incompatible with its operation, the call SHALL raise
`InvalidArgumentError` before reading or mutating the atomic payload.

The initial runtime capability matrix is based on Rust 1.94.1 target
configuration. Each listed target advertises 8-, 16-, 32-, and 64-bit atomics;
The runtime `AtomicCell` currently represents Bool and signed/unsigned
integers of those widths; this runtime ABI is a capability/reference
implementation and SHALL NOT determine the source object's lifetime. Some
targets also advertise 128-bit atomics, but this initial source contract does
not represent them. `i386-unknown-linux-gnu` in the LLVM codegen list maps to
`i686-unknown-linux-gnu` for Rust target configuration. This capability report
does not by itself prove runtime compilation, linking, or execution on a target;
only installed Rust targets receive those checks.

| LLVM codegen triple | Rust target used for capability lookup | AtomicCell widths |
|---|---|---|
| `i686-pc-windows-msvc` | `i686-pc-windows-msvc` | 8, 16, 32, 64 |
| `x86_64-pc-windows-msvc` | `x86_64-pc-windows-msvc` | 8, 16, 32, 64 |
| `aarch64-pc-windows-msvc` | `aarch64-pc-windows-msvc` | 8, 16, 32, 64 |
| `i386-unknown-linux-gnu` | `i686-unknown-linux-gnu` | 8, 16, 32, 64 |
| `x86_64-unknown-linux-gnu` | `x86_64-unknown-linux-gnu` | 8, 16, 32, 64 |
| `armv7-unknown-linux-gnueabihf` | `armv7-unknown-linux-gnueabihf` | 8, 16, 32, 64 |
| `aarch64-unknown-linux-gnu` | `aarch64-unknown-linux-gnu` | 8, 16, 32, 64 |
| `x86_64-apple-darwin` | `x86_64-apple-darwin` | 8, 16, 32, 64 |
| `aarch64-apple-darwin` | `aarch64-apple-darwin` | 8, 16, 32, 64 |

Validation on Rust 1.94.1 installed the standard-library components for all
listed targets and passed both `cargo check -p zirk-runtime --tests --target
<target>` and `cargo build -p zirk-runtime --target <target>` for each row.
The same checks pass for `wasm32-unknown-unknown`. These checks establish
compilation and artifact generation only; they do not claim execution or
consumer linking on non-host targets.

#### Scenario: invalid compare-exchange order pair is rejected
- **WHEN** compare-exchange uses an invalid failure ordering (including release or acq_rel) or a failure order stronger than success
- **THEN** compilation rejects the operation

#### Scenario: pointer atomic cannot create a dangling reference
- **WHEN** code attempts to store/load a managed pointer without an explicit supported root and lifetime protocol
- **THEN** compilation rejects the safe operation

#### Scenario: relaxed ordering outside unsafe is rejected
- **WHEN** code requests `AtomicOrder.relaxed` outside `unsafe`
- **THEN** compilation fails because the proof obligation is explicit

#### Scenario: operation-incompatible constant ordering is rejected
- **WHEN** a statically known order requests `release` or `acq_rel` for `load`, `acquire` or `acq_rel` for `store`, or an invalid compare-exchange success/failure pair
- **THEN** compilation reports an invalid atomic ordering

#### Scenario: dynamic operation-incompatible ordering raises before access
- **WHEN** a runtime-supplied order is invalid for `load`, `store`, a read-modify-write operation, or a compare-exchange pair
- **THEN** `InvalidArgumentError` is raised before the atomic payload is read or changed
### Requirement: Concurrent.protect defers cancellation over non-suspending cleanup

`Concurrent.protect(fn)` SHALL support synchronous cleanup with complete, non-suspending, non-blocking, and non-spawning effects only. It SHALL preserve pending cancellation and deliver it after cleanup; it SHALL NOT make channel, timer, task, or thread waits immune to cancellation. The compiler rejects incomplete or disallowed transitive effect summaries. The compiler does not prove termination or a time bound, and the runtime does not preempt a non-suspending loop; cleanup code must terminate for cancellation to be delivered. No runtime unresolvable-wait detector is assumed.

#### Scenario: cleanup completes then cancellation is delivered
- **WHEN** a branch is cancelled while inside `Concurrent.protect` running synchronous non-suspending cleanup
- **THEN** cleanup completes and `CancelledError` is delivered at the next safe point after `protect` returns

#### Scenario: protected suspension is rejected
- **WHEN** a protected closure directly or transitively calls a channel wait, timer, spawn, or managed thread operation
- **THEN** compilation rejects the suspension effect so cleanup cannot wait indefinitely
### Requirement: Safe-code data-race freedom is fully enforced

The compiler SHALL enforce the `Transfer` and `Share` boundary rules at branch
captures, `concurrent` results, channel send and receive, the `parallel` region
boundary, and the `Thread.run` boundary. A concurrent unsynchronized access where
at least one access mutates shared state SHALL be a compile-time error naming
transfer, strict immutable sharing, cloning, a channel, or a synchronizer.

#### Scenario: parallel region mutates a shared alias
- **WHEN** a parallel region mutates an alias that remains mutable outside the region
- **THEN** compilation fails and identifies the required safe boundary

#### Scenario: two tasks mutate shared list
- **WHEN** two concurrent tasks mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap

#### Scenario: two branches mutate one list
- **WHEN** two concurrent branches mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap
