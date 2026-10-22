## MODIFIED Requirements

### Requirement: Indexing and safe access
String, Tuple, Array, and List SHALL accept normalized negative indexes. Direct missing index/key access SHALL produce a typed controlled exception. Safe `get` on Array/List SHALL return `Result<T, IndexOutOfBoundsError>` and safe `get` on Map SHALL return `Result<V, LookupError>`. These SHALL be the sole safe lookup methods for those operations; `get_or_null` SHALL NOT be published. In a nullable value context, `get` SHALL follow the language-wide Result projection rule and produce `T?` or `V?`; an explicit Result context or `match` SHALL preserve the typed lookup error.

#### Scenario: Dynamic heterogeneous tuple index
- **WHEN** a nonconstant variable indexes a heterogeneous Tuple
- **THEN** compilation fails because one result type cannot be selected

#### Scenario: A safe sequence lookup finds an element
- **WHEN** `items.get(index)` addresses an element within the normalized bounds
- **THEN** the operation produces `Ok(element)` and a nullable value context receives that element

#### Scenario: A safe sequence lookup is out of bounds
- **WHEN** `items.get(index)` addresses no element
- **THEN** an explicit Result context receives `Error(IndexOutOfBoundsError)` and a nullable value context receives `null`

#### Scenario: A safe map lookup misses
- **WHEN** `map.get(key)` has no matching key
- **THEN** an explicit Result context receives `Error(LookupError)` and a nullable value context receives `null`

#### Scenario: Collection lookup preserves its error on request
- **WHEN** a caller matches `items.get(index)` or stores it as `Result<T, IndexOutOfBoundsError>`
- **THEN** the original lookup success or typed error remains available

#### Scenario: Removed nullable lookup alias
- **WHEN** source calls `items.get_or_null(index)` or `map.get_or_null(key)`
- **THEN** compilation reports that the member is unavailable and recommends `get`, nullable projection, `??`, or `match`
