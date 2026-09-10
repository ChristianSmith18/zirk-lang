# timer-operations Specification

## Purpose
TBD - created by archiving change concurrent-blocks-and-timers. Update Purpose after archive.
## Requirements
### Requirement: `Timer.sleep` suspends the current branch

`Timer.sleep(d)` SHALL take a `Duration`, suspend the branch that calls it for at
least `d`, and be a cancellation safe point. A negative `d` SHALL be a controlled
error raised before any waiting begins. `Timer.sleep` SHALL be callable anywhere,
with or without an enclosing `concurrent` block, and SHALL yield the executor so
other ready branches run during the wait.

#### Scenario: sleep yields the executor
- **WHEN** one branch calls `Timer.sleep(10ms)` while another branch is ready
- **THEN** the other branch runs during the wait and the sleeping branch resumes after the deadline

#### Scenario: cancellation interrupts sleep
- **WHEN** a branch is cancelled while suspended in `Timer.sleep(1s)`
- **THEN** `Timer.sleep` raises `CancelledError` at that point

#### Scenario: negative duration is rejected
- **WHEN** `Timer.sleep(d)` is called with a negative `d`
- **THEN** a controlled error is raised and no waiting occurs

### Requirement: `Timer.after` and `Timer.every` are ambient timer jobs

`Timer.after(d, thunk)` SHALL run `thunk` once after `d` and return `Job<T>`
where `T` is `thunk`'s result type. `Timer.every(d, thunk)` SHALL run `thunk`
repeatedly with a fixed delay `d` between runs until cancelled, and return
`Job<Void>` that completes only through cancellation. Both SHALL be owned by the
nearest enclosing lexical `concurrent` block, or by the implicit scope of `main`.
They SHALL be **ambient**: when their owning block closes (normally or
exceptionally) they SHALL be cancelled, not awaited, unless the program holds
their handle and calls `wait()`.

#### Scenario: every re-arms until cancelled
- **WHEN** `Timer.every(3ms, tick)` runs for 11ms and is then cancelled
- **THEN** `tick` has run three times and does not run again

#### Scenario: ambient timer is cancelled on block close
- **WHEN** a `concurrent` block creates `Timer.every(1s, tick)` and its other work finishes
- **THEN** the block closes and the timer job is cancelled without the block waiting for it

#### Scenario: after is scheduled through the timer service
- **WHEN** `Timer.after(50ms, fn)` is created
- **THEN** the runtime arms one timer deadline and runs `fn` on a branch of the owning scope when it expires

