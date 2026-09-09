## ADDED Requirements

### Requirement: Concurrent.of pipeline and terminals

`Concurrent.of` SHALL take 2 to 8 thunks of the form `(): T` and return a
pipeline. `.first()` SHALL run all members, return the first to finish, and
cancel and clean the rest. `.settled()` SHALL run all members to completion and
return `List<Outcome<T>>` in argument order. `.within(d)` SHALL cap the pipeline
at a `Duration`, and on expiry SHALL cancel the members, await their cleanup, and
raise `TimeoutError`; `.within(d)` SHALL be chainable before a terminal.

#### Scenario: first wins and cancels the rest
- **WHEN** `Concurrent.of(a, b).first()` runs and `b` finishes first
- **THEN** the result is `b`'s value and `a` is cancelled and cleaned

#### Scenario: settled reports every outcome in order
- **WHEN** one member fulfills, one throws, and one is cancelled
- **THEN** `.settled()` returns ordered `Fulfilled`, `Rejected`, `Cancelled`

#### Scenario: within raises on expiry
- **WHEN** `Concurrent.of(slow).within(5s).first()` and `slow` is still running at 5s
- **THEN** `slow` is cancelled and cleaned and `TimeoutError` is raised

### Requirement: Concurrent.each and each_settled

`Concurrent.each(coll, fn)` SHALL run `fn` per element concurrently and return
`List<R>` in input order, failing fast on the first unhandled exception.
`Concurrent.each_settled(coll, fn)` SHALL let every element finish and return
`List<Outcome<R>>` in input order.

#### Scenario: each preserves input order
- **WHEN** `Concurrent.each([a, b, c], f)` and `f(c)` finishes first
- **THEN** the result is `[f(a), f(b), f(c)]`

#### Scenario: each fails fast
- **WHEN** `f(b)` throws while `f(a)` and `f(c)` are still running
- **THEN** `f(a)` and `f(c)` are cancelled and cleaned and the exception propagates

### Requirement: Outcome enum

`Outcome<T>` SHALL be a known enum with `Fulfilled(T)`, `Rejected(Throwable)`,
and `Cancelled(CancelledError)`. A branch that returns `Result.Error(e)` SHALL
settle as `Fulfilled(Error(e))`; only an unhandled throwable SHALL settle as
`Rejected`.

#### Scenario: returned Result.Error is a fulfilled outcome
- **WHEN** a settled member returns `Error(problem)` normally
- **THEN** its outcome is `Fulfilled(Error(problem))`, not `Rejected`
