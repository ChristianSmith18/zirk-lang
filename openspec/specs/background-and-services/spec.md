# background-and-services

## Purpose

Defines the normative requirements for background and services in Zirk.

## Requirements

### Requirement: The app background scope owns detached work

The implicit `concurrent` scope of `main` SHALL create one child sub-scope,
`App.background`. `Concurrent.detach(fn)` SHALL spawn `fn` into it and return
nothing. On `main` completion or a shutdown signal, `App.background` SHALL be
cancelled, then drained within a bounded grace period (`App.shutdown_grace`,
default 5 seconds). On deadline expiry the runtime SHALL report unfinished jobs
and follow the host exit policy: generated CLI executables SHALL exit with
status 124, while embedded hosts retain control and choose their own policy.
The runtime SHALL NOT claim to kill a native thread. Admission SHALL be bounded. Handler failures SHALL be captured and
reported without destabilizing shutdown. An unhandled exception
in a detached branch SHALL be delivered to `App.on_background_error`, a settable
`(Throwable): Void` whose default logs the branch diagnostic to stderr.

Native compiled programs SHALL expose `App.set_shutdown_grace(Duration): Void`
to configure this executor's grace before draining begins. The default is five
seconds; zero is valid. A negative duration or a call after draining starts
SHALL raise `InvalidArgumentError`. The browser interpreter has no app-level
executor and SHALL report this capability as unsupported.

The supervisor SHALL admit at most 256 active background jobs. A rejected
`Concurrent.detach` call SHALL throw `BackgroundCapacityError`; the quota is
released when a job reaches a terminal state.

The supervisor lifecycle SHALL be `Created -> Starting -> Ready -> Draining ->
Stopped`. Readiness is published only after every required service reports ready.
A required startup failure or startup cancellation enters `Draining`, stops
already-started services in reverse startup order, and fails application
startup. A host shutdown signal or completion of `main` stops new admission,
cancels `App.background`, and stops services in reverse startup order. A runtime
service failure after `Ready` is reported and enters shutdown unless an explicit
restart policy is configured. Shutdown completion releases runtime roots and
publishes final diagnostics; deadline expiry reports unfinished jobs and uses
the host exit policy while retaining roots for native participants until they
actually exit. The runtime SHALL NOT claim to forcibly terminate native work.

#### Scenario: detached work does not block the caller
- **WHEN** a request handler calls `Concurrent.detach(log_analytics)` and returns
- **THEN** the handler returns immediately and `log_analytics` runs in `App.background`

#### Scenario: detached work is drained at shutdown
- **WHEN** `main` completes with one detached branch still running
- **THEN** the branch is cancelled and given up to the grace period to clean up before the process exits

#### Scenario: background admission is bounded
- **WHEN** 256 background jobs are active and another call invokes `Concurrent.detach`
- **THEN** the call throws `BackgroundCapacityError` without admitting another job

#### Scenario: background quota is released
- **WHEN** a background job completes, fails, or finishes cancellation cleanup
- **THEN** its admission slot can be reused by a later background job

#### Scenario: executor task quota and background quota remain distinguishable
- **WHEN** a detach call is rejected because the executor's 4,096 active-task quota is full before the 256-job background quota is full
- **THEN** the call throws `TaskCapacityError`, preserving the distinct resource cause

#### Scenario: shutdown deadline reports unfinished work
- **WHEN** a detached operation does not finish before `App.shutdown_grace` expires
- **THEN** the runtime reports it as unfinished and follows the host exit policy without claiming forcible thread cancellation

#### Scenario: native code configures the shutdown grace
- **WHEN** a native program calls `App.set_shutdown_grace(250ms)` before shutdown
- **THEN** the executor uses that grace instead of the five-second default

#### Scenario: invalid shutdown grace is rejected
- **WHEN** code supplies a negative duration or changes the grace after draining starts
- **THEN** the call raises `InvalidArgumentError` without changing the active deadline

#### Scenario: background handler failure is contained
- **WHEN** `App.on_background_error` itself throws while handling a detached failure
- **THEN** the secondary failure is captured in diagnostics and shutdown remains coherent

#### Scenario: background handler failure is not delivered recursively
- **WHEN** the managed `App.on_background_error` callback throws while handling a branch failure
- **THEN** the runtime reports the handler failure once and does not invoke that handler recursively for its own failure

#### Scenario: detached failure reaches the handler
- **WHEN** a detached branch throws and `App.on_background_error` is set
- **THEN** the handler receives the throwable and the process is not aborted
### Requirement: Long-lived services transfer to the app supervisor

`application.spawn_service(fn)` SHALL transfer a `(ServiceContext): Void`
callable to the application root supervisor, which SHALL own its captures,
startup failure, cancellation, ordered shutdown, resource cleanup, and final
diagnostics. Every source-level service is required. Registration SHALL be
accepted from managed application tasks while the supervisor is not draining;
it SHALL start the service immediately when its required predecessors are ready.
Each service SHALL depend on all previously registered required services, so
registration order is also startup dependency order. At most 64 source-level
services SHALL be registered per application. The runtime SHALL throw
`ServiceCapacityError` when that quota is full, `TaskCapacityError` when the
shared 4,096 active-task quota prevents admission, and `ServiceLifecycleError`
when registration occurs after draining begins.

The service SHALL call `context.ready()` after its required startup work
succeeds. Returning or throwing before `ready()` is a startup failure. Throwing
or returning normally after readiness is an unexpected runtime service failure
and initiates application shutdown. Application readiness is published only
after every required service reports ready. `application.spawn_service` SHALL
not block the caller while the service starts; a request/handler may return
while its service remains owned by the still-running application. Returning
from a handler does not end the application lifecycle; completion of `main` or
a host shutdown request does. Service shutdown uses the same configured grace
period as `App.background`; deadline handling reports unfinished jobs and
services, preserves roots for live native participants, and follows the host
exit policy. Ordinary code SHALL NOT otherwise detach work from all scopes.

#### Scenario: a service outlives the request that started it
- **WHEN** a handler calls `application.spawn_service(serve_metrics)` and returns
- **THEN** `serve_metrics` keeps running under the supervisor and is shut down in order when the application stops

#### Scenario: service readiness follows successful startup
- **WHEN** a service calls `ServiceContext.ready()` after opening its required resources
- **THEN** the supervisor records it as ready and does not publish application readiness until all required services are ready

#### Scenario: service completion before readiness fails startup
- **WHEN** a service returns or throws before calling `ServiceContext.ready()`
- **THEN** application startup fails and already-started services stop in reverse startup order

#### Scenario: later service waits for earlier required service
- **WHEN** a later service is registered while an earlier required service has not reported ready
- **THEN** the supervisor retains its captures as GC roots and does not begin it until every earlier required service reports ready

#### Scenario: runtime service failure initiates shutdown
- **WHEN** a ready service throws or returns normally
- **THEN** the supervisor records the failure and drains services in reverse startup order

#### Scenario: handler completion preserves application services
- **WHEN** a request handler registers a service and then returns while the application remains active
- **THEN** the service continues under the application supervisor until application shutdown

#### Scenario: service shutdown shares the application deadline
- **WHEN** a service does not finish cleanup before `App.shutdown_grace` expires
- **THEN** the runtime reports the unfinished service and follows the same host exit policy used for unfinished background jobs

#### Scenario: unrestricted detach is rejected
- **WHEN** ordinary code attempts to detach a branch from every scope without the supervisor
- **THEN** compilation fails and directs the developer to `application.spawn_service` or `Concurrent.detach`
