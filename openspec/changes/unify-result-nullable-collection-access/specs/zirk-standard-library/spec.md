## ADDED Requirements

### Requirement: Standard APIs use one outcome contract per operation
The standard library SHALL NOT publish a nullable or default-returning helper that duplicates an operation's `Result<T, E>` lookup or failure outcome. Callers MAY consume that Result through contextual projection and `??`, or preserve its typed error with an explicit Result context or `match`. A nullable-only API MAY remain when absence is its complete outcome and there is no parallel Result operation. When an operation has multiple successful states, its Result payload SHALL encode those states with a non-null domain value rather than using `T?` to distinguish success from absence.

#### Scenario: A Result lookup is consumed as nullable
- **WHEN** a caller uses an array, list, or map `get` in a value context expecting a nullable element
- **THEN** it receives the element or `null` without a separate `get_or_null` method

#### Scenario: A Result lookup retains diagnostic detail
- **WHEN** a caller needs to distinguish an absent element or key from other outcomes
- **THEN** it uses an explicit Result context or `match` and receives the typed error

#### Scenario: A nullable-only search remains available
- **WHEN** a sequence search such as `find(predicate)` has no match and no Result-returning equivalent
- **THEN** its nullable result remains the single API for that operation

#### Scenario: Multiple successful states remain distinct from failure
- **WHEN** a standard API can successfully report either a value or a domain state such as end-of-stream
- **THEN** its `Result` payload uses a non-null domain type to represent those successful states, while `Error` retains operational failure
