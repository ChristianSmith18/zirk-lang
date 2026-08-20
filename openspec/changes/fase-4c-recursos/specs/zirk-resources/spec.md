## MODIFIED Requirements

### Requirement: Match with closes successful acquisitions exactly once

The language SHALL define `Resource<E from Error>` with typed `close` and
closed-state behavior. `match with` SHALL own each successful acquisition and
close it exactly once on normal completion, return, exception, or control
transfer leaving the scope.

This pass (`fase-4c-recursos`) narrows the requirement above to a single
acquisition per `match ... with`: the scrutinee must be `Result<R,Err>`, and
`binding` must be the name exactly one arm's own pattern destructures — no
grouped acquisition (`match a with x, b with y { ... }`) exists yet. Cleanup
runs on normal completion, `return`, `break`, `continue`, and a propagating
exception; cancellation is not covered, since no structured concurrency
exists yet (Phase 5). `close()`'s own returned `Result` is discarded rather
than surfaced — `ResourceFailure<BodyError,CloseError>` does not exist yet
(see the separate, still-`ADDED` requirement below), so a close failure is
silent in this pass.

#### Scenario: Body returns early

- **WHEN** a `match with` success branch returns before its final statement
- **THEN** the owned resource closes before the return value leaves the scope

#### Scenario: Exception propagates through the owning branch

- **WHEN** a `match with` success branch calls a function whose exception
  propagates past it uncaught
- **THEN** the owned resource closes before the exception continues
  propagating

## ADDED Requirements

### Requirement: A close failure is not yet observable

`close()`'s own `Result<Void,E>` SHALL be called for its side effect only;
its returned value SHALL be discarded rather than surfaced — a failing close
is silent to the program in this pass. `ResourceFailure<BodyError,CloseError>`,
`suppressed`, grouped acquisition's right-to-left unwind,
`TransferableResource`/`transfer()`, the escape/use-after-transfer analysis,
and dependent-resource lifetime SHALL remain unimplemented; this requirement
exists so their absence is a documented, verifiable narrowing rather than a
silent gap.

#### Scenario: Close fails after a successful body

- **WHEN** a `match with` branch completes normally and the owned resource's
  `close()` returns `Result.Error`
- **THEN** the program observes no diagnostic and no `ResourceFailure` — the
  failure is discarded
