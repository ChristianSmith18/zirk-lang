## ADDED Requirements

### Requirement: Thread.run executes on a fresh OS thread

`Thread.run(fn)` SHALL run `fn` through bounded managed operating-system-thread admission and return its result. The implementation SHALL use at most 64 process-wide managed workers and at most 1,024 queued callbacks; when the queue is full, callers SHALL apply cooperative backpressure without creating additional OS threads. It SHALL NOT create an unbounded fresh thread per call. The calling branch SHALL suspend cooperatively while the executor keeps
running other branches, and SHALL resume when the OS thread joins. `fn` SHALL NOT
`spawn`, open a `concurrent` block, use a channel, or call `Timer.sleep`; the
compiler SHALL reject such a body and direct ordinary asynchronous I/O to
`concurrent { }`. Calls with incomplete effects or unproven blocking-FFI behavior SHALL be rejected; a blocking native call is allowed only when its complete summary and runtime transition are verified. Every managed worker SHALL publish stable roots and participate in collector coordination while running; compiled loops SHALL contain safepoint polls. Caller cancellation SHALL not release thread-owned values or runtime state before join. Code generation SHALL reject targets without a proven managed-thread and collector ABI. `Thread.run` is implemented on supported native 64-bit targets; the separate `Thread<T>` handle API remains unavailable.

#### Scenario: the executor keeps working during Thread.run
- **WHEN** one branch is inside `Thread.run(blocking_decode)` and another branch waits on a channel
- **THEN** the executor still services the channel branch until the OS thread joins

#### Scenario: suspension inside a Thread.run body is rejected
- **WHEN** a `Thread.run` body calls a channel receive or `Timer.sleep`
- **THEN** compilation fails and directs the developer to `concurrent { }`

#### Scenario: thread admission is bounded
- **WHEN** more calls enter `Thread.run` than the configured managed-thread capacity
- **THEN** excess calls follow the specified bounded queue/backpressure behavior and do not create unbounded OS threads

#### Scenario: cancellation retains managed thread roots
- **WHEN** the calling branch is cancelled while native work is still running
- **THEN** the managed thread retains roots and runtime ownership until it is joined or the documented shutdown deadline is reached

#### Scenario: a collection during Thread.run is safe
- **WHEN** an allocation crosses the GC threshold while a `Thread.run` thread is active
- **THEN** the OS thread publishes and parks with its stable shadow-root frames, and the collection scans those published roots before the thread resumes
