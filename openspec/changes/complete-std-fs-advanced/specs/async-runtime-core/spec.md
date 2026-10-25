## ADDED Requirements

### Requirement: Blocking filesystem work cooperates with the executor

Asynchronous filesystem host operations SHALL use bounded managed blocking admission and SHALL suspend the calling task while native work executes. The cooperative executor thread SHALL remain available to run ready tasks. Filesystem work SHALL share the runtime's process-wide managed worker ceiling and SHALL NOT create a second unbounded pool. Queue saturation SHALL use bounded backpressure or a typed admission failure. Work submission, completion, cancellation, and shutdown SHALL preserve rooted inputs and outputs until native workers can no longer access them.

#### Scenario: Filesystem operation suspends one task
- **WHEN** a task awaits an asynchronous file read that blocks in the host
- **THEN** that task suspends while the executor continues to schedule ready tasks

#### Scenario: Filesystem load reaches admission capacity
- **WHEN** worker and queue capacity is exhausted by managed host operations
- **THEN** a new filesystem operation is bounded by backpressure or receives the documented typed capacity failure
- **AND** the runtime does not exceed its process-wide worker limit

#### Scenario: Runtime shuts down with filesystem work queued
- **WHEN** application shutdown begins while filesystem operations remain queued or running
- **THEN** queued work is cancelled where it has not started, running work reaches a safe completion point, and all resources and roots are released before runtime shutdown completes
