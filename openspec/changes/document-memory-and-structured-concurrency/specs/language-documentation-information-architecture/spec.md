## ADDED Requirements

### Requirement: Complete memory and unsafe path
The documentation SHALL lead from public automatic memory and reference behavior through weak/dependent references, native views, pointers, transactional unsafe rollback, irreversible commit, and undefined-behavior limits with valid and invalid examples.

#### Scenario: Developer prepares native interop
- **WHEN** a reader follows the memory and safety unit
- **THEN** they can identify which operations are safe, unsafe but reversible, irreversible, or fundamentally unrecoverable

### Requirement: Complete structured concurrency path
The documentation SHALL lead from tasks and await through scopes, failure, cancellation, timeout, aggregation, select, channels, transfer/share rules, parallel work, threads, synchronization, atomics, and data-race prevention.

#### Scenario: Developer designs concurrent workflow
- **WHEN** a reader follows the concurrency unit
- **THEN** they can choose an appropriate primitive and predict its lifetime, failure, cancellation, ordering, and sharing behavior
