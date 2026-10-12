## ADDED Requirements

### Requirement: Cancellation closes active resources through structured cleanup

Cancellation of a structured branch SHALL execute active `finally` blocks, close acquired resources in reverse acquisition order, and roll back any uncommitted unsafe journal before the branch is reported terminal. Cancellation SHALL NOT bypass language cleanup through a foreign panic unwind.

#### Scenario: Branch is cancelled while holding a resource
- **WHEN** cancellation reaches a branch with an acquired resource and a `finally` block
- **THEN** `finally` runs and the resource is closed before the branch becomes terminal

#### Scenario: Cancellation exits an uncommitted unsafe block
- **WHEN** a branch is cancelled inside an unsafe block with journaled writes
- **THEN** the journal is rolled back after inner cleanup and before cancellation is reported
