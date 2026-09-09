## MODIFIED Requirements

### Requirement: Typing of optional, named, variadic parameters and default values

The checker SHALL preserve existing optional/named/variadic matching and SHALL type-check explicit spread arguments by requiring an `Iterable<T>`. Spread elements SHALL contribute `T` to collection element unification. A spread into fixed parameters SHALL require a statically provable compatible count; otherwise the target SHALL have a variadic tail.

#### Scenario: Spread element type
- **WHEN** `sum(...values)` is called with `values: List<Int32>`
- **THEN** every expanded argument is checked as `Int32`

#### Scenario: Non-iterable spread rejection
- **WHEN** `sum(...value)` is written and `value` is `Int32`
- **THEN** a diagnostic states that spread requires `Iterable<T>`

### Requirement: Parameter collection semantics

Every variadic parameter SHALL remain an ordered read-only `Iterable<T>`. A rest destructuring binding SHALL have the corresponding ordered collection type and SHALL not mutate the source collection.

#### Scenario: Rest binding type
- **WHEN** `[head, ...tail]` destructures a `List<Int32>`
- **THEN** `tail` has an ordered collection type whose element type is `Int32`

## ADDED Requirements

### Requirement: Object spread and rest typing

Object spread SHALL require a known nominal record/object shape, SHALL verify field compatibility and override types, and SHALL produce a value of that shape. Object rest destructuring SHALL bind selected fields to their declared types and the remainder to a record-compatible type containing exactly the unselected fields.

#### Scenario: Object spread type checking
- **WHEN** `{ ...profile, active: false }` is assigned to `UserProfile`
- **THEN** the expression has type `UserProfile` and `active` is `Boolean`

#### Scenario: Invalid object spread field
- **WHEN** an object spread adds an unknown field to a nominal record
- **THEN** a field/type diagnostic is emitted
