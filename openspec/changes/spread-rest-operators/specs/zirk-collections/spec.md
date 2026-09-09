## MODIFIED Requirements

### Requirement: Collection literal construction

`Array<T>` and `List<T>` literals and constructors SHALL accept scalar elements and explicit spread elements/arguments. Each spread source SHALL implement `Iterable<T>`, SHALL be consumed once, and SHALL expand in place before final allocation. Unannotated literals retain their existing default collection type; destination context selects `List<T>` when requested.

#### Scenario: Array constructor spread
- **WHEN** `Array(...values)` is constructed from `[1, 2, 3]`
- **THEN** the array contains three copied elements in order

#### Scenario: List literal spread
- **WHEN** `inmut values: List<Int32> = [...source]` is constructed
- **THEN** the list contains every source element and remains independently resizable

### Requirement: Record field spread is distinct from collection spread

Record/object spread SHALL copy named fields and SHALL preserve nominal record shape. It SHALL not be implemented by iterating a record as `Iterable<T>`, and collection spread SHALL not copy named fields.

#### Scenario: Field spread preserves record shape
- **WHEN** a `UserProfile` is constructed with `{ ...profile, name: "Grace" }`
- **THEN** the result remains a `UserProfile` with the source fields and the override
