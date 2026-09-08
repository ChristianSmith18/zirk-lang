## ADDED Requirements

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
