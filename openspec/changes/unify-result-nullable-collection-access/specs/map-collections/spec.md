## MODIFIED Requirements

### Requirement: Map supports insertion, lookup, and membership
The checker SHALL dispatch `m.set(key, value)` to `Void`, `m.get(key)` to `Result<V, LookupError>`, and `m.contains_key(key)` to `Boolean` for a `Map<K, V>` receiver. Nullable value contexts SHALL project `get` to `V?`; explicit Result contexts and `match` SHALL preserve `LookupError`. The `get_or_null` alias SHALL NOT be exposed.

#### Scenario: Insert and retrieve
- **WHEN** `m: Map<String, Int32>` is initialized with `Map()`, `m.set("one", 1)` is called, and `m.get("one")` is used
- **THEN** `set` is accepted with `String` and `Int32`, and `get` yields `Result<Int32, LookupError>` or `Int32?` in a nullable value context
