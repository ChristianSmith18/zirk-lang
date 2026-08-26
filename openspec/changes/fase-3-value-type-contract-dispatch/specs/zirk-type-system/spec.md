## MODIFIED Requirements

### Requirement: Value, enum, array, iteration, and generator semantics
Records SHALL be immutable with structural field equality; value classes SHALL be distinct domain types without observable identity and SHALL be storable inline; an unmapped traditional enum case SHALL expose its exact case name as its default string value and no implicit numeric index; all arrays SHALL have fixed length; `String` SHALL be iterable; and a generator SHALL be both `Iterator<T>` and `Iterable<T>` while preserving locals between yields. Holding a `record`/`value class` through a contract-typed reference SHALL NOT grant it observable identity or a mutation path back to the original value.

#### Scenario: Enum default and explicit mapping
- **WHEN** `Direction.North` has no mapping and `Code.North` maps to `"N"`
- **THEN** their observable mapped strings are `"North"` and `"N"` respectively, while both values retain their enum types

#### Scenario: Fixed inferred array
- **WHEN** an array literal contains three elements
- **THEN** its length is fixed at three and append/remove operations are rejected

#### Scenario: Structural field equality compares every field
- **WHEN** `==` compares two values of the same `record` or `value class` type
- **THEN** the result is the conjunction of each field's own equality, recursing into a nested `record`/`value class` field

#### Scenario: Structural field equality short-circuits on the first difference
- **WHEN** two `record`/`value class` values differ in their first field
- **THEN** `==` evaluates `false` without necessarily comparing the remaining fields

#### Scenario: A contract-typed view of a value type grants no new identity
- **WHEN** the same `record`/`value class` value is held through two separately-produced contract-typed references
- **THEN** `is` between them is not guaranteed `true`, and neither reference exposes a way to mutate the original value's storage
