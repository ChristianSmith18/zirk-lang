# concurrent-scopes Specification

## Purpose
TBD - created by archiving change concurrent-blocks-and-timers. Update Purpose after archive.
## Requirements
### Requirement: The `concurrent` block is a structured scope

A `concurrent { ... }` statement SHALL open a structured scope. `inmut` and `mut`
declarations written directly in the block SHALL be visible in the enclosing
scope after the block, and SHALL NOT be readable before the block closes except
inside a branch that depends on them. The block SHALL NOT transfer control past
its closing brace until every branch it owns has reached a terminal state.

#### Scenario: Bindings hoist and fill on close
- **WHEN** `concurrent { inmut a = f(); inmut b = g(); }` is followed by `use(a, b)`
- **THEN** `a` and `b` are in scope after the block and hold their branch results

#### Scenario: Block waits for all branches
- **WHEN** control reaches the closing brace of a `concurrent` block with an unfinished branch
- **THEN** the block waits for that branch to finish (or, on an exceptional exit, cancels it and waits for cleanup) before continuing

#### Scenario: Reading a binding early is rejected
- **WHEN** a non-dependent statement inside the block reads a sibling binding before the block closes
- **THEN** compilation fails, indicating the binding is not available until the block closes

### Requirement: Dataflow branch ordering

Each top-level binding in a `concurrent` block SHALL be a branch. The compiler
SHALL order branches by a dependency DAG built over the block's bindings: an edge
exists when one binding's initializer names another. Independent branches SHALL
run concurrently; a dependent branch SHALL start only after the branches it names
have completed. A dependency cycle SHALL be a compile-time error.

#### Scenario: Independent branches overlap
- **WHEN** two bindings in the block reference neither each other nor a third binding
- **THEN** both branches start together

#### Scenario: Dependent branch waits for its input
- **WHEN** `inmut b = g(a)` appears in a block that also declares `inmut a = f()`
- **THEN** `b`'s branch starts only after `a`'s branch completes, while other independent branches still overlap

#### Scenario: Dependency cycle is rejected
- **WHEN** `inmut a = f(b)` and `inmut b = g(a)` appear in the same block
- **THEN** compilation fails naming the cycle

### Requirement: `spawn` adds a dynamic branch

`spawn expr` and `spawn { block }` SHALL be valid only lexically inside a
`concurrent` block or the implicit root scope of `main`. A `spawn` outside any
such scope SHALL be a compile-time error naming `concurrent { }`. The enclosing
block SHALL wait for every spawned branch. `spawn` body captures SHALL follow the
ordinary closure rules.

#### Scenario: spawn in a loop
- **WHEN** `for x in xs { spawn f(x) }` appears in a `concurrent` block
- **THEN** the block waits for every `f(x)` branch before closing

#### Scenario: spawn outside a scope is rejected
- **WHEN** `spawn f()` appears in a plain function body with no enclosing `concurrent` block
- **THEN** compilation fails and directs the developer to wrap it in `concurrent { }`

### Requirement: `Job<T>` handle

`spawn expr` where `expr` has type `T` SHALL produce a `Job<T>`. `job.wait()`
SHALL produce exactly `T` and SHALL consume the handle: a second `wait()` or any
later use SHALL be a compile-time use-after-consume error identifying the first
consume. `job.cancel()` SHALL request cancellation of the branch. `job.done`
SHALL be a `Boolean`. A `Job<T>` that leaves its scope neither waited nor
cancelled SHALL be a must-use diagnostic dischargeable with `_ = job`; the scope
still joins the branch.

#### Scenario: wait consumes the handle
- **WHEN** `job.wait()` appears twice for the same handle
- **THEN** the second is a use-after-consume error pointing at the first

#### Scenario: unused job is diagnosed
- **WHEN** `spawn f()` is bound to a handle that is never waited or cancelled and the scope ends
- **THEN** a must-use diagnostic is emitted, dischargeable with `_ =`

### Requirement: Structured failure and cooperative cancellation of a scope

When a branch throws an unhandled exception the scope SHALL make it the primary
failure, request cancellation of the sibling branches, await their cleanup,
attach further failures as suppressed, and propagate out of the block.
Cancellation SHALL be cooperative and idempotent, observed only at safe points
(`Timer.sleep`, a blocking channel operation, an explicit cancellation check),
propagated from a scope to its branches, and SHALL raise the compiler-known
`CancelledError`. A `Result.Error` returned by a branch SHALL remain an ordinary
branch value.

#### Scenario: sibling failure cancels the rest
- **WHEN** one branch throws while a sibling is suspended at a safe point
- **THEN** the sibling observes `CancelledError`, runs cleanup, and the thrown exception propagates out of the block with the sibling's outcome suppressed if it also failed

#### Scenario: returned Result.Error is not a failure
- **WHEN** a branch returns `Error(problem)` normally while a sibling runs
- **THEN** the sibling is not cancelled and the block completes with that branch's value being `Error(problem)`

### Requirement: `main` runs in an implicit `concurrent` scope

The body of `main` SHALL be an implicit `concurrent` scope. Top-level `spawn`,
`Timer.after`, and `Timer.every` SHALL be owned by it. The process SHALL NOT exit
until `main`'s explicit body has completed and its owned branches have been
joined or (for ambient timers, and on an exceptional exit) cancelled and cleaned.

#### Scenario: spawned work outlives the last statement of main
- **WHEN** `main` spawns a branch and then reaches the end of its body
- **THEN** the implicit scope waits for that branch before the process exits

