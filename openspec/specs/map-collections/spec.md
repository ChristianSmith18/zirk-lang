# map-collections Specification

## Purpose
Defines the built-in Map and Set types, their construction, key and value
contracts, lookup outcomes, mutation, membership, and managed runtime behavior.

## Requirements

### Requirement: `Map<K, V>` is a resolvable built-in type
The type system SHALL accept `Map<K, V>` when `K` satisfies the `Hash` and `Equal` contracts and `V` is a storable first-class runtime type with defined storage, projection, and managed-reference behavior. `K` and `V` SHALL be preserved through interning and generic specialization. `Void`, `Never`, compile-time-only types, and values with no defined storage or projection semantics SHALL be rejected as collection arguments with a source diagnostic.

#### Scenario: Type reference resolves
- **WHEN** source code contains `mut m: Map<String, Int32>`
- **THEN** the checker resolves it to a distinct `Base::Map` type with `K = String` and `V = Int32`

#### Scenario: Storable aggregate value type resolves
- **WHEN** source code declares a map whose value is a storable tuple, nullable value, payload enum, or union
- **THEN** the checker preserves the complete canonical value type through interning and accepts the declaration

#### Scenario: Non-value type is rejected
- **WHEN** source code declares a map with `Void`, `Never`, or a type with no defined storage and projection behavior as `V`
- **THEN** semantic analysis reports a source-located collection type diagnostic before lowering

### Requirement: `Map` construction produces a managed reference
The expression `Map()` SHALL create an empty `Map<K, V>` whose concrete types are taken from the surrounding expected type when not written explicitly.

#### Scenario: Empty map construction
- **WHEN** a variable of type `Map<String, Int32>` is initialized with `Map()`
- **THEN** the expression type is `Map<String, Int32>` and the result is a managed reference

### Requirement: Map supports insertion, lookup, and membership
The checker SHALL dispatch `m.set(key, value)` to `Void`, `m.get(key)` to `Result<V, LookupError>`, and `m.contains_key(key)` to `Boolean` for a `Map<K, V>` receiver. Nullable value contexts SHALL project `get` to `V?`; explicit Result contexts and `match` SHALL preserve `LookupError`. The `get_or_null` alias SHALL NOT be exposed.

#### Scenario: Insert and retrieve
- **WHEN** `m: Map<String, Int32>` is initialized with `Map()`, `m.set("one", 1)` is called, and `m.get("one")` is used
- **THEN** `set` is accepted with `String` and `Int32`, and `get` yields `Result<Int32, LookupError>` or `Int32?` in a nullable value context

### Requirement: Key types must be hashable
The type checker SHALL reject `Map<K, V>` when `K` does not satisfy the `Hash` and `Equal` contracts.

#### Scenario: Non-hashable key rejected
- **WHEN** a variable of type `Map<List<Int32>, Int32>` is declared
- **THEN** the checker reports a `TYPE_MISMATCH` because `List<Int32>` does not satisfy the key contracts

### Requirement: Map properties are accessible
A `Map<K, V>` value SHALL expose `length` as `UInt64` and `is_empty` as `Boolean`.

#### Scenario: Read map length
- **WHEN** `m.length` is read for a `Map<String, Int32>`
- **THEN** the expression type is `UInt64` and the value is the number of present key/value associations

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

### Requirement: Built-in and derived keys have coherent hashing and equality
All integer widths, `Boolean`, `Char`, `String`, `Decimal`, `Float16`, `Float32`, `Float64`, `Float128`, `Duration`, `Date`, `Time`, and `DateTime` SHALL provide coherent `Hash` and `Equal` behavior as map keys. Floating-point equality SHALL treat positive and negative zero as equal and hash them identically; `NaN` SHALL remain invalid under the scalar contract. Nullable values with a defined runtime layout SHALL distinguish null from non-null values, including nullable text keys; nullable forms without a runtime layout SHALL be rejected before lowering. Tuples, closed records, payload enums, and unions MAY derive both capabilities when all participating fields or alternatives have a canonical stored equality representation; derived union hashing/equality SHALL include the active tag. Mutable reference objects SHALL use identity semantics by default or require explicit stable implementations of both capabilities.

