## MODIFIED Requirements

### Requirement: Single-threaded cooperative executor

The runtime SHALL provide one executor that runs on a single operating-system
thread and schedules branches cooperatively. A branch SHALL yield control only at
a defined safe point (`Timer.sleep`, a blocking channel operation, a timer wait,
or an explicit cancellation check); the executor SHALL NOT preempt a running
branch at an arbitrary instruction. The executor SHALL service ready branches
from a first-in-first-out ready queue and SHALL check timer deadlines once per
scheduling turn. The executor SHALL NOT expose branch priority as a
user-controlled value and SHALL prevent indefinite starvation of a ready branch.

The runtime SHALL additionally provide a fixed pool of operating-system worker
threads, separate from the executor thread, that runs `parallel` regions. A
branch entering a `parallel` region SHALL submit the region's work to the pool
and block cooperatively — the executor keeps servicing other branches — until the
region joins. Worker threads SHALL run only pure-CPU Zirk code with no safe
points and no I/O. The executor thread SHALL remain single-threaded; only the
worker pool is multi-threaded.

#### Scenario: Branch runs until it suspends
- **WHEN** a branch performs a long computation with no safe point
- **THEN** no other branch is scheduled until it reaches a safe point or completes

#### Scenario: Ready branches are served fairly
- **WHEN** several branches are ready at the same time
- **THEN** the executor runs them in first-in-first-out order and each makes progress

#### Scenario: Executor drives the program to completion
- **WHEN** `main` starts branches and reaches the end of its body
- **THEN** the executor runs until `main`'s body and every owned branch have finished (or ambient timers cancelled) before the process exits

#### Scenario: I/O branches keep running during a parallel region
- **WHEN** one branch is inside a long `parallel` region and another branch waits on a channel or timer
- **THEN** the executor still services the waiting branch while the worker pool runs the region

### Requirement: Garbage-collection roots span every live task

The runtime SHALL enumerate garbage-collection roots through one shadow-stack
chain per task, anchored in that task's control block. A collection SHALL walk
the chain of every task the executor still owns — ready, running, suspended, or
running structured cleanup — not only the running task's chain. A reference held
by a suspended task, including a reference that lives only in a compiler-spilled
temporary slot at the suspension point, SHALL NOT be reclaimed while that task is
alive. The object header SHALL be unchanged by this requirement.

While the worker pool has active threads, a collection SHALL NOT run inline: an
allocation that crosses the GC threshold SHALL raise a global request flag, every
thread SHALL park at a safepoint (worker threads at `parallel` loop back-edges,
the executor at its next scheduling turn), and one thread SHALL then walk every
task chain plus each parked worker's shadow-stack chain, mark, sweep the shared
allocation list under a lock, clear the flag, and release every thread. The
collector SHALL remain non-moving mark-sweep.

#### Scenario: Suspended task keeps its references alive

- **WHEN** a task holds the only reference to an object in a local variable, suspends at `await`, and a collection runs while another task allocates
- **THEN** the object survives the collection and is intact when the task resumes

#### Scenario: Reference only in a transient slot survives

- **WHEN** a task holds a reference solely in a compiler-spilled temporary across a suspension point and a collection runs
- **THEN** the reference is treated as a root and the object survives

#### Scenario: Finished task stops rooting its result

- **WHEN** a task has completed and its result has been consumed by `await`
- **THEN** the task's control block no longer roots any object and its stack is reclaimed

#### Scenario: Collection during a parallel region parks every thread

- **WHEN** an allocation-heavy `parallel` region crosses the GC threshold mid-region
- **THEN** every worker thread and the executor park at a safepoint, one collection walks every task chain and every parked worker's stack, and the region resumes with a correct result and no reclaimed live object
