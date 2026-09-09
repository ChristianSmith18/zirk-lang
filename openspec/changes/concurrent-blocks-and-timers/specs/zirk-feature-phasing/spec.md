## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured concurrency semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL deliver the concurrency surface in sub-changes. `remove-task-await-model` removes the old `task` / `await` surface and keeps the runtime. `concurrent-blocks-and-timers` delivers `concurrent { }`, `spawn`, `Job<T>`, and `Timer.sleep` / `Timer.after` / `Timer.every` — a program using these forms SHALL be compiled and run with no phase diagnostic. `parallel-cpu-regions` (`parallel`), `typed-channels` (`Channel<T>`), and `concurrency-completion` (`Concurrent.of` combinators, `Concurrent.detach`, `Thread.run`, `Atomic` / `Mutex` / the synchronizer library) remain deferred until their sub-change lands, reporting the sub-change that delivers them.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts branch scheduling
- **THEN** structured scopes, cancellation, transfer/share analysis, and cleanup behaviour are already specified as prerequisites

#### Scenario: The concurrent and Timer surface is delivered
- **WHEN** a program uses `concurrent { }`, `spawn`, `Job<T>`, `Timer.sleep`, `Timer.after`, or `Timer.every`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: Later concurrency sub-changes stay deferred
- **WHEN** a program uses `parallel`, `Channel<T>`, `Concurrent.of`, `Concurrent.detach`, `Thread.run`, `Mutex`, or `Atomic`
- **THEN** the diagnostic names the construct and the Phase 5 sub-change that delivers it

#### Scenario: Removed constructs are reported as removed
- **WHEN** a program uses `task`, `await`, `select`, or `cancellation shield`
- **THEN** the diagnostic states the construct was removed and names its replacement
