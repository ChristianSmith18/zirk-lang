## ADDED Requirements

### Requirement: Codegen for synchronization and threads

Native code generation SHALL emit `zirk_rt_thread_run` and a cooperative join for
`Thread.run`, with `zirk_rt_safepoint_poll` at every loop back-edge in the
thread body. It SHALL emit LLVM atomic instructions with the requested ordering
for `Atomic` operations, and `zirk_rt_mutex_lock` / `zirk_rt_mutex_unlock` around
a `mutex.with` closure with the unlock on every exit edge. `Concurrent.protect`
SHALL emit a shield-depth increment and a decrement-plus-check on every exit
edge.

#### Scenario: Thread.run emits a spawn, a join, and safepoint polls
- **WHEN** codegen processes `Thread.run(work)` whose body loops
- **THEN** it emits an OS-thread spawn, a cooperative join, and a safepoint poll at the body's loop back-edge

#### Scenario: atomic op carries its ordering
- **WHEN** codegen processes `counter.fetch_add(1)`
- **THEN** it emits an LLVM atomicrmw with sequentially consistent ordering
