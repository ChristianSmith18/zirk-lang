## ADDED Requirements

### Requirement: A concurrent branch's failure is distinct from `Result.Error`

An unhandled `Throwable` escaping a concurrent branch body SHALL fail that branch
and drive sibling-failure propagation in its scope. A branch that returns
`Result.Error(e)` SHALL be a completed branch carrying that value; the language
SHALL NOT convert it to a branch failure and SHALL NOT convert a branch failure
to a `Result.Error`.

#### Scenario: Returned error is a completed value
- **WHEN** a branch returns `Error(problem)` and a sibling is running
- **THEN** the sibling is not cancelled and the branch's value is `Error(problem)`

#### Scenario: Unhandled throwable fails the branch
- **WHEN** a branch body lets a custom exception escape
- **THEN** the branch fails, active siblings are cancelled and cleaned, and the exception propagates with secondary failures suppressed
