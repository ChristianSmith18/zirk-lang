## ADDED Requirements

### Requirement: Root enumeration spans suspended tasks

Automatic memory management SHALL keep a reference reachable while any live task
can still reach it, whether that task is running, ready, suspended at a safe
point, or executing structured cleanup. Root enumeration SHALL walk one
shadow-stack chain per task, including references that live only in a
compiler-managed temporary at a suspension point, references queued inside a
channel, and a `Task<T>` result that has not been consumed.

#### Scenario: Suspended task reference is not reclaimed

- **WHEN** a collection runs while a task is suspended holding the only reference to an object
- **THEN** the object is retained and is intact when the task resumes

#### Scenario: Channel-queued reference is not reclaimed

- **WHEN** a collection runs while a reference sits unreceived in a channel and no other reference exists
- **THEN** the reference is retained until it is received or the channel becomes unreachable

### Requirement: Transfer and Share govern concurrency boundaries

Safe Zirk SHALL derive non-forgeable `Transfer` and `Share` properties and SHALL
enforce them where a value crosses into or out of a concurrent execution context:
`task` creation and captures, `task scope` results, channel send and receive, and
`select` branch values. Value types and projections SHALL cross by copy. A
complete `inmut::strict` reference MAY be shared. An exclusive mutable complete
reference MAY be transferred, after which the origin SHALL NOT use it until it
returns through a structured result or a channel. A cloned reference SHALL cross
as an independent graph. A mutable alias that would stay usable by a parent while
a child can mutate it SHALL be rejected, with a diagnostic naming transfer,
strict immutable sharing, cloning, or a channel.

#### Scenario: Overlapping mutable alias across a task boundary

- **WHEN** a task captures a mutable reference that the parent keeps mutating
- **THEN** compilation fails and names the resolutions

#### Scenario: Transferred reference is unusable by the sender

- **WHEN** a parent transfers an exclusive mutable reference to a child and then reads it
- **THEN** compilation fails because the reference was transferred and has not returned

### Requirement: Single-threaded delivery still enforces the data-race analysis

The `Transfer` and `Share` analysis SHALL be enforced for roadmap Phase 5 steps
1 to 3 even though the executor is single-threaded, so that a logical race —
where one task observes shared mutable state, suspends, and resumes after another
task has mutated it — is rejected at compile time and so that later phases that
introduce real parallelism inherit an already-enforced analysis rather than a
retrofit.

#### Scenario: Logical race across a suspension is rejected

- **WHEN** two concurrent tasks access one ordinary mutable collection with at least one mutation and without transfer or strict sharing
- **THEN** compilation fails regardless of the single-threaded executor and regardless of whether an interleaving happened to be observed
