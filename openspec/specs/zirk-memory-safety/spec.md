# zirk-memory-safety Specification

## Purpose
Defines managed-memory guarantees, dependent and weak references, native views,
pointers, unsafe rollback, irreversible commit, and safety diagnostics.
## Requirements
### Requirement: Strategy-neutral automatic memory
Zirk SHALL reclaim unreachable managed memory including cycles without exposing GC, ownership, regions, moves, or reference counting as mandatory source semantics, and representation changes MUST preserve identity and observable lifetime. This guarantee covers all runtime-allocated opaque handles, including `String` and `Char` values.

#### Scenario: Runtime moves an object
- **WHEN** the runtime relocates a live managed object
- **THEN** safe references remain valid and `is` observes the same identity

#### Scenario: A string becomes unreachable
- **WHEN** a `String` value produced by concatenation, conversion, or slicing is no longer reachable from any root
- **THEN** the collector reclaims both the handle and its bytes

#### Scenario: A character becomes unreachable
- **WHEN** a `Char` value produced by `String[index]` or `Char` literal materialization is no longer reachable from any root
- **THEN** the collector reclaims the character object

### Requirement: Deterministic cleanup belongs to resources
Managed objects MUST NOT expose general-purpose finalizers whose timing is observable, and deterministic cleanup SHALL use `Resource<E>` and `match with`.

#### Scenario: Managed object becomes unreachable
- **WHEN** an ordinary managed object becomes unreachable
- **THEN** the program cannot depend on a destructor running at that moment

### Requirement: Weak references
`Weak<T>` SHALL NOT keep its referent alive, SHALL require `upgrade(): T?` before safe use, and SHALL expose `is_alive: Boolean` only as an observational hint subject to concurrent change.

#### Scenario: Weak referent was reclaimed
- **WHEN** `upgrade()` is called after no strong reference keeps the referent alive
- **THEN** it returns `null` rather than exposing reclaimed memory

### Requirement: Checked dependent lifetimes
Native views, pinned borrows, borrowed iterators, resource-derived handles, and internal-storage views MUST NOT escape the lifetime of their owner, and the compiler SHALL enforce this without requiring public lifetime syntax.

#### Scenario: Native view escapes its borrow
- **WHEN** code attempts to return a `NativeSlice<T>` whose validated owner ends in the function
- **THEN** compilation fails with the owner and escape path identified

### Requirement: Deep clone graph semantics
Deep `clone()` SHALL create new identity for cloned reference objects, preserve internal sharing and cycles within the new graph, and fail at compile time when the declared graph contains a non-`Clone` resource, pointer, lock, task, or other member.

#### Scenario: Graph contains shared child
- **WHEN** two source fields refer to the same clonable child and the root is cloned
- **THEN** the two cloned fields refer to one new cloned child rather than the source child or two unrelated copies

#### Scenario: Graph contains a cycle
- **WHEN** a clonable object's field reaches back to an ancestor already being cloned in the same `clone()` call
- **THEN** the clone completes without infinite recursion and the cloned cycle mirrors the source cycle among new identities

#### Scenario: Graph reaches a non-Clone member
- **WHEN** a class declares a field of type `Resource`, `Pointer<T>`, a lock, `Task<T>`, or another type that is not `Clone`
- **THEN** compilation rejects deriving or using `Clone` for that class, naming the offending field

### Requirement: Closed unsafe operation set
Raw pointer creation, dereference, arithmetic and representation casts; unsafe native calls; unchecked native construction; untagged native-union access; weak atomic ordering; and manual safety-contract implementation SHALL require an explicit unsafe boundary.

#### Scenario: Safe code dereferences a pointer
- **WHEN** code dereferences `Pointer<T>` outside an unsafe block
- **THEN** compilation fails and identifies the required unsafe operation

### Requirement: Pointer and native-view behavior
Pointer arithmetic SHALL be measured in elements, byte offsets SHALL be explicit, null pointers SHALL be permitted only as raw native values, and validated `NativeSlice<T>`/`NativeSliceMut<T>` views SHALL carry bounded extent and lifetime.

#### Scenario: Null raw pointer is inspected
- **WHEN** native code returns a null `Pointer<T>`
- **THEN** `is_null` can inspect it without creating a nullable safe reference

#### Scenario: View indexing stays bounds-checked
- **WHEN** safe code indexes a validated `NativeSlice<T>`/`NativeSliceMut<T>` with an out-of-range position
- **THEN** the operation fails with a controlled bounds error rather than reading or writing outside the validated extent

#### Scenario: Construction validates before a view exists
- **WHEN** `pointer.as_slice(length)` or `pointer.as_slice_mut(length)` is called with a null pointer, misaligned address, or unrepresentable extent
- **THEN** construction returns `Error` and no `NativeSlice<T>`/`NativeSliceMut<T>` value is produced

