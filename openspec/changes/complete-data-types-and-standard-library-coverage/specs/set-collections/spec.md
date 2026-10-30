## ADDED Requirements

### Requirement: Set hashing and equality are coherent for supported recursive elements

Every supported `Set<T>` element representation SHALL provide deterministic `Hash` and `Equal` semantics such that equal elements always produce equal hashes. `String` and `Char` components SHALL use Zirk content equality; unions SHALL include the active canonical alternative and its payload; nullable values SHALL include null state and payload; structural values SHALL use only declared derived or explicit capabilities over stable values. Mutable text reachable through an element SHALL be snapshotted when inserted. Mutable `Object` values SHALL NOT receive implicit recursive structural hashing; they require stable identity semantics or an explicit supported immutable value representation. Any unsupported nested component SHALL be rejected during semantic analysis before lowering.

Collection `Equal` SHALL be a total equivalence relation. Floating elements SHALL compare by canonical width-specific bits, with signed zero normalized and all NaN encodings of that width treated as one element. Decimal elements SHALL compare by exact numeric value and hash from normalized coefficient/scale. Hashing and equality SHALL use identical recursive field order and union/enum active tags.

#### Scenario: Nested tuple text elements deduplicate by content

- **WHEN** two tuple elements contain equal `String` or `Char` contents from distinct or later-mutated text handles
- **THEN** the set stores one logical element and computes matching hashes from the inserted snapshots

#### Scenario: Union elements preserve their active tag

- **WHEN** two union elements have equal-looking payloads under different active canonical alternatives
- **THEN** they remain distinct elements and membership uses tag-aware equality and hashing

#### Scenario: Nullable Decimal elements are coherent

- **WHEN** a set receives null and Decimal values including numerically equal values with different valid decimal scales
- **THEN** null remains distinct, equal Decimal values deduplicate, and equality implies equal hashes

#### Scenario: Unsupported element fails before code generation

- **WHEN** an element contains a nested component without defined equality, hashing, storage, or tracing
- **THEN** semantic analysis reports its type path and unsupported operation before IR or code generation

### Requirement: Set elements accept every supported storable composite layout

A `Set<T>` SHALL store and trace every composite `T` for which equality, hashing, concrete storage, copy/move, ABI, and GC behavior are defined by the language capability matrix. Adding, removing, iterating, or collecting SHALL preserve active union payloads and all reachable managed references.

#### Scenario: Composite managed element survives mutation and collection

- **WHEN** a supported tuple, enum, nullable, or union element contains managed references and the set is mutated or garbage collection runs
- **THEN** remaining elements retain valid references and removed elements release their collection-owned references
