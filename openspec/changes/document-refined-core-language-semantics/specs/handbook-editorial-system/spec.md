## ADDED Requirements

### Requirement: Cross-cutting core semantics remain synchronized
Every accepted callable, projection, object, generic, algebraic-data, collection, iteration, and matching rule SHALL be updated consistently in normative documents, owning handbook chapters, reference indexes, examples, source maps, and affected active planning artifacts. Historical archives SHALL remain historical.

#### Scenario: Projection rule audit
- **WHEN** projection-copy documentation is completed
- **THEN** repository-wide checks find no current claim that nested reference extraction aliases its container

### Requirement: Final semantics and implementation status are separate
Documentation SHALL state final language semantics independently from the compiler phase that delivers them and SHALL link undelivered features to status/roadmap material without weakening or contradicting the final rule.

#### Scenario: Escaping closures before implementation
- **WHEN** readers inspect Fn before its compiler phase ships
- **THEN** they see both the final legal behavior and an explicit implementation-status notice