#### Scenario: Every built-in scalar key width is supported
- **WHEN** a map uses an integer width, Boolean, text, temporal value, Decimal, or Float16/32/64/128 key with defined Hash and Equal semantics
- **THEN** insertion, lookup, membership, removal, and rendering use the declared key type without narrowing its representation

#### Scenario: Floating signed zero obeys equality coherence
- **WHEN** positive and negative zero are used as Float keys
- **THEN** they compare equal, produce the same hash, and identify one association

#### Scenario: Derived aggregate key uses all active fields
- **WHEN** a tuple, structural aggregate, payload enum, nullable, or union key derives Hash and Equal
- **THEN** equality and hashing use the same fields and active alternative, and equal keys retrieve the same association

#### Scenario: Nullable text keys use content equality and preserve null
- **WHEN** a map uses `String?` or `Char?` keys containing null and text values
- **THEN** null keys remain distinct from text keys, equal normalized text keys address the same association, and key snapshots preserve the nullable layout

#### Scenario: Nullable keys without a runtime layout are rejected
- **WHEN** a map uses a nullable type for which Zirk defines no runtime layout
- **THEN** semantic analysis reports an unsupported key diagnostic before lowering


### Requirement: Map storage preserves each generic value layout
Every accepted `Map<K, V>` SHALL store and retrieve keys and values according to their complete Zirk type layouts. Storage SHALL preserve scalar width, nullability, tuple and structural aggregate fields, enum discriminants and payloads, union tags and active payloads, collection handles, class identity, and all managed references. It SHALL NOT truncate an entry to one machine word or reinterpret payload bytes as pointers. Key hashing and equality SHALL satisfy the declared `Hash` and `Equal` implementations, including equal-values-have-equal-hashes coherence.

#### Scenario: Aggregate values round-trip
- **WHEN** a map stores and retrieves a tuple, nullable value, payload enum, or structural union value
- **THEN** the retrieved value has the same active variant, fields, null state, and scalar bits as the inserted value

#### Scenario: Nested managed references remain live
- **WHEN** a map contains a value with a nested managed reference and the map is its only owner
- **THEN** collection retains the reference while the association exists and reclaims it after the association is removed and no other owner remains

#### Scenario: Nullable value distinguishes absence
- **WHEN** `Map<K, V?>` contains a key mapped to `null` and another key is absent
- **THEN** `get` distinguishes the present-null value from `LookupError` in an explicit `Result<V?, LookupError>` context


### Requirement: Map traversal is insertion ordered and typed
`Map<K, V>` SHALL implement `Iterable<Tuple<K, V>>` in insertion order and SHALL expose `keys()`, `values()`, and `entries()` iterables with element types `K`, `V`, and `Tuple<K, V>` respectively. Updating an existing key SHALL retain its iteration position; removing and reinserting a key SHALL place it at the end. Iterators SHALL follow the collection invalidation contract. `remove(K): Boolean` SHALL report whether an association existed; `clear(): Void` SHALL remove every association. The existing signatures SHALL remain `set(K, V): Void`, `get(K): Result<V, LookupError>`, and `contains_key(K): Boolean`. Inserting a new entry when storage growth fails SHALL raise catchable `AllocationError` before mutation; replacing an existing value SHALL not change the key order.

#### Scenario: Map iterates in insertion order
- **WHEN** keys are inserted in the order `"b"`, `"a"`, `"c"` and the map is iterated
- **THEN** entries, keys, and values are yielded in the corresponding insertion order

#### Scenario: Updating and reinserting keys
- **WHEN** an existing key is updated, then removed, then inserted again
- **THEN** updating does not change its position and reinsertion places it after the remaining entries

#### Scenario: Map iterator is invalidated by structural mutation
- **WHEN** an entry is added or removed after a map iterator is created and before it is exhausted
- **THEN** the next advance returns `IteratorInvalidatedError`

#### Scenario: Clear empties the map
- **WHEN** `clear()` is called on a non-empty map
- **THEN** `length` becomes zero, `is_empty` becomes true, and every previous key lookup returns `LookupError`

#### Scenario: Remove reports membership
- **WHEN** `remove(key)` is called for a present or absent key
- **THEN** it returns `true` only when an association was removed, and a removed key can be reinserted at the end of iteration order

#### Scenario: Map insertion allocation failure is atomic
- **WHEN** inserting a new key cannot allocate storage
- **THEN** `AllocationError` is raised and the map's entries, length, and iteration order remain unchanged
