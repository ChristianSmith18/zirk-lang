## ADDED Requirements

### Requirement: Map key hashing and equality are coherent for supported recursive types

Every supported `Map<K, V>` key representation SHALL provide deterministic `Hash` and `Equal` semantics such that equal keys always produce equal hashes. The implementation SHALL compare `String` and `Char` components by Zirk content equality, include the active canonical tag and payload for union keys, compare nullable values by null state and payload, and use the declared derived or explicit capability for structural values. Mutable text reachable through a key SHALL be snapshotted at insertion so later mutation cannot change key identity. Mutable `Object` values SHALL NOT receive implicit recursive structural hashing; they require stable identity semantics or an explicit supported immutable value representation. A key whose nested component lacks these semantics SHALL be rejected in semantic analysis before lowering.

Collection `Equal` SHALL be a total equivalence relation. Floating keys SHALL compare by canonical width-specific bits, with signed zero normalized and all NaN encodings of that width treated as one key. Decimal keys SHALL compare by exact numeric value and hash from normalized coefficient/scale. Hashing and equality SHALL use identical recursive field order and union/enum active tags.

#### Scenario: Tuple text key uses content equality

- **WHEN** two tuple keys contain equal `String` or `Char` contents held by distinct or subsequently mutated text handles
- **THEN** lookup treats them as equal according to their inserted text snapshots and their hashes agree

#### Scenario: Union key includes its active alternative

- **WHEN** two union keys carry payloads that compare equal under different active alternatives
- **THEN** they remain distinct keys and their hashes include the active canonical alternative

#### Scenario: Nullable Decimal key distinguishes absence from value

- **WHEN** `Map<Decimal?, V>` receives `null`, a Decimal, and an equal Decimal represented by a different valid decimal scale
- **THEN** null is distinct from every Decimal and numerically equal Decimal keys share equality and hash behavior

#### Scenario: Unsupported nested key fails before backend emission

- **WHEN** a map key contains a nested component without defined stable equality, hashing, storage, or tracing
- **THEN** semantic analysis reports the offending type path and operation before IR or code generation

### Requirement: Map values accept every supported storable composite layout

A `Map<K, V>` SHALL accept each `V` whose concrete layout, copy/move behavior, ABI conversion, and managed-reference tracing are defined, independent of whether `V` itself is hashable. Nested nullable, tuple, enum, union, and structural value payloads SHALL use the same declared support matrix as the relevant type and runtime capabilities. Unsupported value layouts SHALL produce a source-located semantic diagnostic before backend emission.

#### Scenario: Composite managed map value remains usable

- **WHEN** a map stores a supported tuple, enum, nullable, or union value containing managed references
- **THEN** retrieval preserves the value and every referenced object remains live while reachable through the map
