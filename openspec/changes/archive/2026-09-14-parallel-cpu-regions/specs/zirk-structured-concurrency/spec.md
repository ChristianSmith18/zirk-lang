## MODIFIED Requirements

### Requirement: Parallel CPU operations
The `parallel` block SHALL represent finite CPU work distributed across worker
threads, SHALL reject blocking I/O, suspension, `spawn`, `concurrent { }`, and
`Timer` operations inside a region, SHALL reject unsynchronized shared mutation
across its boundary, SHALL preserve input order for ordered map-like operations,
and SHALL provide an explicit unordered variant when completion order is desired.
The `.parallel` adapter SHALL run one collection chain in parallel outside a
`parallel` block with a runtime-chosen core count.

#### Scenario: Parallel map finishes out of order
- **WHEN** later input elements finish before earlier elements inside a `parallel` region
- **THEN** the returned ordered collection still matches input order

#### Scenario: I/O inside a parallel region is rejected
- **WHEN** a `parallel` region body performs a channel operation or `Timer.sleep`
- **THEN** compilation fails and directs I/O concurrency to `concurrent { }`

### Requirement: Parallel reductions
A parallel reduction inside a `parallel` block or via `.parallel.reduce` SHALL
require an associative combiner, MAY regroup operations, and SHALL provide an
explicit deterministic variant (`reduce_ordered`) when grouping-sensitive results
are required.

#### Scenario: Floating reduction is parallel
- **WHEN** floating values are reduced with the ordinary parallel reduction
- **THEN** documentation and types do not promise bit-identical grouping to sequential evaluation

#### Scenario: Non-associative combiner is rejected
- **WHEN** `parallel { rows.reduce(0, (a, b) => a - b) }` is checked
- **THEN** compilation fails, requiring an associative combiner or `reduce_ordered`

### Requirement: Safe-code data-race freedom
Safe Zirk SHALL reject concurrent unsynchronized accesses when at least one
access mutates shared state, at a concurrent-branch boundary, a channel
operation, and the `parallel` region boundary, while making no guarantee that
independent branch completion order is deterministic.

#### Scenario: Two branches mutate a shared list
- **WHEN** two concurrent branches mutate one ordinary `List<T>` without transfer or synchronization
- **THEN** compilation fails regardless of whether testing happened to avoid overlap

#### Scenario: Parallel region mutates a shared alias
- **WHEN** a `parallel` region mutates a list its enclosing scope also reads
- **THEN** compilation fails and names transfer, strict immutable sharing, cloning, a channel, or a synchronizer
