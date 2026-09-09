## ADDED Requirements

### Requirement: Lowering of parallel regions

The lowering SHALL wrap a `parallel` block in a `ParallelRegion` instruction
carrying the resolved core budget and the region body. A `for` loop inside a
region SHALL lower to a work-splitting loop that submits chunks to the worker
pool; a collection pipeline SHALL lower to parallel combinator calls preserving
input order for ordered operations. The lowering SHALL insert a safepoint-poll
instruction at every loop back-edge inside a region.

#### Scenario: parallel for lowers to a work-splitting loop
- **WHEN** `parallel { for f in files { compress(f); } }` is lowered
- **THEN** the IR contains a `ParallelRegion` wrapping a loop that submits chunks to the pool, with a safepoint poll at the back-edge

#### Scenario: parallel pipeline lowers to combinators
- **WHEN** `parallel { rows.map(f).sum() }` is lowered
- **THEN** the pipeline lowers to parallel map and reduce combinators under the `ParallelRegion`
