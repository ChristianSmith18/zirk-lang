## ADDED Requirements

### Requirement: Collection operations retain complete generic layouts in portable IR
Portable IR SHALL preserve the canonical concrete type and storage layout for every accepted `Map<K, V>` and `Set<T>` operation. Insert, lookup, membership, removal, iteration, algebra, copy, and managed-reference metadata SHALL retain multiword scalar, nullable, tuple/record, payload enum, union, and collection-reference layouts. Generic specialization SHALL preserve key, value, and element type arguments. The verifier SHALL reject mismatched layout metadata, and source combinations without a defined path SHALL be diagnosed before backend emission.

#### Scenario: Map stores a multiword value
- **WHEN** a `Map<K, V>` stores a `V` represented by multiple machine words and a later lookup retrieves it
- **THEN** verified IR carries the complete `V` layout through insertion and lookup without truncation or an untyped pointer cast

#### Scenario: Set hashes and compares a composite element
- **WHEN** a `Set<T>` inserts, checks, removes, or iterates a hashable tuple, payload enum, nullable, or tagged union element
- **THEN** IR operations retain the exact canonical `T` layout and the declared hash/equality dispatch

#### Scenario: Generic specialization keeps nested arguments
- **WHEN** a generic function operates on `Map<K, Tuple<A, B>>` or `Set<Enum<A>>`
- **THEN** specialization substitutes every nested type and emits verified typed collection operations

#### Scenario: Unsupported layout fails before backend emission
- **WHEN** a collection operation is requested for a type lacking a defined storage, hash/equality, or tracing path
- **THEN** compilation produces a source-located diagnostic and emits no malformed portable IR or native module
