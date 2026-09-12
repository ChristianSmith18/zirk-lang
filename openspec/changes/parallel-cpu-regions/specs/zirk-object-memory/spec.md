## ADDED Requirements

### Requirement: Collector triggering and root enumeration are thread-aware

When the worker pool has active threads, the runtime SHALL NOT collect inline at
the allocation site. An allocation that crosses the GC threshold — on a worker
thread or the executor thread — SHALL raise a global "collection requested" flag.
Collection SHALL proceed only once every thread has parked at a safepoint, at
which point one thread SHALL enumerate roots across the executor's current branch
chain, every suspended branch chain, and each parked worker thread's
shadow-stack chain before marking and sweeping the single shared allocation list
under a lock. With no active worker threads, inline cooperative triggering at the
allocation site is unchanged.

The object header SHALL be unchanged: the 3-word layout (dispatch descriptor,
intrusive `next` pointer with the mark bit in its low bit, allocation size) and
every site that reads the descriptor are untouched by thread-aware triggering.

#### Scenario: Allocation during a parallel region defers collection
- **WHEN** a worker thread's allocation crosses the GC threshold inside a `parallel` region
- **THEN** the runtime raises the request flag instead of collecting, and collection runs only after every thread parks at a safepoint

#### Scenario: Object header is unchanged by the multi-threaded collector
- **WHEN** an object is allocated and inspected after `parallel-cpu-regions` lands
- **THEN** its header has the same 3-word layout and field offsets as before

#### Scenario: Single-threaded programs keep inline triggering
- **WHEN** a program uses no `parallel` region and an allocation crosses the GC threshold
- **THEN** the runtime collects inline at the allocation site as before
