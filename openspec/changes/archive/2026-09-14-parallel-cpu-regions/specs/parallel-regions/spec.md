## ADDED Requirements

### Requirement: The parallel block is a CPU-parallel region

The `parallel` block SHALL open a region in which `for` loops distribute their
iterations across worker threads and collection pipelines run in parallel. It
SHALL accept an optional `;`-separated `name: value` option header between the
keyword and the opening brace. The block MAY be used where an expression is
expected and SHALL then yield the value of its final expression. Outside a
`parallel` region and without the `.parallel` adapter, `for` and pipelines SHALL
run sequentially.

#### Scenario: parallel for spreads iterations
- **WHEN** `parallel { for f in files { compress(f) } }` runs on a machine with 8 cores
- **THEN** the `compress(f)` calls are distributed across worker threads

#### Scenario: parallel block as an expression
- **WHEN** `inmut total = parallel { rows.map(cost).sum() }` is evaluated
- **THEN** `total` is the parallel sum and the pipeline ran across cores

#### Scenario: ordered pipeline preserves input order
- **WHEN** a `.map` inside a `parallel` region finishes later elements before earlier ones
- **THEN** the resulting sequence still matches input order

### Requirement: The `cores` option

`cores: N` with `N > 0` SHALL use exactly `N` worker threads. `cores: -N` SHALL
use the detected core count minus `N`, and SHALL be a runtime error if that is
not positive. `cores: A..=B` SHALL let the runtime choose a count in `[A, B]`.
Absent, the region SHALL use all available cores. `cores: 0` SHALL be a
compile-time error. The option SHALL be evaluated once at region entry.

#### Scenario: leave a core free
- **WHEN** `parallel; cores: -1 { ... }` runs on an 8-core machine
- **THEN** the region uses 7 worker threads

#### Scenario: not enough cores
- **WHEN** `parallel; cores: -2 { ... }` runs on a 2-core machine
- **THEN** a runtime error is raised at region entry naming the detected core count

### Requirement: A `parallel` region performs no I/O and no suspension

A `parallel` region SHALL NOT contain a blocking channel operation, `Timer.sleep`,
`Timer.after` / `Timer.every`, `spawn`, `concurrent { }`, or a call statically
known to perform I/O. The compiler SHALL reject such a region.

#### Scenario: I/O in a parallel region is rejected
- **WHEN** a `parallel` region body calls `Http.get(...)` or `Timer.sleep(...)`
- **THEN** compilation fails, indicating `parallel` is for CPU work and directing I/O concurrency to `concurrent { }`

### Requirement: Transfer and Share at the `parallel` boundary

Values and projections SHALL copy into a `parallel` region; a strict immutable
complete reference MAY be shared; a mutable reference the enclosing scope keeps
using SHALL be rejected with the transfer / strict-share / clone / channel
resolutions. A parallel reduction SHALL require an associative combiner; a
`reduce_ordered` variant SHALL provide deterministic grouping.

#### Scenario: shared mutable alias into a region is rejected
- **WHEN** a `parallel` region mutates a list its enclosing scope also reads
- **THEN** compilation fails and names the resolutions

#### Scenario: non-associative parallel reduce is rejected
- **WHEN** `parallel { rows.reduce(0, (a, b) => a - b) }` is checked
- **THEN** compilation fails, requiring an associative combiner or `reduce_ordered`

### Requirement: `.parallel` adapter and `Parallel.each`

`collection.parallel` SHALL yield a parallel sequence exposing the parallel-safe
subset of the sequence API (`map`, `filter`, `reduce`, `sum`, `count`, `collect`,
`for_each`), running the chain across runtime-chosen cores. `Parallel.each(coll,
fn)` SHALL run `fn` per element in parallel and return `List<R>` in input order.

#### Scenario: adapter runs one chain in parallel
- **WHEN** `rows.parallel.map(cost).sum()` is evaluated outside a `parallel` block
- **THEN** the chain runs across cores and yields the same value as the sequential chain

#### Scenario: Parallel.each preserves order
- **WHEN** `Parallel.each([a, b, c], f)` is evaluated and `f(c)` finishes first
- **THEN** the result is `[f(a), f(b), f(c)]`
