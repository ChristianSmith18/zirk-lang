## ADDED Requirements

### Requirement: The worker pool runs CPU regions off the cooperative executor

The runtime SHALL provide a fixed pool of operating-system worker threads,
separate from the single cooperative executor thread. A branch entering a
`parallel` region SHALL split the region's work, submit it to the pool, and block
cooperatively while the executor keeps running other I/O branches, until the
region joins. Worker threads SHALL run only pure-CPU Zirk code with no safe
points and no I/O.

#### Scenario: I/O branches keep running during a parallel region
- **WHEN** one branch is inside a long `parallel` region and another branch waits on a channel
- **THEN** the executor still services the channel branch while the pool works

#### Scenario: pool size follows the core budget
- **WHEN** a `parallel; cores: 4` region is entered on an 8-core machine
- **THEN** at most 4 worker threads run that region's work

### Requirement: Stop-the-world safepoint collection

A garbage collection triggered while worker threads are active SHALL raise a
global request flag rather than collecting inline. Every worker thread SHALL poll
a safepoint at `parallel` loop back-edges and park when the flag is set, and the
executor thread SHALL park at its next scheduling turn. When every thread is
parked, one thread SHALL walk all roots — the executor's current branch chain,
every suspended branch chain, and each parked worker's shadow-stack chain — then
mark, sweep the shared allocation list under a lock, clear the flag, and release
all threads. The collector SHALL remain non-moving mark-sweep and the object
header SHALL be unchanged.

#### Scenario: collection during a parallel reduce is safe
- **WHEN** an allocation-heavy `parallel` reduce crosses the GC threshold mid-region
- **THEN** all threads park at a safepoint, one collection runs over every thread's roots, and the reduce resumes with a correct result and no reclaimed live object

#### Scenario: no safepoint means no collection on that thread
- **WHEN** a worker thread is between two safepoints
- **THEN** no collection begins until it reaches the next safepoint and parks
