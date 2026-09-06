# set-collections Specification

## ADDED Requirements

### Requirement: `Set<T>` is a resolvable built-in type
The type system SHALL accept `Set<T>` for any hashable element type `T`, and `T` SHALL be preserved through interning.

#### Scenario: Type reference resolves
- **WHEN** source code contains `mut s: Set<String>`
- **THEN** the checker resolves it to a distinct `Base::Set` type with `T = String`

### Requirement: `Set` construction produces a managed reference
The expression `Set()` SHALL create an empty `Set<T>` whose concrete `T` is taken from the surrounding expected type.

#### Scenario: Empty set construction
- **WHEN** a variable of type `Set<String>` is initialized with `Set()`
- **THEN** the expression type is `Set<String>` and the result is a managed reference

### Requirement: Set supports add, remove, and membership
The checker SHALL dispatch `s.add(value)` to `Boolean`, `s.remove(value)` to `Boolean`, and `s.contains(value)` to `Boolean` for a `Set<T>` receiver.

#### Scenario: Add and check membership
- **WHEN** `s: Set<String>` is initialized with `Set()`, `s.add("one")` is called, and `s.contains("one")` is used
- **THEN** `add` is accepted with `String` and `contains` yields `Boolean`

### Requirement: Element types must be hashable
The type checker SHALL reject `Set<T>` when `T` does not satisfy the `Hash` and `Equal` contracts.

#### Scenario: Non-hashable element rejected
- **WHEN** a variable of type `Set<List<Int32>>` is declared
- **THEN** the checker reports a `TYPE_MISMATCH` because `List<Int32>` does not satisfy the element contracts

### Requirement: Set properties are accessible
A `Set<T>` value SHALL expose `length` as `Int32` and `is_empty` as `Boolean`.

#### Scenario: Read set length
- **WHEN** `s.length` is read for a `Set<String>`
- **THEN** the expression type is `Int32`
