## ADDED Requirements

### Requirement: Cancellation is a cleanup-aware structured exit

Cancellation SHALL enter the compiler's structured cleanup path rather than unwind across generated language frames as an opaque host panic. A cancelled branch SHALL be reported terminal only after its lexical cleanup has completed.

#### Scenario: Cancellation crosses nested cleanup scopes
- **WHEN** cancellation leaves nested `try/finally`, resource, and unsafe scopes
- **THEN** cleanup executes from the innermost scope outward and the branch reports cancellation afterward

#### Scenario: Cancellation is observed after cleanup
- **WHEN** a job is cancelled while it owns language resources
- **THEN** `Job` completion becomes observable only after cleanup and rollback obligations finish
