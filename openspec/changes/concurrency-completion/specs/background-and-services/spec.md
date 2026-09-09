## ADDED Requirements

### Requirement: The app background scope owns detached work

The implicit `concurrent` scope of `main` SHALL create one child sub-scope,
`App.background`. `Concurrent.detach(fn)` SHALL spawn `fn` into it and return
nothing. On `main` completion or a shutdown signal, `App.background` SHALL be
cancelled, then drained within a bounded grace period (`App.shutdown_grace`,
default 5 seconds), and only then SHALL the process exit. An unhandled exception
in a detached branch SHALL be delivered to `App.on_background_error`, a settable
`(Throwable): Void` whose default logs the branch diagnostic to stderr.

#### Scenario: detached work does not block the caller
- **WHEN** a request handler calls `Concurrent.detach(log_analytics)` and returns
- **THEN** the handler returns immediately and `log_analytics` runs in `App.background`

#### Scenario: detached work is drained at shutdown
- **WHEN** `main` completes with one detached branch still running
- **THEN** the branch is cancelled and given up to the grace period to clean up before the process exits

#### Scenario: detached failure reaches the handler
- **WHEN** a detached branch throws and `App.on_background_error` is set
- **THEN** the handler receives the throwable and the process is not aborted

### Requirement: Long-lived services transfer to the app supervisor

`application.spawn_service(fn)` SHALL transfer `fn` to the application root
supervisor, which SHALL own its startup failure, cancellation, ordered shutdown,
resource cleanup, and final diagnostics. Ordinary code SHALL NOT otherwise detach
work from all scopes.

#### Scenario: a service outlives the request that started it
- **WHEN** a handler calls `application.spawn_service(serve_metrics)` and returns
- **THEN** `serve_metrics` keeps running under the supervisor and is shut down in order when the application stops

#### Scenario: unrestricted detach is rejected
- **WHEN** ordinary code attempts to detach a branch from every scope without the supervisor
- **THEN** compilation fails and directs the developer to `application.spawn_service` or `Concurrent.detach`
