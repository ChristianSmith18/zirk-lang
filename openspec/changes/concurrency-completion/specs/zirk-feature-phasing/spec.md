## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured concurrency semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL deliver the concurrency surface in sub-changes: `concurrent-blocks-and-timers`, `parallel-cpu-regions`, `typed-channels`, and `concurrency-completion`. `concurrency-completion` delivers the `Concurrent.of(...)` combinators (`first` / `settled` / `within`), `Concurrent.each` / `each_settled`, `Outcome<T>`, `Concurrent.detach` and the app background scope, `application.spawn_service`, `Thread.run`, `Atomic<T>`, `Mutex<T>` and the `RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>` library, `Concurrent.protect`, and full `Transfer` / `Share` enforcement. When it lands, Phase 5 is complete: every concurrency construct in the language is implemented, and no concurrency construct emits a phase diagnostic. `task`, `await`, `select`, and `cancellation shield` remain removed constructs.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts branch scheduling
- **THEN** structured scopes, cancellation, transfer/share analysis, and cleanup behaviour are already specified as prerequisites

#### Scenario: Phase 5 is complete
- **WHEN** a program uses any of `concurrent { }`, `spawn`, `parallel`, `Timer`, `Channel<T>`, `Concurrent.of`, `Concurrent.each`, `Concurrent.detach`, `Concurrent.protect`, `Thread.run`, `Atomic`, `Mutex`, `RwLock`, `Semaphore`, `Barrier`, `Once`, or `application.spawn_service`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: Removed constructs stay removed
- **WHEN** a program uses `task`, `await`, `select`, or `cancellation shield`
- **THEN** the diagnostic states the construct was removed and names its replacement
