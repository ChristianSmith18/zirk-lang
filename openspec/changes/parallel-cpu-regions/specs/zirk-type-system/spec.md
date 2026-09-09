## ADDED Requirements

### Requirement: Parallel block and adapter typing

A `parallel` block used as an expression SHALL be typed as its final expression.
A `cores` option operand SHALL be `Int` or an inclusive `Int` range; a `chunk`
operand SHALL be `Int`. `collection.parallel` SHALL be typed `ParallelSeq<T>`
exposing the parallel-safe subset of the sequence API (`map`, `filter`, `reduce`,
`sum`, `count`, `collect`, `for_each`). `Parallel.each(coll, fn)` SHALL be typed
`List<R>` for `fn: (T): R`. A parallel `reduce` combiner that the checker cannot
establish as associative SHALL be rejected unless `reduce_ordered` is used.

#### Scenario: parallel block expression type
- **WHEN** `inmut total = parallel { rows.map(cost).sum() }` is checked and `cost` returns `Int64`
- **THEN** `total` has type `Int64`

#### Scenario: cores operand type
- **WHEN** `parallel; cores: "four" { ... }` is checked
- **THEN** compilation fails naming the required `Int` or `Int` range

#### Scenario: adapter chain type
- **WHEN** `rows.parallel.map(cost).sum()` is checked
- **THEN** the chain is typed exactly as the sequential `rows.map(cost).sum()`
