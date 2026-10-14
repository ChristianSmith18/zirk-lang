## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured concurrency semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

The existing Phase 5 sub-changes remain delivered: `concurrent-blocks-and-timers`, `parallel-cpu-regions`, and `typed-channels`. `concurrency-completion` delivers bounded combinators, `Outcome<T>`, supervised background work, managed collector-aware threads where supported, cooperative synchronizers, portable atomics, bounded non-suspending cleanup protection, and sound effect plus Transfer/Share enforcement. Phase 5 SHALL be marked complete only after all capability gates and supported-target evidence pass; unsupported targets retain a precise phase/capability diagnostic. `task`, `await`, `select`, and `cancellation shield` remain removed constructs.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts branch scheduling
- **THEN** structured scopes, cancellation, transfer/share analysis, and cleanup behaviour are already specified as prerequisites

#### Scenario: The concurrent and Timer surface is delivered
- **WHEN** a program uses `concurrent { }`, `spawn`, `Job<T>`, `Timer.sleep`, `Timer.after`, or `Timer.every`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: Completion sub-change stays deferred
- **WHEN** a completion API is not yet enabled because its safety gate is incomplete
- **THEN** the diagnostic names the construct and the unmet Phase 5 capability gate

#### Scenario: Channel surface is delivered
- **WHEN** a program constructs `Channel<T>` and uses `send`, `receive`, `try_send`, `try_receive`, `close`, or `drain`
- **THEN** no phase diagnostic is emitted

#### Scenario: The parallel surface is delivered
- **WHEN** a program uses `parallel`, `.parallel`, or `Parallel.each`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: Removed constructs are reported as removed
- **WHEN** a program uses `task`, `await`, `select`, or `cancellation shield`
- **THEN** the diagnostic states the construct was removed and names its replacement

#### Scenario: Phase 5 is complete
- **WHEN** all Phase 5 implementation gates pass for a supported target and a program uses a delivered concurrency construct
- **THEN** no phase diagnostic is emitted and the construct is compiled and run
- **AND** unsupported target capabilities retain a precise diagnostic

#### Scenario: Removed constructs stay removed
- **WHEN** a program uses `task`, `await`, `select`, or `cancellation shield`
- **THEN** the diagnostic states the construct was removed and names its replacement
