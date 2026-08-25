## MODIFIED Requirements

### Requirement: Weak references
`Weak<T>` SHALL NOT keep its referent alive, SHALL require `upgrade(): T?` before safe use, and SHALL expose `is_alive: Boolean` only as an observational hint subject to concurrent change.

#### Scenario: Weak referent was reclaimed
- **WHEN** `upgrade()` is called after no strong reference keeps the referent alive
- **THEN** it returns `null` rather than exposing reclaimed memory
