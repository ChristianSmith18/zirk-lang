## RENAMED Requirements

- FROM: ### Requirement: `Task.all`, `Task.first`, and `Task.settled` typing
- TO: ### Requirement: `Task.all`, `Task.combine`, `Task.first`, and `Task.settled` typing

## MODIFIED Requirements

### Requirement: Async core types are known

The checker SHALL recognize `Task<T>`, `Task.Final<T>`, `Channel<T>`, and the
generic enum `TaskSettlement<T>` with variants `Fulfilled(T)`,
`Rejected(Throwable)`, and `Cancelled(CancelledError)` as ordinary known types,
parameterized over any valid element type. `CancellationReason` SHALL be a known
enum whose default value is `Cancelled` and which SHALL also carry
`Custom(String)`.

#### Scenario: Task annotation resolves

- **WHEN** `mut u: Task<Result<User, LoadError>> = task load_user(42);` is checked
- **THEN** the binding type is `Task<Result<User, LoadError>>` and the initializer must produce that task element type

#### Scenario: Final task annotation resolves

- **WHEN** `mut h: Task.Final<Void> = final task cleanup();` is checked
- **THEN** the binding type is `Task.Final<Void>`

#### Scenario: Settlement enum is matchable

- **WHEN** a `match` over a `TaskSettlement<User>` value handles `Fulfilled`, `Rejected`, and `Cancelled`
- **THEN** the match is exhaustive

### Requirement: `task` produces a typed handle and `await` produces the element type

The checker SHALL type `task expression` as `Task<T>` and `final task expression`
as `Task.Final<T>`, where `T` is the type the expression produces; `task { block }`
and `final task { block }` SHALL be typed the same way over the block's result
type. `await handle` SHALL be typed as exactly `T` for a `handle` of type
`Task<T>` or `Task.Final<T>`; `await` SHALL NOT introduce implicit `Result`,
nullability, or exception wrapping. There SHALL be no `task scope` typing rule.

#### Scenario: Await unwraps exactly the element type

- **WHEN** `mut r = await someTask;` is checked where `someTask: Task<Int32>`
- **THEN** `r` has type `Int32`

#### Scenario: Await of a final task unwraps the element type

- **WHEN** `mut r = await someFinal;` is checked where `someFinal: Task.Final<Int32>`
- **THEN** `r` has type `Int32`

### Requirement: `Task.all`, `Task.combine`, `Task.first`, and `Task.settled` typing

The checker SHALL type `Task.all(tasks)` over an iterable of `Task<T>` as
producing `List<T>` in input order, `Task.combine(a, b, ...)` over 2 to 8
arguments each of type `Task<Ti>` or `Task.Final<Ti>` as producing the tuple
`(T1, T2, ...)`, `Task.first(tasks)` as producing `T`, and `Task.settled(tasks)`
as producing `List<TaskSettlement<T>>` in input order. A `Task<Result<U, E>>`
input to `Task.settled` SHALL settle as `Fulfilled(Result<U, E>)`, never as
`Rejected` for an ordinary `Error(e)` value.

#### Scenario: All-aggregation result type

- **WHEN** `mut users = await Task.all(taskList);` is checked where `taskList` holds `Task<User>` values
- **THEN** `users` has type `List<User>`

#### Scenario: Combine result type

- **WHEN** `mut r = await Task.combine(u, roles, prefs);` is checked where `u: Task<User>`, `roles: Task<List<Role>>`, `prefs: Task<Prefs>`
- **THEN** `r` has type `(User, List<Role>, Prefs)`

#### Scenario: Combine arity ceiling

- **WHEN** `Task.combine` is called with nine arguments
- **THEN** compilation fails and directs the developer to `Task.all` over a list

#### Scenario: Settled result type

- **WHEN** `mut settlements = await Task.settled(taskList);` is checked
- **THEN** `settlements` has type `List<TaskSettlement<User>>`

## ADDED Requirements

### Requirement: A `Task.Final<T>` cannot be cancelled and its await is shielded

The checker SHALL reject `handle.cancel()` when `handle` has type `Task.Final<T>`,
with a diagnostic that points at the `final task` creation site. The checker
SHALL mark an `await` whose operand has static type `Task.Final<T>` as a shielded
await so that lowering can defer the awaiting task's pending cancellation over it.
Single-consume tracking SHALL apply to `Task.Final<T>` exactly as to `Task<T>`.

#### Scenario: Cancel on a final handle is rejected

- **WHEN** `h.cancel();` is checked where `h: Task.Final<Void>`
- **THEN** compilation fails with a diagnostic naming the `final task` site

#### Scenario: Await of a final handle is marked shielded

- **WHEN** `await h;` is checked where `h: Task.Final<T>`
- **THEN** the checked node records that the await is shielded

#### Scenario: Final handle is still single-consume

- **WHEN** `await h;` appears twice for the same `h: Task.Final<T>`
- **THEN** the second await is a use-after-consume error identifying the first consume

### Requirement: Timer factory methods on `Task`

The checker SHALL type `Task.sleep(d)` as `Task<Void>` where `d` is `Duration`,
`Task.after(d, body)` as `Task<T>` where `body` is a callable `(): T` and `d` is
`Duration`, and `Task.every(d, body)` as `Task<Void>` where `body` is a callable
`(): Void` and `d` is `Duration`. `body` SHALL be checked under the ordinary
`task`-body capture rules. `Task.every` SHALL NOT be wrapped in `final task`.
A non-`Duration` first argument SHALL fail with a diagnostic naming `Duration`.

#### Scenario: Sleep is a `Task<Void>`

- **WHEN** `mut nap = Task.sleep(200ms);` is checked
- **THEN** `nap` has type `Task<Void>` and `await nap` has type `Void`

#### Scenario: After carries the callable's result type

- **WHEN** `mut later = Task.after(1s, (): Int32 => compute());` is checked
- **THEN** `later` has type `Task<Int32>`

#### Scenario: Every is a repeating `Task<Void>`

- **WHEN** `mut ticker = Task.every(500ms, (): Void => poll());` is checked
- **THEN** `ticker` has type `Task<Void>`

#### Scenario: A final repeating interval is rejected

- **WHEN** `final task { _ = Task.every(1s, tick); }` wraps an interval so it cannot be cancelled
- **THEN** compilation fails and states that a repeating interval must remain cancellable

#### Scenario: Non-duration delay is rejected

- **WHEN** `Task.sleep(5)` is checked with an integer
- **THEN** compilation fails naming the required `Duration` type
