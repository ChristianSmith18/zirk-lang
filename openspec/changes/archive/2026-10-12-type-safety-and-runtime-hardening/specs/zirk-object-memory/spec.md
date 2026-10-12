## ADDED Requirements

### Requirement: Managed object descriptors match initialized storage

Every GC descriptor SHALL identify only initialized managed-reference fields at the offsets and representation used by the object layout. Construction SHALL initialize all storage that a collector may inspect before the object or value can become a root.

#### Scenario: Enum with an inactive reference payload is collected
- **WHEN** an enum value is constructed with a variant whose inactive variants contain managed references and collection runs
- **THEN** the collector reads only valid initialized references and does not interpret inactive payload storage as a pointer

#### Scenario: Compound value is embedded in a managed object
- **WHEN** a record, tuple, closure, nullable value, or enum containing managed references is stored in a managed object
- **THEN** the descriptor retains every live reference path represented by that value

### Requirement: Runtime collection handles use managed object allocation

Identity-bearing runtime collection handles that are included in compiler-generated roots SHALL carry a valid managed header and collector descriptor. Runtime-owned backing storage SHALL be reclaimed exactly once when its handle is reclaimed.

#### Scenario: Live map is traced
- **WHEN** a map handle is present in a live compiler root during collection
- **THEN** its header is valid, its entries are traced according to their type layout, and its backing storage remains usable

#### Scenario: Unreachable runtime collection is reclaimed
- **WHEN** a map or set handle becomes unreachable and collection sweeps it
- **THEN** its private backing storage is released exactly once without invoking user-visible finalization
