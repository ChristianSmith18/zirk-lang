## ADDED Requirements

### Requirement: Map handles and entries participate in managed reachability

Every `Map<K, V>` accepted by the compiler SHALL have a managed handle and runtime entry representation that preserves all managed references in supported keys and values. Map backing resources SHALL be reclaimed with the handle, and hashing/equality SHALL follow the declared key type.

#### Scenario: Map stores the only reference to a value
- **WHEN** a map is the only reachable owner of a managed value and collection runs
- **THEN** the value remains valid until the map entry is removed or the map is reclaimed

#### Scenario: Map stores a managed key
- **WHEN** a supported managed key is inserted and later used for lookup or rendering after collection
- **THEN** the key remains valid and lookup follows the key type's equality semantics

#### Scenario: Mutable text key preserves its inserted value
- **WHEN** a `String` or `Char` key is inserted and the source `String` handle is later mutated
- **THEN** the map retains the normalized text value from insertion, lookup hashes that immutable key snapshot, and rendering does not expose the mutated source handle

#### Scenario: Unsupported map shape reaches semantic analysis
- **WHEN** a key or value has no defined runtime representation
- **THEN** compilation fails with a semantic diagnostic rather than a backend panic or pointer reinterpretation
