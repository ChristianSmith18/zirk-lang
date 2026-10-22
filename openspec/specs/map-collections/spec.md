# map-collections Specification

## Purpose
Defines the built-in Map and Set types, their construction, key and value
contracts, lookup outcomes, mutation, membership, and managed runtime behavior.

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
The checker SHALL dispatch `m.set(key, value)` to `Void`, `m.get(key)` to `Result<V, LookupError>`, and `m.contains_key(key)` to `Boolean` for a `Map<K, V>` receiver. Nullable value contexts SHALL project `get` to `V?`; `get_or_null` SHALL NOT be exposed.

#### Scenario: Insert and retrieve
- **WHEN** `m: Map<String, Int32>` is initialized with `Map()`, `m.set("one", 1)` is called, and `m.get("one")` is used
- **THEN** `set` is accepted with `String` and `Int32`, and `get` yields `Result<Int32, LookupError>` or `Int32?` in a nullable value context

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

### Requirement: Map handles and entries participate in managed reachability

Every `Map<K, V>` accepted by the compiler SHALL have a managed handle and runtime entry representation that preserves all managed references in supported keys and values. Map backing resources SHALL be reclaimed with the handle, and hashing/equality SHALL follow the declared key type.

#### Scenario: Map stores the only reference to a value
- **WHEN** a map is the only reachable owner of a managed value and collection runs
- **THEN** the value remains valid until the map entry is removed or the map is reclaimed

#### Scenario: Map stores a managed key
- **WHEN** a supported managed key is inserted and later used for lookup or rendering after collection
- **THEN** the key remains valid and lookup follows the key type's equality semantics

#### Scenario: Mutable text key preserves its inserted value
- **WHEN** a `String` or `Char` key is inserted and the source `String` handle is later mutated
- **THEN** the map retains the normalized text value from insertion, lookup hashes that immutable key snapshot, and rendering does not expose the mutated source handle

#### Scenario: Unsupported map shape reaches semantic analysis
- **WHEN** a key or value has no defined runtime representation
- **THEN** compilation fails with a semantic diagnostic rather than a backend panic or pointer reinterpretation
