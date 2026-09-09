## ADDED Requirements

### Requirement: App background scope and shutdown drain

The runtime SHALL create one `App.background` sub-scope of `main`'s implicit
scope. `Concurrent.detach(fn)` SHALL register `fn` as a branch of it. On `main`
completion or a shutdown signal the runtime SHALL cancel `App.background`, drain
it within `App.shutdown_grace` (default 5 seconds, settable), then exit. An
unhandled exception in a detached branch SHALL be delivered to
`App.on_background_error` (a settable `(Throwable): Void`, default logs to
stderr) and SHALL NOT abort the process.

#### Scenario: detached work is drained at shutdown
- **WHEN** `main` completes with one detached branch still running
- **THEN** the branch is cancelled and given up to the grace period to clean up before the process exits

#### Scenario: detached failure reaches the handler
- **WHEN** a detached branch throws and `App.on_background_error` is set
- **THEN** the handler receives the throwable and the process is not aborted

### Requirement: Thread.run join and the synchronizer runtime

The runtime SHALL spawn a fresh OS thread for `Thread.run`, register it with the
collector safepoint, suspend the calling branch, and resume it on join. The
runtime SHALL provide `Atomic<T>` over the target's native atomics, `Mutex<T>`
with FIFO fairness, and `RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>`.

#### Scenario: executor works during Thread.run
- **WHEN** one branch is inside `Thread.run(blocking_decode)` and another waits on a channel
- **THEN** the executor services the channel branch until the OS thread joins

#### Scenario: Once runs its initializer once
- **WHEN** several branches call `once.get_or_init(f)` concurrently
- **THEN** `f` runs exactly once and every caller observes the same value
