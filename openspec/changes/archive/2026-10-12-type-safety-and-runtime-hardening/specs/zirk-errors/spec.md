## ADDED Requirements

### Requirement: Exception-owned cached references follow exception reachability

Cached stack-trace strings and suppressed-exception references SHALL be traced as edges owned by their exception. They SHALL remain valid while the owner is reachable and SHALL be removed when that owner is reclaimed. A stale entry SHALL NOT be returned for a later object allocated at the same address.

#### Scenario: Reachable exception retains its stack trace
- **WHEN** a reachable exception has requested `stack_trace()` and collection runs
- **THEN** the cached string remains valid and repeated access returns the same cached value

#### Scenario: Dead exception cache entry is pruned
- **WHEN** an exception and its cache become unreachable and its address is later reused
- **THEN** lookup for the new exception cannot return the former exception's metadata
