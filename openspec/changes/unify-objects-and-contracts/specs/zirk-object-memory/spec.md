## REMOVED Requirements

### Requirement: Managed object descriptors match initialized storage
**Reason**: Its scenario list includes inline record references and does not describe dynamic property storage.
**Migration**: Trace initialized managed references in class fields, dynamic object properties, tuples, enums, closures, nullable values, and collections.

## ADDED Requirements

### Requirement: Dynamic object properties are managed references
An `Object` SHALL have stable reference identity and managed storage for a dynamically sized set of string-keyed properties. The runtime SHALL preserve each property's tagged Zirk value and trace every nested managed reference during collection. Contract conformance SHALL preserve object identity and make the complete current key set immutable across aliases.

Class instances SHALL retain their fixed declared-field layout and store properties added through an `Object` view in a sidecar associated with the same managed identity. The collector SHALL trace sidecar values while the owner is live and release the sidecar when the owner is swept.

#### Scenario: Nested object survives collection
- **WHEN** a live object property is the only path to a nested managed object and collection runs
- **THEN** the nested object remains reachable

#### Scenario: Contract view shares the allocation
- **WHEN** a dynamic object is assigned to a contract-typed alias
- **THEN** both references designate the same allocation and observe the same property values

#### Scenario: Sealed key set remains stable
- **WHEN** collection or aliasing occurs after contract conformance
- **THEN** all existing keys remain traceable and no new or removed key is observable

### Requirement: Managed descriptors trace only initialized references
Every garbage-collection descriptor SHALL identify only initialized managed references at their runtime representation and storage locations. Construction SHALL initialize every location that collection may inspect before the object can become a root.

#### Scenario: Dynamic property contains managed references
- **WHEN** an object property stores a nested managed object and collection runs
- **THEN** the nested object is traced while the containing object is reachable

#### Scenario: Inactive enum payload is not traced
- **WHEN** a live enum selects one variant and another variant has reference-typed fields
- **THEN** collection preserves the selected variant's references and safely ignores inactive storage
