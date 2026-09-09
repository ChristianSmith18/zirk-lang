## ADDED Requirements

### Requirement: Lowering of synchronization, threads, and protect

`Thread.run(fn)` SHALL lower to an OS-thread spawn instruction plus a
join-suspension point, with a safepoint poll at every loop back-edge in `fn`'s
body. `Atomic` operations SHALL lower to typed atomic instructions carrying an
`AtomicOrder`. `mutex.with(closure)` SHALL lower to a lock instruction, the
closure body, and an unlock installed on every exit edge. `Concurrent.protect(fn)`
SHALL lower to a shield-depth increment before the body and a decrement plus a
pending-cancellation check on every exit edge.

#### Scenario: Thread.run join is a suspension point
- **WHEN** `Thread.run(work)` is lowered
- **THEN** the IR contains an OS-thread spawn and a join instruction that is a suspension point

#### Scenario: mutex unlock on every exit edge
- **WHEN** a `mutex.with` closure contains an early `return` and can throw
- **THEN** the unlock instruction is reached on the normal, `return`, and exceptional edges

#### Scenario: protect brackets the shield depth
- **WHEN** `Concurrent.protect(fn)` is lowered
- **THEN** the IR increments shield depth before `fn` and decrements it with a cancellation check on every exit edge
