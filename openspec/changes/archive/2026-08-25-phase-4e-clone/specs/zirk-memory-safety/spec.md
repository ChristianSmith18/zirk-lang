## MODIFIED Requirements

### Requirement: Deep clone graph semantics
Deep `clone()` SHALL create new identity for cloned reference objects, preserve internal sharing and cycles within the new graph, and fail at compile time when the declared graph contains a non-`Clone` resource, pointer, lock, task, or other member.

#### Scenario: Graph contains shared child
- **WHEN** two source fields refer to the same clonable child and the root is cloned
- **THEN** the two cloned fields refer to one new cloned child rather than the source child or two unrelated copies

#### Scenario: Graph contains a cycle
- **WHEN** a clonable object's field reaches back to an ancestor already being cloned in the same `clone()` call
- **THEN** the clone completes without infinite recursion and the cloned cycle mirrors the source cycle among new identities

#### Scenario: Graph reaches a non-Clone member
- **WHEN** a class declares a field of type `Resource`, `Pointer<T>`, a lock, `Task<T>`, or another type that is not `Clone`
- **THEN** compilation rejects deriving or using `Clone` for that class, naming the offending field
