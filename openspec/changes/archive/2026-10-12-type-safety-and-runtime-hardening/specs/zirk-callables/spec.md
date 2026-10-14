## ADDED Requirements

### Requirement: Callable captures remain visible to managed-memory tracing

Every managed reference captured by a closure SHALL remain represented in its environment descriptor, including when a closure is nested inside a supported tuple, record, enum payload, or collection element. Copies and clones SHALL preserve the correct capture edges.

#### Scenario: Closure is stored in a supported aggregate
- **WHEN** a reachable aggregate contains a closure as its only path to a captured object and collection runs
- **THEN** the captured object remains valid while the aggregate and closure remain reachable

#### Scenario: Closure clone is collected during construction
- **WHEN** collection runs while a closure clone is being constructed
- **THEN** partially built capture environments remain reachable until cloning completes or fails