### Requirement: Transactional unsafe mutation
An ordinary unsafe block SHALL isolate and journal writes to Zirk-managed state and validated native ranges, SHALL commit them on success, and SHALL close newly acquired resources and roll them back on a controlled `Error`, exception, runtime trap, or cancellation before commit.

#### Scenario: Validation fails after managed writes
- **WHEN** an unsafe block mutates managed state and then propagates a validation `Error`
- **THEN** the managed state is restored to its pre-block value before the error escapes

### Requirement: Irreversible commit boundary
External I/O, unknown-effect FFI, volatile or device memory, manual release, concurrently observable publication, and raw writes without proven provenance and extent MUST occur inside an explicit `commit {}` region within unsafe code.

#### Scenario: Socket send appears in reversible region
- **WHEN** unsafe code attempts to send network bytes before an explicit commit boundary
- **THEN** compilation fails because the effect cannot be rolled back

### Requirement: Unsafe transaction isolation
An unsafe transaction MUST NOT suspend, await, spawn a task or thread, or expose tentative state to another execution context, and entering `commit` SHALL publish pending reversible writes before irreversible effects execute.

#### Scenario: Await appears in unsafe transaction
- **WHEN** an ordinary transactional unsafe block contains `await`
- **THEN** compilation fails and requires completion or commit before suspension

#### Scenario: Commit publishes pending writes before an irreversible effect
- **WHEN** an `unsafe {}` block has journaled managed writes and then enters `commit {}`
- **THEN** every journaled write is durably committed before `commit`'s own body executes

### Requirement: Honest failure boundary
Zirk SHALL guarantee rollback only for controlled failures detected before irreversible commit and MUST NOT claim recovery after arbitrary native corruption, invalid instructions, abrupt process termination, or true undefined behavior.

#### Scenario: Development sanitizer detects invalid native access
- **WHEN** a development build detects an invalid native access before corruption
- **THEN** it may raise a controlled trap and roll back the still-reversible transaction

### Requirement: Unsafe journal rolls back on non-exceptional exits
An `unsafe { ... }` block whose journal has not been committed by `commit { ... }` SHALL roll back all recorded writes when control leaves the block through `return`, `break`, or `continue`.

#### Scenario: return inside unsafe rolls back
- **WHEN** a `return` statement appears inside an `unsafe { ... }` block that has recorded slot or field writes
- **THEN** the compiler emits a `JournalRollback` for that block before the function returns

#### Scenario: break inside unsafe rolls back
- **WHEN** a `break` statement appears inside an `unsafe { ... }` block
- **THEN** the compiler emits a `JournalRollback` for that block before jumping to the loop's `break_to` target

#### Scenario: continue inside unsafe rolls back
- **WHEN** a `continue` statement appears inside an `unsafe { ... }` block
- **THEN** the compiler emits a `JournalRollback` for that block before jumping to the loop's `continue_to` target

#### Scenario: nested try and unsafe cleanup order is lexical
- **WHEN** `try { unsafe { ... return ... } }` or `unsafe { try { ... return ... } }` is compiled
- **THEN** `finally` and `JournalRollback` are emitted in the order they were lexically entered, from innermost to outermost

#### Scenario: commit prevents rollback
- **WHEN** `commit { ... }` is executed inside `unsafe { ... }` before a `return`/`break`/`continue`
- **THEN** the journal is committed and no `JournalRollback` is emitted for that `unsafe` frame

### Requirement: inmut::strict applies to field declarations
A field declared `inmut::strict` SHALL be unwritable through any projection, regardless of whether its container is `mut` or `inmut`.

#### Scenario: Strict field written through mut container
- **WHEN** a `record R { x: inmut::strict Int32 }` exists and code writes `r.x = 5` even though `r` is `mut`
- **THEN** compilation fails with a strict-field write diagnostic

### Requirement: inmut::strict rejects mutating method calls
An `inmut::strict` reference SHALL NOT be used as the receiver of a method declared `mut`, and the compiler SHALL reject such a call.

#### Scenario: Mutating call on strict reference
- **WHEN** `p: inmut::strict Point` and `p.move()` is called where `move` is declared `mut`
- **THEN** compilation fails with a strict-mutation diagnostic

#### Scenario: Non-mutating call on strict reference is allowed
- **WHEN** `p: inmut::strict Point` and `p.distance()` is called where `distance` does not mutate
- **THEN** the call type-checks and compiles

### Requirement: Native view provenance is tracked
The compiler SHALL track the known extent of a `NativeSlice<T>`/`NativeSliceMut<T>` constructed from `Pointer.from(place).as_slice(n)` and SHALL reject uses that exceed that extent.

#### Scenario: View exceeds known extent
- **WHEN** `Pointer.from(arr).as_slice(100)` is used and the compiler can prove `arr` has fewer than 100 elements
- **THEN** compilation fails with an extent diagnostic

#### Scenario: View within known extent is accepted
- **WHEN** `Pointer.from(arr).as_slice(100)` is used and `arr` has at least 100 elements
- **THEN** the expression type-checks

