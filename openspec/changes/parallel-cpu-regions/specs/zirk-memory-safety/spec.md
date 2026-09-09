## MODIFIED Requirements

### Requirement: Root enumeration spans suspended tasks

Automatic memory management SHALL keep a reference reachable while any live
branch or worker thread can still reach it, whether running, ready, suspended at
a safe point, executing structured cleanup, or parked at a `parallel` safepoint.
Root enumeration SHALL walk one shadow-stack chain per live branch and one per
parked worker thread, including references in a compiler-managed temporary at a
suspension point, references queued inside a channel, and a `Job<T>` result that
has not been consumed.

#### Scenario: Suspended branch reference is not reclaimed
- **WHEN** a collection runs while a branch is suspended holding the only reference to an object
- **THEN** the object is retained and is intact when the branch resumes

#### Scenario: Parked worker thread stack is walked
- **WHEN** a stop-the-world collection runs during a `parallel` region and a worker holds the only reference to an object on its stack
- **THEN** the object is retained and is intact when the worker resumes

### Requirement: Single-threaded delivery still enforces the data-race analysis

The `Transfer` and `Share` analysis SHALL be enforced for the I/O concurrency
surface even though its executor is single-threaded, and SHALL additionally be
enforced at the `parallel` region boundary where real parallelism exists, so that
both a logical race across a suspension and a true data race across worker threads
are rejected at compile time.

#### Scenario: Logical race across a suspension is rejected
- **WHEN** two concurrent branches access one ordinary mutable collection with at least one mutation and without transfer or strict sharing
- **THEN** compilation fails regardless of the single-threaded executor

#### Scenario: True race across a parallel boundary is rejected
- **WHEN** a `parallel` region and its enclosing scope both mutate one collection
- **THEN** compilation fails and names the resolutions
