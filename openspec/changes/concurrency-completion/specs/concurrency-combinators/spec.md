## ADDED Requirements

### Requirement: Concurrent.of pipeline and terminals

`Concurrent.of` SHALL take 2 to 8 homogeneous thunks of the form `(): T` and return a pipeline; `Void` and `Never` result types SHALL be rejected because they have no representable `Outcome<T>` payload. `.first()` SHALL return the first successful result and cancel and clean the rest. A failure before any success SHALL be retained as the primary failure while siblings are cancelled and joined; simultaneous failures SHALL use a documented deterministic tie-break and later failures SHALL be suppressed. `.settled()` SHALL run all members to completion and
return `List<Outcome<T>>` in argument order. `.within(d)` SHALL cap the pipeline
at a `Duration`, and on expiry SHALL cancel the members, await their cleanup, and
raise `TimeoutError`; `.within(d)` SHALL be chainable before a terminal.
For `.settled()`, a member that is cancelled by the scheduler or whose thunk
throws `CancelledError` SHALL produce `Outcome.Cancelled`; throwing
`CancelledError` is the source-level way to report cancellation of one member
without cancelling the pipeline. The thunk's `finally` cleanup SHALL complete
before its outcome is returned. If that cleanup throws another error, the
member SHALL produce `Outcome.Rejected` carrying the cleanup error.
The primary failure is selected by executor completion sequence (input order
breaks an equal-sequence tie); other rejected branches observed before cleanup
finishes SHALL be retained on the primary failure in completion order. If the
selected branch succeeds but cancellation cleanup of a losing branch fails,
`.first()` SHALL reject with the earliest cleanup failure as primary and retain
the remaining cleanup failures as suppressed, in completion order.

#### Scenario: first wins and cancels the rest
- **WHEN** `Concurrent.of(a, b).first()` runs and `b` finishes first
- **THEN** the result is `b`'s value and `a` is cancelled and cleaned

#### Scenario: settled reports every outcome in order
- **WHEN** one member fulfills, one throws an ordinary error, and one throws `CancelledError` after cleanup
- **THEN** `.settled()` returns ordered `Fulfilled`, `Rejected`, `Cancelled`, and waits for the cancelled member's cleanup

#### Scenario: settled reports cleanup failure as rejection
- **WHEN** a member throws `CancelledError` and its `finally` cleanup throws another error
- **THEN** that member returns `Outcome.Rejected` with the cleanup error after cleanup completes

#### Scenario: first retains failure and cleans siblings
- **WHEN** one member fails before any member succeeds
- **THEN** the failure is primary, active siblings are cancelled and joined, and cleanup failures are suppressed

#### Scenario: first reports cleanup failure after a successful winner
- **WHEN** a member succeeds and cancellation cleanup of one or more losing members fails
- **THEN** `.first()` rejects with the earliest cleanup failure as primary and retains remaining cleanup failures as suppressed

#### Scenario: caller cancellation differs from timeout
- **WHEN** the caller is cancelled while a pipeline has a deadline
- **THEN** caller cancellation remains cancellation and is not converted to `TimeoutError`

#### Scenario: within raises on expiry
- **WHEN** `Concurrent.of(slow).within(5s).first()` and `slow` is still running at 5s
- **THEN** `slow` is cancelled and cleaned and `TimeoutError` is raised

### Requirement: Concurrent.each and each_settled

`Concurrent.each(coll, fn)` and `Concurrent.each_settled(coll, fn)` SHALL accept only `Array<T>` or `List<T>` inputs containing at most 256 elements. The implementation SHALL validate the collection size before starting any callback. Both operations SHALL accept an optional explicit concurrency limit in the range 1 through 256; when omitted, the limit SHALL be 32. A limit larger than the collection length SHALL admit at most the collection length. Empty input SHALL return an empty list without starting callbacks.

The collection SHALL be held by an `inmut::strict` binding and its complete element graph SHALL satisfy `Share`; the collection is shared with callbacks and SHALL NOT be accepted merely because one local name is transferred. This prevents mutable aliases through other bindings, captures, fields, or views from racing with callbacks. The callback SHALL be a one-argument function accepting `T`. Its result type `R` SHALL be a value type other than `Void` or `Never`, and its captures and result SHALL satisfy the existing `Share` and `Transfer` checks for managed concurrent work. `Concurrent.each` SHALL return `List<R>` in input order and fail fast on the first unhandled exception, cancelling and joining every admitted sibling before propagating the failure. `Concurrent.each_settled` SHALL let every callback finish and return `List<Outcome<R>>` in input order. Caller cancellation SHALL cancel and join all admitted callbacks before propagating caller cancellation; it SHALL NOT return a partial result list. Invalid limits and collections larger than 256 elements SHALL raise `InvalidArgumentError` before any callback starts.

#### Scenario: each preserves input order
- **WHEN** `items` is an `inmut::strict` collection `[a, b, c]`, and `Concurrent.each(items, f)` completes `f(c)` first
- **THEN** the result is `[f(a), f(b), f(c)]`

#### Scenario: each respects the active-work limit
- **WHEN** `Concurrent.each` receives more elements than its configured concurrency limit
- **THEN** no more than that many callbacks are active at once and remaining elements are admitted as slots become available

#### Scenario: each fails fast
- **WHEN** `f(b)` throws while `f(a)` and `f(c)` are still running
- **THEN** `f(a)` and `f(c)` are cancelled and cleaned and the exception propagates

#### Scenario: collection limit is checked before callback admission
- **WHEN** either collection combinator receives 257 elements
- **THEN** it raises `InvalidArgumentError` and invokes no callback

#### Scenario: default concurrency and explicit limit are bounded
- **WHEN** the collection combinator receives a valid collection with no limit, or an explicit limit from 1 through 256
- **THEN** at most 32 callbacks are active by default, or at most the explicit limit when supplied, capped by the collection length

#### Scenario: invalid concurrency limits are rejected before callback admission
- **WHEN** either collection combinator receives an explicit limit of 0 or greater than 256
- **THEN** it raises `InvalidArgumentError` and invokes no callback

#### Scenario: callback result must have a representable concurrent payload
- **WHEN** the callback returns `Void` or `Never`, or its captures/results fail `Share` or `Transfer` checks
- **THEN** semantic analysis rejects the call before lowering

#### Scenario: caller cancellation joins admitted callbacks
- **WHEN** the caller is cancelled while callbacks are active
- **THEN** every admitted callback is cancelled and cleaned before caller cancellation propagates, with no partial list returned

### Requirement: Outcome enum

`Outcome<T>` SHALL be a known enum with `Fulfilled(T)`, `Rejected(Throwable)`,
and `Cancelled(CancelledError)`. A branch that returns `Result.Error(e)` SHALL
settle as `Fulfilled(Error(e))`; only an unhandled throwable SHALL settle as
`Rejected`.

#### Scenario: returned Result.Error is a fulfilled outcome
- **WHEN** a settled member returns `Error(problem)` normally
- **THEN** its outcome is `Fulfilled(Error(problem))`, not `Rejected`
