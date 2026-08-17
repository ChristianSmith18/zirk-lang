## ADDED Requirements

### Requirement: Safety and concurrency source synchronization
The editorial system SHALL identify canonical owners for memory/unsafe and concurrency semantics and SHALL update all derivative handbook, reference, roadmap, example, and agent-context pages when those rules change.

#### Scenario: Unsafe rollback rule changes
- **WHEN** the canonical transactional unsafe rule is edited
- **THEN** pointer, unsafe, runtime, compiler, example, and reference pages are checked for contradictory wording
