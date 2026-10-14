## ADDED Requirements

### Requirement: Set handles and elements participate in managed reachability

Every `Set<T>` accepted by the compiler SHALL have a managed handle and element representation that preserves managed references in supported `T`. Set backing resources SHALL be reclaimed with the handle, and membership/removal SHALL follow the element type's equality and hashing semantics.

#### Scenario: Set stores the only reference to an element
- **WHEN** a set is the only reachable owner of a managed element and collection runs
- **THEN** the element remains valid until it is removed or the set is reclaimed

#### Scenario: Set element is rendered after collection
- **WHEN** a supported managed element remains in a reachable set through collection and is converted to text
- **THEN** rendering reads a live element and does not dereference reclaimed storage

#### Scenario: Mutable text element preserves its inserted value
- **WHEN** a `String` or `Char` element is inserted and the source `String` handle is later mutated
- **THEN** membership and rendering continue to use the normalized text value captured at insertion

#### Scenario: Unsupported set element is rejected
- **WHEN** an element type has no defined hash, equality, storage, or GC behavior
- **THEN** semantic analysis rejects it before code generation and names the unsupported operation
