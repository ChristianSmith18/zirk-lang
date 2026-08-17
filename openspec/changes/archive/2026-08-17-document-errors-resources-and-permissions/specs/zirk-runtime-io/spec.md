## ADDED Requirements

### Requirement: Runtime preserves typed failure and cleanup
The runtime SHALL represent implicit safety failures as typed `RuntimeError` exceptions, preserve exact rethrows, lazily materialize structured traces, redact secrets, attach suppressed cleanup failures, and execute managed resource close exactly once on every exit path.

#### Scenario: Exception and close both fail
- **WHEN** a throwable is propagating and resource close reports an error
- **THEN** the throwable remains primary and the close failure appears in its suppressed list

### Requirement: Environment and external I/O enforce effective grants
Runtime I/O, environment, secret, network, process, and shell operations SHALL validate the signed effective policy and normalized dynamic target before performing an external effect. Denial SHALL produce the operation's typed permission error and SHALL NOT prompt or modify project files.

#### Scenario: Redirect leaves network scope
- **WHEN** an authorized HTTP request redirects to an unauthorized host
- **THEN** the redirect is denied before connecting to the new host
