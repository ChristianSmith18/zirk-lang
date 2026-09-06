# map-collections Specification

## Purpose
TBD - created by archiving change map-set-collections. Update Purpose after archive.
## Requirements
### Requirement: `Map<K, V>` is a resolvable built-in type
The type system SHALL accept `Map<K, V>` for any hashable key type `K` and any value type `V`, and `K` and `V` SHALL be preserved through interning.

#### Scenario: Type reference resolves
- **WHEN** source code contains `mut m: Map<String, Int32>`
- **THEN** the checker resolves it to a distinct `Base::Map` type with `K = String` and `V = Int32`

### Requirement: `Map` construction produces a managed reference
The expression `Map()` SHALL create an empty `Map<K, V>` whose concrete types are taken from the surrounding expected type when not written explicitly.

#### Scenario: Empty map construction
- **WHEN** a variable of type `Map<String, Int32>` is initialized with `Map()`
- **THEN** the expression type is `Map<String, Int32>` and the result is a managed reference

### Requirement: Map supports insertion, lookup, and membership
The checker SHALL dispatch `m.set(key, value)` to `Void`, `m.get_or_null(key)` to `V?`, and `m.contains_key(key)` to `Boolean` for a `Map<K, V>` receiver.

#### Scenario: Insert and retrieve
- **WHEN** `m: Map<String, Int32>` is initialized with `Map()`, `m.set("one", 1)` is called, and `m.get_or_null("one")` is used
- **THEN** `set` is accepted with `String` and `Int32`, and `get_or_null` yields `Int32?`

### Requirement: Key types must be hashable
The type checker SHALL reject `Map<K, V>` when `K` does not satisfy the `Hash` and `Equal` contracts.

#### Scenario: Non-hashable key rejected
- **WHEN** a variable of type `Map<List<Int32>, Int32>` is declared
- **THEN** the checker reports a `TYPE_MISMATCH` because `List<Int32>` does not satisfy the key contracts

### Requirement: Map properties are accessible
A `Map<K, V>` value SHALL expose `length` as `Int32` and `is_empty` as `Boolean`.

#### Scenario: Read map length
- **WHEN** `m.length` is read for a `Map<String, Int32>`
- **THEN** the expression type is `Int32`

