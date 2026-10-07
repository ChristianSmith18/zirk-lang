## ADDED Requirements

### Requirement: Deep clone executes on the interpreter heap

The interpreter SHALL execute deep `clone()` with the graph semantics of the language specification: every reference object reachable from the receiver through cloneable members SHALL receive a new identity, sharing inside the source graph SHALL be preserved inside the cloned graph, cycles SHALL terminate and mirror the source cycle among new identities, and the runtime class of each object SHALL decide its layout regardless of the static receiver type. The operation SHALL grant no host authority.

#### Scenario: Clone is independent of the source
- **WHEN** an interpreted program clones an object and then mutates a field of a nested object in the clone
- **THEN** the corresponding field in the source graph is unchanged and the output matches native execution

#### Scenario: Shared child stays shared
- **WHEN** two fields of the cloned root refer to one child before cloning
- **THEN** the cloned fields refer to one new child distinct from the source child

#### Scenario: Cyclic graph
- **WHEN** a cloned object graph contains a field that refers back to an ancestor
- **THEN** the clone completes without unbounded recursion and the cloned cycle refers to the new ancestor

#### Scenario: Clone through a supertype reference
- **WHEN** a program clones a subclass instance through a variable declared with its superclass type
- **THEN** the clone has the subclass's runtime type and fields

#### Scenario: Collections and callables inside the graph
- **WHEN** the cloned graph holds a `List`, `Map`, `Set`, or closure that references other cloned objects
- **THEN** the cloned containers and closure environments reference the new objects

### Requirement: Deep clone work is bounded

Deep clone SHALL be charged against the configured instruction budget and heap limit before it allocates, and exhausting either SHALL end execution with the existing `limit-exceeded` result without a partially visible clone and without mutating the source graph.

#### Scenario: Clone exceeds the heap limit
- **WHEN** cloning a graph would exceed the configured heap limit
- **THEN** execution ends with a `limit-exceeded` result naming `heap`
- **AND** the source graph is unchanged

#### Scenario: Clone exceeds the instruction budget
- **WHEN** the work to clone a graph would exceed the remaining instruction budget
- **THEN** execution ends with a `limit-exceeded` result naming `instructions`
