## ADDED Requirements

### Requirement: Collector roots are validated managed references

The collector SHALL only inspect headers of addresses proven to identify a currently allocated managed object. Compiler-generated roots and managed field descriptors SHALL never classify raw native pointers or foreign runtime allocations as managed references. An invalid candidate SHALL NOT be dereferenced.

#### Scenario: Raw native pointer is not a collector root
- **WHEN** a safe frame contains a `Pointer<T>` value that points outside the managed heap
- **THEN** root enumeration does not interpret that address as a managed object

#### Scenario: Invalid descriptor candidate is encountered
- **WHEN** a candidate reference does not identify a current managed allocation
- **THEN** the collector reports a deterministic internal runtime failure without reading or writing through the candidate address

### Requirement: Runtime-owned reference edges remain reachable

Every managed reference stored in a supported collection, exception cache, closure environment, channel payload, or compiler-managed temporary SHALL remain reachable for the lifetime of its owning managed value or live execution context. Side tables SHALL be pruned when their owner is reclaimed and SHALL not retain owners indefinitely.

#### Scenario: Collection is the only owner of a value
- **WHEN** a collection is the only reachable value that contains a managed reference and collection runs
- **THEN** the referenced object remains valid until the collection removes the edge or becomes unreachable

#### Scenario: Exception metadata owner is reclaimed
- **WHEN** an exception with cached metadata becomes unreachable and collection runs
- **THEN** the exception and metadata may be reclaimed, and no stale cache entry can be observed after address reuse

### Requirement: Managed graph traversal is independent of native stack depth

Garbage-collector marking and deep cloning SHALL terminate for finite reachable graphs without using native call-stack depth proportional to graph depth. Deep clone SHALL continue to preserve cycles and shared-child identity.

#### Scenario: Deep acyclic object graph is marked
- **WHEN** collection traverses a finite object graph deeper than the native recursion stack can safely represent
- **THEN** marking completes or reports a controlled allocation failure without stack overflow

#### Scenario: Deep graph is cloned
- **WHEN** a finite clonable graph contains a long chain and shared or cyclic edges
- **THEN** cloning preserves graph identity relationships without native stack overflow
