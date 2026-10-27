## ADDED Requirements

### Requirement: Collection failures have typed, non-partial outcomes
The standard error hierarchy SHALL provide catchable `AllocationError` for a collection growth allocation that cannot be completed and `EmptyCollectionError` for a removal or inspection that requires an element from an empty collection. APIs specified to return `Result<T, EmptyCollectionError>` SHALL return that error without mutation. Collection growth APIs specified to return `Void` SHALL raise `AllocationError` before mutation. Both error types SHALL be catchable as their concrete type or as `RuntimeError`; they SHALL NOT abort the process, return fabricated values, or leave partially updated collection state.

#### Scenario: Empty removal is handled as a typed result
- **WHEN** a caller removes or inspects an item from an empty Deque, PriorityQueue, Queue, or Stack
- **THEN** the operation returns `Error(EmptyCollectionError)` and can be matched without an exception or process abort

#### Scenario: Collection allocation failure is catchable
- **WHEN** growth of a collection fails to reserve required storage
- **THEN** the operation raises `AllocationError` that a caller can catch, and the collection remains unchanged

#### Scenario: Collection errors share the runtime error hierarchy
- **WHEN** code catches `RuntimeError` around an empty operation or failed collection growth
- **THEN** the corresponding `EmptyCollectionError` or `AllocationError` is caught as that base type
