## ADDED Requirements

### Requirement: Thread.run executes on a fresh OS thread

`Thread.run(fn)` SHALL run `fn` on a fresh operating-system thread and return its
result. The calling branch SHALL suspend cooperatively while the executor keeps
running other branches, and SHALL resume when the OS thread joins. `fn` SHALL NOT
`spawn`, open a `concurrent` block, use a channel, or call `Timer.sleep`; the
compiler SHALL reject such a body and direct ordinary asynchronous I/O to
`concurrent { }`. The OS thread SHALL register with the collector and park at a
safepoint on request, with safepoint polls at its loop back-edges.

#### Scenario: the executor keeps working during Thread.run
- **WHEN** one branch is inside `Thread.run(blocking_decode)` and another branch waits on a channel
- **THEN** the executor still services the channel branch until the OS thread joins

#### Scenario: suspension inside a Thread.run body is rejected
- **WHEN** a `Thread.run` body calls a channel receive or `Timer.sleep`
- **THEN** compilation fails and directs the developer to `concurrent { }`

#### Scenario: a collection during Thread.run is safe
- **WHEN** an allocation crosses the GC threshold while a `Thread.run` thread is active
- **THEN** the OS thread parks at a safepoint and the collection walks its stack roots
