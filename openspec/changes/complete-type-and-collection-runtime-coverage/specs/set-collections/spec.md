## MODIFIED Requirements

### Requirement: `Set<T>` is a resolvable built-in type
The type system SHALL accept `Set<T>` when `T` satisfies the `Hash` and `Equal` contracts and has defined storage, projection, and managed-reference behavior. `T` SHALL be preserved through interning and generic specialization. Types with no defined hash, equality, storage, or projection behavior SHALL be rejected with a source diagnostic.

#### Scenario: Type reference resolves
- **WHEN** source code contains `mut s: Set<String>`
- **THEN** the checker resolves it to a distinct `Base::Set` type with `T = String`

#### Scenario: Hashable aggregate element type resolves
- **WHEN** source code declares a set of a tuple or payload enum whose components satisfy `Hash` and `Equal`
- **THEN** the checker accepts the type and preserves its canonical element type through interning

#### Scenario: Element lacking collection semantics is rejected
- **WHEN** a type has no defined `Hash`, `Equal`, storage, or projection behavior
- **THEN** semantic analysis rejects `Set<T>` before lowering and names the missing capability

### Requirement: Set properties are accessible
A `Set<T>` value SHALL expose `length` as `UInt64` and `is_empty` as `Boolean`.

#### Scenario: Read set length
- **WHEN** `s.length` is read for a `Set<String>`
- **THEN** the expression type is `UInt64` and the value is the number of unique elements

## ADDED Requirements

### Requirement: Built-in and derived set elements have coherent hashing and equality
All integer widths, `Boolean`, `Char`, `String`, `Decimal`, `Float16`, `Float32`, `Float64`, `Float128`, `Duration`, `Date`, `Time`, and `DateTime` SHALL provide coherent `Hash` and `Equal` behavior as set elements. Floating-point equality SHALL treat positive and negative zero as equal and hash them identically; `NaN` SHALL remain invalid under the scalar contract. Nullable values with a defined runtime layout SHALL distinguish null from non-null values, including nullable text elements; nullable forms without a runtime layout SHALL be rejected before lowering. Tuples, closed records, payload enums, and unions MAY derive both capabilities when all participating fields or alternatives have a canonical stored equality representation; derived union hashing/equality SHALL include the active tag. Mutable reference objects SHALL use identity semantics by default or require explicit stable implementations of both capabilities.

#### Scenario: Every built-in scalar key width is supported
- **WHEN** a set stores an integer width, Boolean, text, temporal value, Decimal, or Float16/32/64/128 element with defined Hash and Equal semantics
- **THEN** insertion, membership, removal, and iteration use the declared element type without narrowing its representation

#### Scenario: Floating signed zero obeys equality coherence
- **WHEN** positive and negative zero are added as Float elements
- **THEN** they compare equal, produce the same hash, and only one element is stored

#### Scenario: Derived aggregate element uses all active fields
- **WHEN** a tuple, closed record, payload enum, nullable, or union element derives Hash and Equal
- **THEN** equality and hashing use the same fields and active alternative, and equal values are treated as one set member

#### Scenario: Nullable text elements use content equality and preserve null
- **WHEN** a set stores `String?` or `Char?` values containing null and text
- **THEN** null is distinct from text, equal normalized text values occupy one element, and iteration snapshots preserve the nullable layout

#### Scenario: Nullable elements without a runtime layout are rejected
- **WHEN** a set uses a nullable type for which Zirk defines no runtime layout
- **THEN** semantic analysis reports an unsupported element diagnostic before lowering

### Requirement: Set storage preserves each generic element layout
Every accepted `Set<T>` SHALL store, compare, hash, retrieve, and trace elements according to their complete Zirk type layouts. Hash and equality SHALL be coherent and deterministic for the lifetime of an inserted element. Union equality and hashing SHALL include the active alternative; nullable values SHALL distinguish `null` from non-null values; aggregate derivation SHALL compare and hash the same fields in the same semantic order. Mutable reference types SHALL require an explicit stable `Hash`/`Equal` implementation or use identity semantics. Entry storage SHALL NOT truncate values to one machine word or trace inactive payload bytes as references.

#### Scenario: Aggregate element round-trips
- **WHEN** a set stores a hashable tuple or payload enum and later iterates it
- **THEN** every field and active variant is preserved and membership recognizes an equal value

#### Scenario: Nullable and union elements remain distinct
- **WHEN** a set stores `null`, a non-null nullable value, or values from distinct union alternatives
- **THEN** membership and duplicate detection distinguish values according to nullable and tagged-union equality semantics

#### Scenario: Derived hash and equality are coherent
- **WHEN** a closed record or payload enum requests derived `Hash` and `Equal` and all participating fields implement both
- **THEN** derivation uses the same participating fields and equal values produce equal hashes

### Requirement: Set algebra and traversal are deterministic
`Set<T>` SHALL implement `Iterable<T>` in insertion order and SHALL expose `union`, `intersection`, `difference`, `symmetric_difference`, `is_subset_of`, `is_superset_of`, and `is_disjoint`. Algebraic operations SHALL return a new set without mutating either operand. `union` SHALL preserve the left operand's order and append new right-side elements in right-side order; `intersection` and `difference` SHALL preserve the left operand's order; `symmetric_difference` SHALL yield left-only elements followed by right-only elements in their respective insertion orders. `clear(): Void` SHALL remove every element. Structural mutation SHALL invalidate an existing iterator. Adding a new element when storage growth fails SHALL raise catchable `AllocationError` before mutation. Algebraic result construction SHALL raise the same error if it cannot allocate and SHALL leave both inputs unchanged.

#### Scenario: Set iteration preserves insertion order
- **WHEN** distinct elements are added in a known order and the set is iterated
- **THEN** elements are yielded in that order

#### Scenario: Set algebra preserves defined order and does not mutate inputs
- **WHEN** two ordered sets are combined with each algebraic operation
- **THEN** the result follows the specified operand ordering and both input sets remain unchanged

#### Scenario: Set relation queries compare membership
- **WHEN** subset, superset, or disjoint queries are applied to two sets
- **THEN** each returns the Boolean result defined by mathematical set membership

#### Scenario: Duplicate insertion reports no change
- **WHEN** an element already equal to one in the set is added
- **THEN** `add` returns `false`, the existing iteration position remains unchanged, and `length` is unchanged

#### Scenario: Clear empties the set
- **WHEN** `clear()` is called on a non-empty set
- **THEN** `length` becomes zero, `is_empty` becomes true, and membership checks for all previous elements return false

#### Scenario: Set growth failure is atomic
- **WHEN** adding a new element cannot allocate storage
- **THEN** `AllocationError` is raised and the set's members, length, and iteration order remain unchanged

#### Scenario: Set algebra allocation failure preserves operands
- **WHEN** an algebraic operation cannot allocate its result
- **THEN** it raises `AllocationError` and neither input set is changed
