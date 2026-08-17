## ADDED Requirements

### Requirement: Strategy-neutral automatic memory
Zirk SHALL reclaim unreachable managed memory including cycles without exposing GC, ownership, regions, moves, or reference counting as mandatory source semantics, and representation changes MUST preserve identity and observable lifetime.

#### Scenario: Runtime moves an object
- **WHEN** the runtime relocates a live managed object
- **THEN** safe references remain valid and `is` observes the same identity

### Requirement: Deterministic cleanup belongs to resources
Managed objects MUST NOT expose general-purpose finalizers whose timing is observable, and deterministic cleanup SHALL use `Resource<E>` and `match with`.

#### Scenario: Managed object becomes unreachable
- **WHEN** an ordinary managed object becomes unreachable
- **THEN** the program cannot depend on a destructor running at that moment

### Requirement: Weak references
`Weak<T>` SHALL NOT keep its referent alive, SHALL require `upgrade(): Option<T>` before safe use, and SHALL expose `is_alive: Boolean` only as an observational hint subject to concurrent change.

#### Scenario: Weak referent was reclaimed
- **WHEN** `upgrade()` is called after no strong reference keeps the referent alive
- **THEN** it returns `None` rather than exposing reclaimed memory

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

### Requirement: Honest failure boundary
Zirk SHALL guarantee rollback only for controlled failures detected before irreversible commit and MUST NOT claim recovery after arbitrary native corruption, invalid instructions, abrupt process termination, or true undefined behavior.

#### Scenario: Development sanitizer detects invalid native access
- **WHEN** a development build detects an invalid native access before corruption
- **THEN** it may raise a controlled trap and roll back the still-reversible transaction
