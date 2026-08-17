## ADDED Requirements

### Requirement: Memory and concurrency implementation order
The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured task semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts task scheduling
- **THEN** typed scopes, cancellation, transfer/share analysis, and cleanup behavior are already specified as prerequisites
