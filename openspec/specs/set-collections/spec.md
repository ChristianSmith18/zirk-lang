# set-collections Specification

## Purpose
TBD - created by archiving change map-set-collections. Update Purpose after archive.

## Requirements

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

### Requirement: Set handles and elements participate in managed reachability

Every `Set<T>` accepted by the compiler SHALL have a managed handle and element representation that preserves managed references in supported `T`. Set backing resources SHALL be reclaimed with the handle, and membership/removal SHALL follow the element type's equality and hashing semantics.

#### Scenario: Set stores the only reference to an element
- **WHEN** a set is the only reachable owner of a managed element and collection runs
- **THEN** the element remains valid until it is removed or the set is reclaimed

#### Scenario: Set element is rendered after collection
- **WHEN** a supported managed element remains in a reachable set through collection and is converted to text
- **THEN** rendering reads a live element and does not dereference reclaimed storage

#### Scenario: Mutable text element preserves its inserted value
- **WHEN** a `String` or `Char` element is inserted and the source `String` handle is later mutated
- **THEN** membership and rendering continue to use the normalized text value captured at insertion

#### Scenario: Unsupported set element is rejected
- **WHEN** an element type has no defined hash, equality, storage, or GC behavior
- **THEN** semantic analysis rejects it before code generation and names the unsupported operation
