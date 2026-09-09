## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured concurrency semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL deliver the concurrency surface in sub-changes. `concurrent-blocks-and-timers` delivers the I/O surface on a single-threaded cooperative executor. `parallel-cpu-regions` delivers the `parallel` block, the `.parallel` adapter, and `Parallel.each` on a separate worker pool, and with it the runtime gains a stop-the-world safepoint garbage collector — the multi-threading work `ADR-003` and `ADR-017` deferred. A program using `parallel` or `.parallel` after this sub-change SHALL be compiled and run with no phase diagnostic. `typed-channels` (`Channel<T>`) and `concurrency-completion` (`Concurrent.of`, `Concurrent.detach`, `Thread.run`, `Atomic` / `Mutex` / the synchronizer library) remain deferred until their sub-change lands.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts branch scheduling
- **THEN** structured scopes, cancellation, transfer/share analysis, and cleanup behaviour are already specified as prerequisites

#### Scenario: The parallel surface is delivered
- **WHEN** a program uses a `parallel` block, `.parallel`, or `Parallel.each`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: The multi-threaded collector milestone
- **WHEN** `parallel-cpu-regions` lands
- **THEN** the collector has a stop-the-world safepoint and root enumeration is thread-aware, with the object header unchanged

#### Scenario: Channels and synchronization stay deferred
- **WHEN** a program uses `Channel<T>`, `Concurrent.of`, `Thread.run`, `Mutex`, or `Atomic`
- **THEN** the diagnostic names the construct and the Phase 5 sub-change that delivers it
