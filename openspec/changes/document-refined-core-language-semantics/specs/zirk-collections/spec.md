## ADDED Requirements

### Requirement: Collection families and alias boundary
Array and fixed `T[n]` SHALL be ordered fixed-size references, List an ordered dynamic reference, Map a key/value reference without implicit order, Set a unique-membership reference without implicit order, Range a lazy value, and Tuple a heterogeneous value. Whole reference assignment SHALL alias; extraction SHALL deep-clone.

#### Scenario: List element extraction
- **WHEN** `mut first = users[0]` extracts a User and first is mutated
- **THEN** the stored users[0] remains unchanged

#### Scenario: Indexed place mutation
- **WHEN** `users[0].name = "Grace"` is assigned
- **THEN** the User stored in the list is mutated

### Requirement: Indexing and safe access
String, Tuple, Array, and List SHALL accept normalized negative indexes. Direct missing index/key access SHALL produce a typed controlled exception; safe `get` SHALL return typed Result and `get_or_null` MAY deliberately collapse absence with nullable value.

#### Scenario: Dynamic heterogeneous tuple index
- **WHEN** a nonconstant variable indexes a heterogeneous Tuple
- **THEN** compilation fails because one result type cannot be selected

### Requirement: Python-style slice components with strict bounds
String, Array, and List slices SHALL accept `[start:end:step]` with omitted positive defaults `(0,length,1)` and omitted negative defaults `(last,before-first,negative step)`. End SHALL be exclusive, zero step and explicitly out-of-range bounds SHALL fail, and slices SHALL deep-copy independent results. Tuple, Map, and Set SHALL not slice.

#### Scenario: Complete reverse slice
- **WHEN** `values[::-1]` is evaluated
- **THEN** it returns an independent reverse-order copy

#### Scenario: Omitted ordinary step
- **WHEN** `values[n:w]` is evaluated
- **THEN** its step is one

### Requirement: Slice replacement and structural APIs
Slice assignment SHALL require exactly equal element count and SHALL never resize. Array SHALL reject structural growth; List SHALL use explicit add/insert/remove/splice operations. Mutating operations SHALL require a non-strict mutable referent and SHALL report typed allocation/capacity errors.

#### Scenario: Unequal slice replacement
- **WHEN** a two-element slice is assigned three replacements
- **THEN** a controlled slice-length-mismatch error is produced without partial mutation

### Requirement: Iteration, invalidation, views, and equality
Iteration SHALL use `Iteration<T>.Item/Done`, yield independent copies, and make `Iterable<out T>` create a new iterator. Structural mutation SHALL invalidate existing iterators deterministically. Ordinary views SHALL be explicit read-only shared storage with bounded lifetime. Collection equality SHALL follow order for arrays/lists, mappings for Map, membership for Set, and range definition for Range.

#### Scenario: Structural invalidation
- **WHEN** a List grows after an iterator is created and that iterator advances
- **THEN** it produces IteratorInvalidatedError rather than observing stale storage
