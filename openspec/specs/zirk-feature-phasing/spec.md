# zirk-feature-phasing

## Purpose

Defines the discipline that keeps the compiler honest about what it does not implement yet.

The language is documented in full while it is built in phases. That gap is deliberate, and it is only safe as long as every documented feature has an owning phase and the compiler names that phase instead of failing as if the feature did not exist.

## Requirements

### Requirement: Every documented feature has an owning phase

Every feature defined by the normative sources SHALL have exactly one
construction phase assigned to it in `docs/init/ZIRK_ROADMAP.md`.

A feature without an assigned phase is not a deferred feature: it is a
feature that nobody will build. Assigning it is what prevents it from
sneaking into the active phase and overflowing it, or from remaining as debt
with no due date.

The infix exponentiation operator `**` and its compound form `**=` are
delivered by Phase 3b, together with the numeric families they operate on.
Once delivered, `**` is part of the implemented subset and is no longer a
deferred construct.

#### Scenario: Normative feature without a phase
- **WHEN** a normative source defines a feature that no phase of the roadmap names
- **THEN** the roadmap is corrected by assigning it a phase before implementing any part of it

#### Scenario: Features orphaned by normative refinement
- **WHEN** the `Float` family, graphemic `Char`, final-result scalar conversion, mathematical integer division, the bitwise and shift operators, and string interpolation are consulted
- **THEN** the roadmap assigns them to Phase 3b
- **AND** assigns `inmut::strict` to Phase 4 and the temporal family to Phase 7

#### Scenario: Exponentiation operator is delivered, not deferred
- **WHEN** a program in the implemented subset uses `**` or `**=`
- **THEN** no phase diagnostic is emitted and the operator is compiled

### Requirement: Phase diagnostic for what is not implemented

The compiler SHALL emit a diagnostic naming the construct and its arrival
phase for any construct, keyword, operator, literal, or type that belongs to
the language but not to the implemented phase.

The compiler SHALL NOT produce a generic syntax error, an "unrecognized
character" error, or a silent interpretation different from the construct as
written.

#### Scenario: Construct from a later phase
- **WHEN** the source contains a language construct that the current phase does not implement
- **THEN** the diagnostic names it and indicates the phase in which it arrives

#### Scenario: Absence of silent interpretation
- **WHEN** the source contains a literal or an operator of the language that the current phase does not implement
- **THEN** the compiler recognizes it as such and defers it along with its phase
- **AND** it does NOT reinterpret it as a different sequence of tokens

### Requirement: The table of pending types reflects the current language

The table of known-but-not-implemented types SHALL contain only types that
the language defines today, each with the phase that actually brings it.

A type withdrawn from the language SHALL disappear from the table:
announcing its arrival would teach a language that does not exist.

`Task<T>`, `Channel<T>`, and `TaskSettlement<T>` SHALL NOT appear in the table
once Phase 5 steps 1 to 3 are delivered; they resolve as ordinary known types.
`Atomic<T>` SHALL remain in the table with its Phase 5 arrival step.

#### Scenario: Type withdrawn from the language
- **WHEN** an annotation names `Decimal64` or another member of the old `Decimal` family
- **THEN** the diagnostic treats it as a nonexistent type
- **AND** it does NOT announce any arrival phase

#### Scenario: Correctly declared phase
- **WHEN** an annotation names a pending type
- **THEN** the phase indicated by the diagnostic matches the one the roadmap assigns to it

#### Scenario: Delivered async type no longer pends

- **WHEN** an annotation names `Channel<Int32>` or `Task<String>` after Phase 5 steps 1 to 3
- **THEN** it resolves as a known type and no pending-type diagnostic is emitted

#### Scenario: Atomic still pends

- **WHEN** an annotation names `Atomic<Int32>`
- **THEN** the pending-type diagnostic names it and its Phase 5 arrival step

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
### Requirement: Browser interpretation is distinct from a WebAssembly language target

Project phase and implementation-status documentation SHALL describe browser execution as compiler tooling in which the Rust frontend and IR interpreter are compiled to WebAssembly. It SHALL NOT claim that Zirk programs compile to WebAssembly or that `wasm32` is a supported Zirk target.

#### Scenario: Publish browser playground availability
- **WHEN** the browser interpreter and companion playground integration are complete
- **THEN** status documentation may state that supported Zirk programs can be interpreted locally in the browser
- **AND** it continues to list a Zirk-to-WebAssembly backend as excluded from Zirk 1.x or deferred to its future phase

#### Scenario: Inspect target support
- **WHEN** a user reads the target matrix after this change
- **THEN** no WebAssembly target triple is listed as a supported Zirk compilation target

### Requirement: Browser compatibility claims remain scoped

Implementation-status and limitation documentation SHALL distinguish the native compiler's delivered language surface from the browser interpreter's compatibility surface. A feature SHALL NOT be marked browser-executable merely because the parser, checker, IR, native backend, or native runtime implements it.

#### Scenario: Native-only feature is implemented
- **WHEN** a language feature works through native compilation but requires an excluded browser capability
- **THEN** browser compatibility documentation identifies it as unavailable or constrained

#### Scenario: Browser-compatible feature is claimed
- **WHEN** a feature is described as executable in the browser
- **THEN** interpreter coverage, browser-package tests, and differential conformance evidence exist for that feature

### Requirement: Cross-repository status is released coherently

Public browser-execution status SHALL be updated in the compiler repository and `zirk-lang-site` within the same coordinated workstream. The website SHALL pin the exact clean compiler revision and browser artifact provenance that support its claim.

#### Scenario: Compiler support lands before site integration
- **WHEN** interpreter implementation exists but the website has not consumed and verified its package
- **THEN** the public site continues to describe Run and Check as unavailable

#### Scenario: Publish the integrated feature
- **WHEN** compiler conformance, artifact provenance, site integration, accessibility, and Firebase build verification are complete
- **THEN** both repositories describe the same supported browser execution surface and exclusions
