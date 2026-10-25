## ADDED Requirements

### Requirement: Async filesystem resources participate in structured cleanup

Asynchronous filesystem operations, lock waits, and watcher waits SHALL be safe points for cooperative cancellation. A cancelled branch SHALL remove pending work that has not started, wait for or safely detach non-interruptible work without invalidating its roots or handles, close resources that cannot be delivered to the cancelled branch, release any lock acquired during a cancellation race, and close watchers before the owning scope completes. A `Result.Error` from a filesystem operation SHALL remain an ordinary branch value and SHALL NOT cancel sibling branches unless user code throws or explicitly cancels them.

#### Scenario: Cancel an async read branch
- **WHEN** a structured scope cancels a branch awaiting a file read
- **THEN** the operation reaches a safe completion/cleanup point and the scope does not finish while its worker still holds branch-owned data

#### Scenario: Filesystem Result error is returned normally
- **WHEN** one branch returns `Error(FileError)` while another branch is reading
- **THEN** the sibling is not cancelled merely because the first branch returned a Result error

#### Scenario: Cancel a watcher branch
- **WHEN** a watcher branch is cancelled while waiting for an event
- **THEN** it resumes with a cancellation outcome, closes watcher state, and finishes cleanup before scope completion

#### Scenario: Cancellation races with lock acquisition
- **WHEN** a cancelled branch and a lock release race
- **THEN** the lock is either never acquired or is acquired only under cleanup ownership and released before the branch becomes terminal
