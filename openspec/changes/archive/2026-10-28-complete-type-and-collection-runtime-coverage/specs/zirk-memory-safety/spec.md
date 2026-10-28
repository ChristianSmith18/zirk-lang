## ADDED Requirements

### Requirement: Generic collection entries trace only their live managed fields
The collector SHALL trace each managed reference held by a live `Map<K, V>`, `Set<T>`, `Deque<T>`, `PriorityQueue<T>`, `Queue<T>`, or `Stack<T>` according to the complete concrete entry layout. Tracing SHALL recurse through tuples, records, nullable values, payload enums, and active union alternatives. It SHALL ignore scalar bits, inactive union storage, deleted entries, and hash/index metadata. Collection removal or reclamation SHALL release entry edges without leaving stale roots or side-table owners.

#### Scenario: Nested value in a map retains its reference
- **WHEN** a reachable map contains the only reference to an object nested in a tuple, enum payload, nullable, or union value and collection runs
- **THEN** the object remains live while the association exists

#### Scenario: Set retains a reference-bearing element
- **WHEN** a reachable set contains the only reference to an object nested in a hashable aggregate and collection runs
- **THEN** the object remains live until its element is removed or the set is reclaimed

#### Scenario: Inactive union bytes are not traced
- **WHEN** a live collection entry contains a union whose active alternative has no references and inactive payload bytes resemble a managed address
- **THEN** the collector does not treat those bytes as a live reference

#### Scenario: Removal releases nested edges
- **WHEN** the only collection entry referencing an object is removed and collection runs
- **THEN** the removed edge does not retain the object or leave a stale reference in collection metadata
