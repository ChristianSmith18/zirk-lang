## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured concurrency semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL deliver the concurrency surface `concurrent { }` / `parallel { }` / `spawn` and the method API on `Concurrent` / `Timer` / `Channel<T>` / `Thread`, on a single-threaded cooperative executor for the I/O surface and a separate worker pool for the CPU surface. Within Phase 5 the runtime infrastructure — the cooperative executor, the branch control block, the stackful-coroutine suspend/resume driver, the timer service, per-branch garbage-collection roots, and the executor lifecycle around `main` — SHALL be delivered and independently verified before any concurrency syntax is parsed, checked, lowered, or emitted.

The `task` and `await` keywords, `Task<T>`, `task scope`, `select`, and `cancellation shield` are **removed** from the language. Source using them SHALL receive a removed-construct diagnostic naming the replacement, not a deferred-construct diagnostic. `parallel`, `thread`, `Atomic`, and the synchronizer library remain deferred within Phase 5 until their sub-change lands.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts branch scheduling
- **THEN** structured scopes, cancellation, transfer/share analysis, and cleanup behaviour are already specified as prerequisites

#### Scenario: Removed constructs are reported as removed
- **WHEN** a program uses `task`, `await`, `select`, or `cancellation shield`
- **THEN** the diagnostic states the construct was removed and names its replacement, rather than naming a future phase

#### Scenario: The concurrency surface is the Phase 5 deliverable
- **WHEN** a program uses `concurrent { }`, `spawn`, `parallel { }`, `Timer`, or `Channel<T>` after Phase 5 delivers each
- **THEN** no phase diagnostic is emitted and the construct is compiled and run
