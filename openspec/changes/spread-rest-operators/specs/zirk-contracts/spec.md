## MODIFIED Requirements

### Requirement: Iteration via contract

`Iterable<T>` SHALL be the source contract for explicit spread in calls, collection literals, and rest destructuring. Spread SHALL use the iterable's documented order and ordinary iteration projection-copy, invalidation, transfer, and sharing rules.

#### Scenario: User iterable spread
- **WHEN** an object implementing `Iterable<Int32>` is used as `...source`
- **THEN** its iterator supplies the expanded `Int32` values in order

## ADDED Requirements

### Requirement: Object spread does not require Iterable

Record/object spread and rest SHALL use named-field metadata and SHALL remain valid for records that do not implement `Iterable<T>`. Only collection/call spread requires the iterable contract.

#### Scenario: Non-iterable record field spread
- **WHEN** a record without `Iterable<T>` is used as `{ ...record }`
- **THEN** named fields are copied successfully according to its record shape
