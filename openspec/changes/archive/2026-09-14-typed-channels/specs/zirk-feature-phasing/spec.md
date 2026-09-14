## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured concurrency semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL deliver the concurrency surface in sub-changes: `concurrent-blocks-and-timers` (the I/O surface), `parallel-cpu-regions` (the CPU surface), `typed-channels` (`Channel<T>`), and `concurrency-completion` (the `Concurrent.of` combinators, `Concurrent.detach`, `Thread.run`, and the synchronizer library). `typed-channels` delivers the core `Channel<T>` — bounded, rendezvous, growable-with-limit, suspendible `send` / `receive`, non-blocking `try_*` with typed outcomes, idempotent `close`, and `drain` — on the single-threaded cooperative executor. A program using `Channel<T>` after this sub-change SHALL be compiled and run with no phase diagnostic. `select` is removed, not deferred. The broadcast, latest-value watch, and one-shot channel families remain specified and deferred to a follow-up. `concurrency-completion` remains deferred until its sub-change lands.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts branch scheduling
- **THEN** structured scopes, cancellation, transfer/share analysis, and cleanup behaviour are already specified as prerequisites

#### Scenario: The Channel surface is delivered
- **WHEN** a program constructs a `Channel<T>` and uses `send` / `receive` / `try_send` / `try_receive` / `close` / `drain`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: select is removed, channel families are deferred
- **WHEN** a program uses `select`, or a broadcast / watch / oneshot channel
- **THEN** `select` gets the removed-construct diagnostic and the channel families get a deferred diagnostic naming the follow-up

#### Scenario: The completion surface stays deferred
- **WHEN** a program uses `Concurrent.of`, `Concurrent.detach`, `Thread.run`, `Mutex`, or `Atomic`
- **THEN** the diagnostic names the construct and the Phase 5 sub-change that delivers it
