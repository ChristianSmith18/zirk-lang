## ADDED Requirements

### Requirement: Job handle type

`Job<T>` SHALL be a known one-parameter generic type. `spawn expr` SHALL be typed
`Job<T>` where `T` is the type `expr` produces; `spawn { block }` SHALL be typed
`Job<T>` where `T` is the block's result type. `job.wait()` SHALL be typed exactly
`T` and SHALL consume the binding linearly: a second `wait()` or any later use of
the binding SHALL be a compile-time use-after-consume error identifying the first
consume. `job.cancel()` SHALL be `Void`; `job.done` SHALL be `Boolean`. A `Job<T>`
that leaves its scope neither waited nor cancelled SHALL be a must-use diagnostic
dischargeable with `_ = job`.

#### Scenario: spawn is a Job
- **WHEN** `inmut h = spawn compute();` is checked where `compute(): Int32`
- **THEN** `h` has type `Job<Int32>` and `h.wait()` has type `Int32`

#### Scenario: second wait is rejected
- **WHEN** `h.wait()` appears twice for the same handle
- **THEN** the second is a use-after-consume error pointing at the first

### Requirement: Timer static-member typing

`Timer.sleep(d)` SHALL require `d: Duration` and be typed `Void`.
`Timer.after(d, thunk)` SHALL require `d: Duration` and `thunk: (): T` and be
typed `Job<T>`. `Timer.every(d, thunk)` SHALL require `d: Duration` and
`thunk: (): Void` and be typed `Job<Void>`.

#### Scenario: Timer.after carries the thunk result type
- **WHEN** `inmut j = Timer.after(1s, (): Int32 => 42);` is checked
- **THEN** `j` has type `Job<Int32>`

### Requirement: Concurrent block binding hoisting

Bindings declared directly in a `concurrent { }` block SHALL be introduced into
the enclosing scope with the type of their branch initializer, available after
the block. Reading such a binding before the block closes, other than inside a
dependent branch, SHALL be a compile-time error. A dependency cycle among the
block's bindings SHALL be a compile-time error.

#### Scenario: hoisted bindings are usable after the block
- **WHEN** `concurrent { inmut a = f(); }` is followed by `use(a)` where `f(): User`
- **THEN** `a` has type `User` after the block

#### Scenario: dependency cycle is rejected
- **WHEN** `inmut a = f(b)` and `inmut b = g(a)` appear in the same block
- **THEN** compilation fails naming the cycle
