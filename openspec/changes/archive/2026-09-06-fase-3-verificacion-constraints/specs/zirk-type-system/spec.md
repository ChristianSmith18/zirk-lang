# Delta spec: zirk-type-system

## MODIFIED Requirements

### Requirement: Value, enum, array, iteration, and generator semantics

Records SHALL be immutable with structural field equality; an unmapped
traditional enum case SHALL expose its exact case name as its default string
value and no implicit numeric index; all arrays SHALL have fixed length;
`String` SHALL be iterable; and a generator SHALL be both `Iterator<T>` and
`Iterable<T>` while preserving locals between yields. Holding a `record`
through a contract-typed reference SHALL NOT grant it observable identity or
a mutation path back to the original value.

#### Scenario: Enum default and explicit mapping
- **WHEN** `Direction.North` has no mapping and `Code.North` maps to `"N"`
- **THEN** their observable mapped strings are `"North"` and `"N"` respectively, while both values retain their enum types

#### Scenario: Fixed inferred array
- **WHEN** an array literal contains three elements
- **THEN** its length is fixed at three and append/remove operations are rejected

#### Scenario: Structural field equality compares every field
- **WHEN** `==` compares two values of the same `record` type
- **THEN** the result is the conjunction of each field's own equality, recursing into a nested `record` field

#### Scenario: Structural field equality short-circuits on the first difference
- **WHEN** two `record` values differ in their first field
- **THEN** `==` evaluates `false` without necessarily comparing the remaining fields

#### Scenario: A contract-typed view of a record grants no new identity
- **WHEN** the same `record` value is held through two separately-produced contract-typed references
- **THEN** `is` between them is not guaranteed `true`, and neither reference exposes a way to mutate the original value's storage

#### Scenario: String iteration
- **WHEN** a string is consumed by `for ... in`
- **THEN** each element is a `Char`

### Requirement: Nominal types and subtyping

The checker SHALL treat every class, record and enum as a distinct nominal
type, and SHALL accept a value where one of its superclasses or an
implemented contract is expected.

#### Scenario: Subclass where base is expected
- **WHEN** an instance of `Admin` is passed to a parameter of type `User`
- **THEN** the check succeeds

#### Scenario: Implementation where contract is expected
- **WHEN** an instance is passed to a parameter whose type is a contract it implements
- **THEN** the check succeeds

#### Scenario: Two types with the same shape are not the same type
- **WHEN** two classes declare the same fields and one is assigned where the other is expected
- **THEN** a diagnostic is emitted: equivalence is by name, not by shape

#### Scenario: Base where subclass is expected
- **WHEN** an instance of `User` is passed to a parameter of type `Admin`
- **THEN** a diagnostic is emitted
