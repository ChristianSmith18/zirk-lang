## ADDED Requirements

### Requirement: Task-aware I/O and blocking isolation
Runtime I/O SHALL suspend tasks without occupying scheduler threads where the platform permits, SHALL support cancellation-safe cleanup, and SHALL route explicitly wrapped legacy blocking work to a separate pool.

#### Scenario: Task waits for file or socket readiness
- **WHEN** an authorized asynchronous I/O operation cannot complete immediately
- **THEN** the task suspends and the scheduler thread remains available for other work

### Requirement: Irreversible effects preserve permission enforcement
External effects issued from an unsafe commit region MUST still satisfy normal project permissions and operating-system validation.

#### Scenario: Unsafe network send lacks permission
- **WHEN** an unsafe commit region attempts a network send outside the approved scope
- **THEN** permission enforcement rejects it before the external effect occurs
