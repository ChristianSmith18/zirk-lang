## Why

Zirk's public documentation names automatic memory management, `unsafe`, tasks,
channels, and parallelism, but it does not yet define the precise contracts an
implementer needs for transactional unsafe mutation, dependent references,
structured failure, cancellation, transfer, or race prevention. These rules
must become normative before the compiler and runtime phases build incompatible
interpretations of safety and concurrency.

## What Changes

- Define the public managed-memory model without exposing a mandatory GC,
  ownership, region, or reference-counting syntax.
- Specify identity, weak references, pinning, dependent native views, pointer
  operations, unsafe functions, casts, volatile access, and deep cloning.
- Make ordinary `unsafe` blocks transactional for Zirk-managed state and
  validated native ranges, with journal/copy-on-write rollback on controlled
  failure and explicit `commit` boundaries for irreversible effects.
- Define `Task<T>`, task scopes, task creation and awaiting, sibling failure,
  cooperative cancellation, shielding, timeouts, and supervised services.
- Specify `Task.all`, `Task.first`, `Task.settled`, `TaskSettlement<T>`, and
  fair `select` over tasks, channels, timers, and cancellation.
- Define channels, inferred transfer/share safety, parallel collection work,
  reductions, scoped threads, blocking adapters, synchronization, and atomics.
- Establish the safe-code data-race guarantee and the restrictions between
  unsafe transactions, suspension, external effects, and concurrency.
- Align the canonical language/runtime/stdlib/compiler documents, handbook,
  reference material, roadmap, examples, and agent-facing context.

## Capabilities

### New Capabilities

- `zirk-memory-safety`: Managed memory, safe and weak references, native views,
  pointers, unsafe operations, transactional rollback, and irreversible commit
  boundaries.
- `zirk-structured-concurrency`: Tasks, scopes, awaiting, cancellation,
  aggregation, selection, channels, parallelism, threads, synchronization, and
  static data-race prevention.

### Modified Capabilities

- `zirk-grammar`: Add and constrain the syntax for unsafe functions,
  transactional unsafe blocks, commit regions, task scopes, cancellation
  shields, timeouts, and select branches.
- `zirk-type-system`: Define `Task<T>`, `TaskSettlement<T>`, `Weak<T>`, native
  views, pointers, deep clone eligibility, and compiler-derived transfer/share
  properties.
- `zirk-runtime-io`: Require task-aware nonblocking I/O, blocking adapters,
  cancellation-safe cleanup, and permission-preserving irreversible effects.
- `zirk-feature-phasing`: Align implementation phases with the now-normative
  memory, unsafe, and concurrency contract.
- `handbook-editorial-system`: Require one consistent normative owner and
  cross-document synchronization for safety and concurrency rules.
- `language-documentation-information-architecture`: Add complete learning and
  reference paths for managed memory, transactional unsafe operations, and
  structured concurrency.

## Impact

The change affects compiler parsing and diagnostics, escape/alias/effect and
race analysis, runtime memory and scheduler design, native interop, standard
library task/synchronization APIs, permission integration, handbook navigation,
implementation roadmaps, and all agents implementing the relevant phases. It
does not require a single internal garbage collector or public lifetime syntax.
