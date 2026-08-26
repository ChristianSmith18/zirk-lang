## MODIFIED Requirements

### Requirement: Transactional unsafe mutation
An ordinary unsafe block SHALL isolate and journal writes to Zirk-managed state and validated native ranges, SHALL commit them on success, and SHALL close newly acquired resources and roll them back on a controlled `Error`, exception, runtime trap, or cancellation before commit.

#### Scenario: Validation fails after managed writes
- **WHEN** an unsafe block mutates managed state and then propagates a validation `Error`
- **THEN** the managed state is restored to its pre-block value before the error escapes

### Requirement: Unsafe transaction isolation
An unsafe transaction MUST NOT suspend, await, spawn a task or thread, or expose tentative state to another execution context, and entering `commit` SHALL publish pending reversible writes before irreversible effects execute.

#### Scenario: Await appears in unsafe transaction
- **WHEN** an ordinary transactional unsafe block contains `await`
- **THEN** compilation fails and requires completion or commit before suspension

#### Scenario: Commit publishes pending writes before an irreversible effect
- **WHEN** an `unsafe {}` block has journaled managed writes and then enters `commit {}`
- **THEN** every journaled write is durably committed before `commit`'s own body executes
